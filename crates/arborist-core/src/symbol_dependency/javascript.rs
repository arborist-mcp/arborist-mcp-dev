use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;

use crate::language::{
    ParsedDocument, detect_language, javascript_export_local_names,
    javascript_module_reexport_module_paths_with_overrides_and_check,
    javascript_named_export_names, javascript_named_import_module_paths_with_overrides_and_check,
    javascript_named_reexport_module_paths_with_overrides_and_check,
    javascript_star_reexport_module_paths_with_overrides_and_check, normalize_path, parse_document,
    parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct JavaScriptImportBinding {
    pub(crate) imported_name: String,
    pub(crate) module_paths: BTreeSet<String>,
    pub(crate) unresolved: bool,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct JavaScriptImportContext {
    pub(crate) named_import_bindings: BTreeMap<String, JavaScriptImportBinding>,
    pub(crate) named_reexport_bindings: BTreeMap<String, JavaScriptImportBinding>,
    pub(crate) star_reexport_module_paths: BTreeSet<String>,
    pub(crate) named_export_names: BTreeSet<String>,
    /// Exported-name to local-name mappings for aliased direct exports
    /// (`export { local as exported }` and CommonJS
    /// `module.exports = { exported: local }`), so namespace members resolve
    /// to the declaring local symbol.
    pub(crate) export_local_names: BTreeMap<String, String>,
    /// Local module paths re-exported wholesale through
    /// `module.exports = require("./module")`; the module's namespace is the
    /// target module's export object, so members and callable-object calls
    /// resolve within the terminal module of the re-export chain.
    pub(crate) module_reexport_module_paths: BTreeSet<String>,
}

fn javascript_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaScriptImportContext> {
    let path = Path::new(file_path);
    if !matches!(
        detect_language(path).ok(),
        Some(LanguageId::JavaScript | LanguageId::TypeScript | LanguageId::Tsx)
    ) {
        return Ok(JavaScriptImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading JavaScript/TypeScript import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing JavaScript/TypeScript import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing JavaScript/TypeScript import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    if let Some(deadline) = deadline {
        deadline.check("extracting JavaScript/TypeScript import bindings")?;
    }

    let check_traversal_deadline = || -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("extracting JavaScript/TypeScript import bindings")?;
        }
        Ok(())
    };

    Ok(JavaScriptImportContext {
        named_import_bindings: javascript_named_import_module_paths_with_overrides_and_check(
            path,
            document.tree.root_node(),
            &source,
            file_overrides,
            Some(&check_traversal_deadline),
        )?
        .into_iter()
        .map(|(local_name, binding)| {
            (
                local_name,
                JavaScriptImportBinding {
                    imported_name: binding.imported_name,
                    module_paths: binding
                        .module_paths
                        .into_iter()
                        .map(|path| normalize_path(&path))
                        .collect(),
                    unresolved: binding.unresolved,
                },
            )
        })
        .collect(),
        named_reexport_bindings: javascript_named_reexport_module_paths_with_overrides_and_check(
            path,
            document.tree.root_node(),
            &source,
            file_overrides,
            Some(&check_traversal_deadline),
        )?
        .into_iter()
        .map(|(exported_name, binding)| {
            (
                exported_name,
                JavaScriptImportBinding {
                    imported_name: binding.imported_name,
                    module_paths: binding
                        .module_paths
                        .into_iter()
                        .map(|path| normalize_path(&path))
                        .collect(),
                    unresolved: binding.unresolved,
                },
            )
        })
        .collect(),
        star_reexport_module_paths: javascript_star_reexport_module_paths_with_overrides_and_check(
            path,
            document.tree.root_node(),
            &source,
            file_overrides,
            Some(&check_traversal_deadline),
        )?
        .into_iter()
        .map(|path| normalize_path(&path))
        .collect(),
        named_export_names: javascript_named_export_names(
            document.tree.root_node(),
            &source,
            Some(&check_traversal_deadline),
        )?,
        export_local_names: javascript_export_local_names(
            document.tree.root_node(),
            &source,
            Some(&check_traversal_deadline),
        )?,
        module_reexport_module_paths:
            javascript_module_reexport_module_paths_with_overrides_and_check(
                path,
                document.tree.root_node(),
                &source,
                file_overrides,
                Some(&check_traversal_deadline),
            )?
            .into_iter()
            .map(|path| normalize_path(&path))
            .collect(),
    })
}

/// Follows `module.exports = require("./module")` wholesale re-export chains
/// from `module_path` to the terminal module that actually declares exports.
/// Returns `None` when the chain is empty, ambiguous (multiple targets), or
/// cyclic so callers fail closed instead of guessing.
fn javascript_module_reexport_terminal(
    module_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    let mut visited = BTreeSet::new();
    let mut current = module_path.to_owned();
    loop {
        if !visited.insert(current.clone()) {
            return Ok(None);
        }
        let context = javascript_import_context_from_cache(
            &current,
            file_overrides,
            contexts_by_file,
            deadline,
        )?;
        match context.module_reexport_module_paths.len() {
            0 => return Ok(Some(current)),
            1 => {
                let Some(target) = context.module_reexport_module_paths.iter().next() else {
                    return Ok(None);
                };
                current = target.clone();
            }
            _ => return Ok(None),
        }
    }
}

pub(in crate::symbol_dependency) fn resolve_javascript_named_import_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<JavaScriptImportBinding>> {
    let source_context = javascript_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(binding) = source_context.named_import_bindings.get(reference_name) else {
        return Ok(None);
    };

    let mut resolution_stack = BTreeSet::new();
    Ok(Some(resolve_named_module_binding(
        binding.clone(),
        file_overrides,
        contexts_by_file,
        deadline,
        &mut resolution_stack,
    )?))
}

fn resolve_named_module_binding(
    binding: JavaScriptImportBinding,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
    resolution_stack: &mut BTreeSet<(String, String)>,
) -> Result<JavaScriptImportBinding> {
    if let Some(deadline) = deadline {
        deadline.check("resolving JavaScript/TypeScript named import")?;
    }
    if binding.unresolved || binding.module_paths.is_empty() {
        return Ok(binding);
    }

    let mut resolved: Option<JavaScriptImportBinding> = None;
    for module_path in &binding.module_paths {
        let resolution_key = (module_path.clone(), binding.imported_name.clone());
        if !resolution_stack.insert(resolution_key.clone()) {
            return Ok(unresolved_named_module_binding(&binding.imported_name));
        }

        let module_context = javascript_import_context_from_cache(
            module_path,
            file_overrides,
            contexts_by_file,
            deadline,
        )?;
        // A wholesale re-export (`module.exports = require("./impl")`) aliases
        // this module's exports to the target module's, so a named member
        // resolves within the terminal module of the chain. Namespace
        // bindings keep pointing at the bound module; member and
        // namespace-object resolution follow the chain at use time.
        let effective_module_path = if binding.imported_name == "<namespace>"
            || module_context.module_reexport_module_paths.is_empty()
        {
            module_path.clone()
        } else {
            let Some(terminal) = javascript_module_reexport_terminal(
                module_path,
                file_overrides,
                contexts_by_file,
                deadline,
            )?
            else {
                resolution_stack.remove(&resolution_key);
                return Ok(unresolved_named_module_binding(&binding.imported_name));
            };
            terminal
        };
        let module_context = javascript_import_context_from_cache(
            &effective_module_path,
            file_overrides,
            contexts_by_file,
            deadline,
        )?;
        let candidate = if let Some(reexport_binding) = module_context
            .named_reexport_bindings
            .get(&binding.imported_name)
        {
            resolve_named_module_binding(
                reexport_binding.clone(),
                file_overrides,
                contexts_by_file,
                deadline,
                resolution_stack,
            )?
        } else if binding.imported_name != "default" {
            // `export * from "./module"` forwards the target's named exports,
            // but never a module's default export.
            match resolve_star_reexported_module_paths(
                &effective_module_path,
                &binding.imported_name,
                file_overrides,
                contexts_by_file,
                deadline,
                resolution_stack,
            )? {
                StarReexportLookup::Unresolved => {
                    return Ok(unresolved_named_module_binding(&binding.imported_name));
                }
                StarReexportLookup::Found(paths) if paths.len() == 1 => JavaScriptImportBinding {
                    imported_name: binding.imported_name.clone(),
                    module_paths: paths,
                    unresolved: false,
                },
                // Multiple defining modules make the star re-export ambiguous;
                // fail closed instead of guessing.
                StarReexportLookup::Found(_) => {
                    return Ok(unresolved_named_module_binding(&binding.imported_name));
                }
                StarReexportLookup::Absent => JavaScriptImportBinding {
                    imported_name: binding.imported_name.clone(),
                    module_paths: BTreeSet::from([effective_module_path.clone()]),
                    unresolved: false,
                },
            }
        } else {
            JavaScriptImportBinding {
                imported_name: binding.imported_name.clone(),
                module_paths: BTreeSet::from([effective_module_path.clone()]),
                unresolved: false,
            }
        };
        resolution_stack.remove(&resolution_key);

        if candidate.unresolved || candidate.module_paths.is_empty() {
            return Ok(unresolved_named_module_binding(&binding.imported_name));
        }
        match &mut resolved {
            Some(resolved_binding) if resolved_binding.imported_name == candidate.imported_name => {
                resolved_binding.module_paths.extend(candidate.module_paths);
            }
            Some(_) => return Ok(unresolved_named_module_binding(&binding.imported_name)),
            None => resolved = Some(candidate),
        }
    }

    Ok(resolved.unwrap_or(binding))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StarReexportLookup {
    /// The name is not reachable through the module's re-export machinery.
    Absent,
    /// The name is exported; the set holds the defining module paths (never
    /// empty, and more than one entry means the export is ambiguous).
    Found(BTreeSet<String>),
    /// The re-export chain is broken, ambiguous, or cyclic and must fail
    /// closed.
    Unresolved,
}

/// Resolves the module paths that define `name` when it is exported by
/// `module_path` through star re-exports, following named re-export chains
/// and nested star re-exports transitively with cycle detection. Direct named
/// exports shadow star re-exports of the same name, matching ESM semantics.
fn resolve_star_reexported_module_paths(
    module_path: &str,
    name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
    resolution_stack: &mut BTreeSet<(String, String)>,
) -> Result<StarReexportLookup> {
    if let Some(deadline) = deadline {
        deadline.check("resolving JavaScript/TypeScript star re-export")?;
    }
    // `export *` never re-exports a module's default export.
    if name == "default" {
        return Ok(StarReexportLookup::Absent);
    }
    let module_context = javascript_import_context_from_cache(
        module_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    // A named re-export of the name is an explicit export and takes
    // precedence over any star re-export of the same name.
    if let Some(reexport_binding) = module_context.named_reexport_bindings.get(name) {
        let resolved = resolve_named_module_binding(
            reexport_binding.clone(),
            file_overrides,
            contexts_by_file,
            deadline,
            resolution_stack,
        )?;
        if resolved.unresolved || resolved.module_paths.is_empty() {
            return Ok(StarReexportLookup::Unresolved);
        }
        return Ok(StarReexportLookup::Found(resolved.module_paths));
    }
    // A direct named export shadows star re-exports of the same name.
    if module_context.named_export_names.contains(name) {
        return Ok(StarReexportLookup::Found(BTreeSet::from([
            module_path.to_owned()
        ])));
    }
    if module_context.star_reexport_module_paths.is_empty() {
        return Ok(StarReexportLookup::Absent);
    }
    let mut defining_paths = BTreeSet::new();
    for star_target in &module_context.star_reexport_module_paths {
        let resolution_key = (star_target.clone(), name.to_owned());
        if !resolution_stack.insert(resolution_key.clone()) {
            // A star re-export cycle must fail closed.
            return Ok(StarReexportLookup::Unresolved);
        }
        match resolve_star_reexported_module_paths(
            star_target,
            name,
            file_overrides,
            contexts_by_file,
            deadline,
            resolution_stack,
        )? {
            StarReexportLookup::Unresolved => {
                resolution_stack.remove(&resolution_key);
                return Ok(StarReexportLookup::Unresolved);
            }
            StarReexportLookup::Found(paths) => defining_paths.extend(paths),
            StarReexportLookup::Absent => {}
        }
        resolution_stack.remove(&resolution_key);
    }
    Ok(if defining_paths.is_empty() {
        StarReexportLookup::Absent
    } else {
        StarReexportLookup::Found(defining_paths)
    })
}

fn unresolved_named_module_binding(imported_name: &str) -> JavaScriptImportBinding {
    JavaScriptImportBinding {
        imported_name: imported_name.to_owned(),
        module_paths: BTreeSet::new(),
        unresolved: true,
    }
}

/// Reads and parses a JavaScript/TypeScript module with the cooperative parse
/// deadline applied when one is supplied.
fn read_javascript_module_document(
    module_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
    deadline_label: &'static str,
) -> Result<(String, ParsedDocument)> {
    let path = Path::new(module_path);
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros(deadline_label)?,
        )?
    } else {
        parse_document(path, &source)?
    };
    Ok((source, document))
}

/// Returns the local name of `module_path`'s default export when it is a
/// named declaration (`export default function foo() {}`,
/// `export default foo;`, `export { foo as default };`) or a CommonJS
/// `exports.default = ...` / `module.exports.default = ...` member assignment
/// naming a module-level symbol; anonymous default exports, conflicting
/// default declarations, and modules with absent default exports return `None`
/// and fail closed. The CommonJS callable `module.exports = ...` export is not
/// a `.default` member and is intentionally excluded here.
pub(in crate::symbol_dependency) fn resolve_javascript_module_default_export_name(
    module_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(deadline) = deadline {
        deadline.check("reading JavaScript/TypeScript default export module")?;
    }
    let (source, document) = read_javascript_module_document(
        module_path,
        file_overrides,
        deadline,
        "parsing JavaScript/TypeScript default export module",
    )?;
    crate::language::javascript_module_default_export_local_name(document.tree.root_node(), &source)
}

/// Returns the local name a default import (`import name from "./module")`
/// binds when the module's default export can be resolved conservatively. The
/// module's ESM default export or CommonJS `exports.default` /
/// `module.exports.default` member assignment names the target; when neither
/// exists, a CommonJS callable `module.exports = <callable>` export is the
/// default import target under interop semantics. Ambiguous or absent default
/// exports return `None` and fail closed.
pub(in crate::symbol_dependency) fn resolve_javascript_default_import_local_name(
    module_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<String>> {
    if let Some(deadline) = deadline {
        deadline.check("reading JavaScript/TypeScript default import module")?;
    }
    let (source, document) = read_javascript_module_document(
        module_path,
        file_overrides,
        deadline,
        "parsing JavaScript/TypeScript default import module",
    )?;
    let root = document.tree.root_node();
    let mut names = BTreeSet::new();
    if let Some(name) = crate::language::javascript_module_default_export_local_name(root, &source)?
    {
        names.insert(name);
    }
    if names.is_empty()
        && let Some(name) =
            crate::language::javascript_module_callable_export_local_name(root, &source)?
    {
        names.insert(name);
    }
    Ok((names.len() == 1)
        .then(|| names.iter().next().cloned())
        .flatten())
}

/// Resolves the binding for a bare call to `module_path`'s namespace object
/// (`ns(...)` in CommonJS interop) when the module exports a single callable
/// value through `module.exports = ...`. The returned binding's `imported_name`
/// is the callable's local name in the module. `None` means the module is not a
/// CommonJS callable export (including ESM-only `.mjs`/`.mts` modules), and
/// namespace-object calls must fail closed for it.
pub(in crate::symbol_dependency) fn resolve_javascript_namespace_object_call_binding(
    module_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<JavaScriptImportBinding>> {
    if let Some(deadline) = deadline {
        deadline.check("reading JavaScript/TypeScript CommonJS callable export module")?;
    }
    // `module.exports = require("./impl")` aliases the namespace object to the
    // target module's export object, so callability is decided by the terminal
    // module of the wholesale re-export chain; broken or cyclic chains fail
    // closed.
    let Some(terminal_path) = javascript_module_reexport_terminal(
        module_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?
    else {
        return Ok(None);
    };
    let module_path = terminal_path.as_str();
    // `.mjs` and `.mts` are ESM-only: their namespace objects are never
    // callable even when the source text mentions `module.exports`.
    let extension = Path::new(module_path)
        .extension()
        .and_then(|extension| extension.to_str());
    if matches!(extension, Some("mjs" | "mts")) {
        return Ok(None);
    }
    let path = Path::new(module_path);
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros(
                "parsing JavaScript/TypeScript CommonJS callable export module",
            )?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let Some(callable_name) = crate::language::javascript_module_callable_export_local_name(
        document.tree.root_node(),
        &source,
    )?
    else {
        return Ok(None);
    };
    Ok(Some(JavaScriptImportBinding {
        imported_name: callable_name,
        module_paths: BTreeSet::from([module_path.to_owned()]),
        unresolved: false,
    }))
}

/// Resolves the binding that defines `member_name` when it is accessed as a
/// member of `module_path`'s namespace object. Direct named exports resolve to
/// the module itself; named re-export and star re-export chains are followed
/// transitively with cycle detection, and the `default` member resolves to the
/// module's named default export. The returned binding's `imported_name` is
/// the member's local name in its defining module so aliased re-exports
/// resolve to the right symbol. `None` means the member is not exported or the
/// chain is broken, ambiguous, or cyclic; both cases fail closed for namespace
/// member lookup.
pub(in crate::symbol_dependency) fn resolve_javascript_namespace_member_binding(
    module_path: &str,
    member_name: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<JavaScriptImportBinding>> {
    if let Some(deadline) = deadline {
        deadline.check("resolving JavaScript/TypeScript namespace member")?;
    }
    let module_context = javascript_import_context_from_cache(
        module_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    if !module_context.module_reexport_module_paths.is_empty() {
        // `module.exports = require("./impl")` aliases this module's namespace
        // to the target module's export object; members resolve within the
        // terminal module of the wholesale re-export chain, and broken or
        // cyclic chains fail closed.
        let Some(terminal_path) = javascript_module_reexport_terminal(
            module_path,
            file_overrides,
            contexts_by_file,
            deadline,
        )?
        else {
            return Ok(None);
        };
        return resolve_javascript_namespace_member_binding(
            &terminal_path,
            member_name,
            file_overrides,
            contexts_by_file,
            deadline,
        );
    }
    let mut resolution_stack = BTreeSet::new();
    if let Some(reexport_binding) = module_context.named_reexport_bindings.get(member_name) {
        let resolved = resolve_named_module_binding(
            reexport_binding.clone(),
            file_overrides,
            contexts_by_file,
            deadline,
            &mut resolution_stack,
        )?;
        if resolved.unresolved || resolved.module_paths.is_empty() {
            return Ok(None);
        }
        // `export { default } from "./module"` resolves to a binding whose
        // imported name is still `default`; name its terminal module's actual
        // default export so the right symbol is collected.
        if resolved.imported_name == "default" {
            if resolved.module_paths.len() != 1 {
                return Ok(None);
            }
            let Some(module_path) = resolved.module_paths.iter().next() else {
                return Ok(None);
            };
            let Some(default_name) = resolve_javascript_module_default_export_name(
                module_path,
                file_overrides,
                deadline,
            )?
            else {
                return Ok(None);
            };
            return Ok(Some(JavaScriptImportBinding {
                imported_name: default_name,
                module_paths: resolved.module_paths,
                unresolved: false,
            }));
        }
        return Ok(Some(resolved));
    }
    // A namespace object exposes the module's default export as `default`.
    // `export { default } from "./module"` re-exports are handled above; a
    // direct default export resolves to its named local symbol, and anonymous
    // default exports fail closed. Star re-exports never forward a default.
    if member_name == "default" {
        let Some(default_name) =
            resolve_javascript_module_default_export_name(module_path, file_overrides, deadline)?
        else {
            return Ok(None);
        };
        return Ok(Some(JavaScriptImportBinding {
            imported_name: default_name,
            module_paths: BTreeSet::from([module_path.to_owned()]),
            unresolved: false,
        }));
    }
    if module_context.named_export_names.contains(member_name) {
        // Aliased direct exports (`export { local as exported }` and
        // CommonJS `module.exports = { exported: local }`) resolve to the
        // declaring local symbol so namespace members bind correctly.
        let local_name = module_context
            .export_local_names
            .get(member_name)
            .cloned()
            .unwrap_or_else(|| member_name.to_owned());
        return Ok(Some(JavaScriptImportBinding {
            imported_name: local_name,
            module_paths: BTreeSet::from([module_path.to_owned()]),
            unresolved: false,
        }));
    }
    if module_context.star_reexport_module_paths.is_empty() {
        return Ok(None);
    }
    match resolve_star_reexported_module_paths(
        module_path,
        member_name,
        file_overrides,
        contexts_by_file,
        deadline,
        &mut resolution_stack,
    )? {
        // Multiple defining modules make the star re-export ambiguous; fail
        // closed instead of guessing.
        StarReexportLookup::Found(paths) if paths.len() == 1 => Ok(Some(JavaScriptImportBinding {
            imported_name: member_name.to_owned(),
            module_paths: paths,
            unresolved: false,
        })),
        StarReexportLookup::Found(_)
        | StarReexportLookup::Absent
        | StarReexportLookup::Unresolved => Ok(None),
    }
}

fn javascript_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, JavaScriptImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<JavaScriptImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = javascript_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;

    use super::{
        javascript_import_context_for_file_with_overrides_and_deadline,
        resolve_javascript_default_import_local_name,
        resolve_javascript_named_import_binding_for_reference,
        resolve_javascript_namespace_member_binding,
        resolve_javascript_namespace_object_call_binding,
    };
    use crate::language::normalize_path;

    #[test]
    fn import_context_reads_source_overrides() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-context-overrides-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.ts");
        fs::write(&caller, "export function caller() {}\n").unwrap();
        fs::write(&helper, "export function helper() {}\n").unwrap();
        let mut overrides = BTreeMap::new();
        overrides.insert(
            normalize_path(&caller),
            "import { helper } from \"./helper\";\nexport function caller() { return helper(); }\n"
                .to_owned(),
        );

        let context = javascript_import_context_for_file_with_overrides_and_deadline(
            &normalize_path(&caller),
            Some(&overrides),
            None,
        )
        .unwrap();
        assert!(
            context
                .named_import_bindings
                .get("helper")
                .is_some_and(|binding| binding.module_paths.contains(&normalize_path(&helper)))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_named_reexport_chains_from_source_overrides() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-reexport-context-overrides-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        for path in [&caller, &bridge, &helper] {
            fs::write(path, "export function placeholder() {}\n").unwrap();
        }
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { forwarded as selected } from \"./bridge\";\nselected();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export { helper as forwarded } from \"./helper\";\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "selected",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_named_reexport_chains_through_overlay_only_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-overlay-only-reexports-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { forwarded as selected } from \"./bridge\";\nselected();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export { helper as forwarded } from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "selected",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        assert!(!binding.unresolved);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn treats_named_reexport_cycles_as_unresolved() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-reexport-cycle-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let first = root.join("first.ts");
        let second = root.join("second.ts");
        for path in [&caller, &first, &second] {
            fs::write(path, "export function placeholder() {}\n").unwrap();
        }
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./first\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&first),
                "export { helper } from \"./second\";\n".to_owned(),
            ),
            (
                normalize_path(&second),
                "export { helper } from \"./first\";\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn resolves_named_import_through_star_reexport_from_source_overrides() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-reexport-context-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        assert!(!binding.unresolved);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_star_reexport_through_named_reexport_chain() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-named-chain-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let mid = root.join("mid.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./mid\";\n".to_owned(),
            ),
            (
                normalize_path(&mid),
                "export { helper } from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        assert!(!binding.unresolved);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_star_reexport_to_direct_export_when_both_present() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-direct-shadow-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./helper\";\nexport function helper() {}\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&bridge)])
        );
        assert!(!binding.unresolved);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn treats_star_reexport_cycles_as_unresolved() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-cycle-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let first = root.join("first.ts");
        let second = root.join("second.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./first\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&first),
                "export * from \"./second\";\n".to_owned(),
            ),
            (
                normalize_path(&second),
                "export * from \"./first\";\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn treats_ambiguous_star_reexports_as_unresolved() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-ambiguous-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let first = root.join("first.ts");
        let second = root.join("second.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./first\";\nexport * from \"./second\";\n".to_owned(),
            ),
            (
                normalize_path(&first),
                "export function helper() {}\n".to_owned(),
            ),
            (
                normalize_path(&second),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_star_reexport_missing_exports_absent() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-star-missing-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { helper } from \"./bridge\";\nhelper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * from \"./helper\";\n".to_owned(),
            ),
            (normalize_path(&helper), "function helper() {}\n".to_owned()),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "helper");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&bridge)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_reexports_to_target_module() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-reexport-binding-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { ns } from \"./bridge\";\nns.helper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * as ns from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert_eq!(binding.imported_name, "<namespace>");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_reexports_with_missing_targets_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-reexport-missing-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&caller),
                "import { ns } from \"./bridge\";\nns.helper();\n".to_owned(),
            ),
            (
                normalize_path(&bridge),
                "export * as ns from \"./missing\";\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named import should be recorded");
        assert!(binding.unresolved);
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_member_direct_export_to_module() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-member-direct-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let overrides = BTreeMap::from([(
            normalize_path(&module),
            "export function helper() {}\n".to_owned(),
        )]);

        let binding = resolve_javascript_namespace_member_binding(
            &normalize_path(&module),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("direct export should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&module)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_member_through_star_reexport_chain() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-member-star-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&module),
                "export * from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_namespace_member_binding(
            &normalize_path(&module),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("star re-export should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_member_through_named_reexport_alias() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-member-alias-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&module),
                "export { helper as other } from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_namespace_member_binding(
            &normalize_path(&module),
            "other",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("aliased named re-export should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_member_ambiguous_star_reexports_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-member-ambiguous-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let first = root.join("first.ts");
        let second = root.join("second.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&module),
                "export * from \"./first\";\nexport * from \"./second\";\n".to_owned(),
            ),
            (
                normalize_path(&first),
                "export function helper() {}\n".to_owned(),
            ),
            (
                normalize_path(&second),
                "export function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_namespace_member_binding(
            &normalize_path(&module),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap();
        assert!(
            binding.is_none(),
            "ambiguous star re-exports must fail closed"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_member_non_exported_symbols_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-member-nonexported-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let overrides =
            BTreeMap::from([(normalize_path(&module), "function helper() {}\n".to_owned())]);

        let binding = resolve_javascript_namespace_member_binding(
            &normalize_path(&module),
            "helper",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap();
        assert!(binding.is_none(), "non-exported members must fail closed");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_member_default_export_to_local_name() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-default-direct-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let overrides = BTreeMap::from([(
            normalize_path(&module),
            "export default function helper() {}\n".to_owned(),
        )]);

        let binding = resolve_javascript_namespace_member_binding(
            &normalize_path(&module),
            "default",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named default export should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&module)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_member_default_export_through_named_reexport() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-default-reexport-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let helper = root.join("helper.ts");
        let overrides = BTreeMap::from([
            (
                normalize_path(&module),
                "export { default } from \"./helper\";\n".to_owned(),
            ),
            (
                normalize_path(&helper),
                "export default function helper() {}\n".to_owned(),
            ),
        ]);

        let binding = resolve_javascript_namespace_member_binding(
            &normalize_path(&module),
            "default",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("re-exported default should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_member_anonymous_default_exports_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-namespace-default-anonymous-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        let overrides = BTreeMap::from([(
            normalize_path(&module),
            "export default function () {}\n".to_owned(),
        )]);

        let binding = resolve_javascript_namespace_member_binding(
            &normalize_path(&module),
            "default",
            Some(&overrides),
            &mut BTreeMap::new(),
            None,
        )
        .unwrap();
        assert!(
            binding.is_none(),
            "anonymous default exports must fail closed"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_object_call_binding_for_commonjs_callable_export() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-callable-binding-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.ts");
        fs::write(
            &module,
            "function helper(value) { return value + 1; }\nmodule.exports = helper;\n",
        )
        .unwrap();

        let binding = resolve_javascript_namespace_object_call_binding(
            &normalize_path(&module),
            None,
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("CommonJS callable export should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&module)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_object_call_binding_for_named_function_expression() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-function-expression-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.cjs");
        fs::write(
            &module,
            "module.exports = function helper(value) { return value; };\n",
        )
        .unwrap();

        let binding = resolve_javascript_namespace_object_call_binding(
            &normalize_path(&module),
            None,
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("named function expression export should resolve");
        assert_eq!(binding.imported_name, "helper");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_object_call_binding_fail_closed_for_esm_modules() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-binding-esm-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for (name, source) in [
            ("default.ts", "export default function helper() {}\n"),
            ("named.ts", "export function helper() {}\n"),
        ] {
            let module = root.join(name);
            fs::write(&module, source).unwrap();
            let binding = resolve_javascript_namespace_object_call_binding(
                &normalize_path(&module),
                None,
                &mut BTreeMap::new(),
                None,
            )
            .unwrap();
            assert!(
                binding.is_none(),
                "ESM modules must stay fail-closed for namespace-object calls, source: {source:?}"
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_object_call_binding_fail_closed_for_esm_only_extensions() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-binding-mjs-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let module = root.join("module.mjs");
        fs::write(&module, "function helper() {}\nmodule.exports = helper;\n").unwrap();

        let binding = resolve_javascript_namespace_object_call_binding(
            &normalize_path(&module),
            None,
            &mut BTreeMap::new(),
            None,
        )
        .unwrap();
        assert!(
            binding.is_none(),
            ".mjs namespace objects are never callable and must fail closed"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_object_call_binding_fail_closed_for_non_callable_exports() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-binding-non-callable-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for (name, source) in [
            ("anonymous.ts", "module.exports = function () {}\n"),
            (
                "object.ts",
                "function helper() {}\nmodule.exports = { helper };\n",
            ),
            ("value.ts", "const helper = 42;\nmodule.exports = helper;\n"),
            (
                "conflict.ts",
                "function first() {}\nfunction second() {}\nmodule.exports = first;\nmodule.exports = second;\n",
            ),
            ("arrow.ts", "module.exports = () => 1;\n"),
        ] {
            let module = root.join(name);
            fs::write(&module, source).unwrap();
            let binding = resolve_javascript_namespace_object_call_binding(
                &normalize_path(&module),
                None,
                &mut BTreeMap::new(),
                None,
            )
            .unwrap();
            assert!(
                binding.is_none(),
                "non-callable exports must fail closed, source: {source:?}"
            );
        }
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn resolves_require_namespace_binding_for_references() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-namespace-binding-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.ts");
        fs::write(&helper, "export function helper() {}\n").unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./helper\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        assert_eq!(binding.imported_name, "<namespace>");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("namespace member should resolve through the require binding");
        assert_eq!(member.imported_name, "helper");
        assert_eq!(
            member.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_require_destructured_member_binding_for_references() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-destructured-binding-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.ts");
        fs::write(&helper, "export function helper() {}\n").unwrap();
        fs::write(
            &caller,
            "const { helper: bound } = require(\"./helper\");\nexport function caller() { return bound(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "bound",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("destructured require binding should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_require_namespace_object_call_binding_for_commonjs_callable_export() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-object-call-binding-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let legacy = root.join("legacy.cjs");
        fs::write(&legacy, "function helper() {}\nmodule.exports = helper;\n").unwrap();
        fs::write(
            &caller,
            "const legacy = require(\"./legacy.cjs\");\nexport function caller() { return legacy(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "legacy",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        assert_eq!(binding.imported_name, "<namespace>");
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&legacy)])
        );
        let callable = resolve_javascript_namespace_object_call_binding(
            binding.module_paths.iter().next().unwrap(),
            None,
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("CommonJS callable export should resolve");
        assert_eq!(callable.imported_name, "helper");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_require_missing_module_bindings_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-missing-binding-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        fs::write(
            &caller,
            "const ns = require(\"./missing\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("missing module still records a binding");
        assert!(binding.unresolved);
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn resolves_require_namespace_member_binding_for_commonjs_object_exports() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-object-export-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.cjs");
        fs::write(
            &helper,
            "function helper() {}\nmodule.exports = { helper };\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./helper.cjs\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        assert_eq!(binding.imported_name, "<namespace>");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("CommonJS object export member should resolve");
        assert_eq!(member.imported_name, "helper");
        assert_eq!(
            member.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_aliased_commonjs_object_export_namespace_members_to_local_symbols() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-alias-object-export-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.cjs");
        fs::write(
            &helper,
            "function localHelper() {}\nmodule.exports = { helper: localHelper };\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./helper.cjs\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("aliased CommonJS object export member should resolve");
        assert_eq!(member.imported_name, "localHelper");
        assert_eq!(
            member.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_require_namespace_member_binding_for_commonjs_exports_member_assignments() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-exports-member-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.cjs");
        fs::write(
            &helper,
            "exports.helper = function helper() {}\nmodule.exports.direct = function direct() {}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./helper.cjs\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        for member_name in ["helper", "direct"] {
            let member = resolve_javascript_namespace_member_binding(
                binding.module_paths.iter().next().unwrap(),
                member_name,
                None,
                &mut contexts,
                None,
            )
            .unwrap()
            .unwrap_or_else(|| panic!("{member_name} exports member should resolve"));
            assert_eq!(member.imported_name, member_name);
            assert_eq!(
                member.module_paths,
                BTreeSet::from([normalize_path(&helper)])
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_aliased_commonjs_exports_member_namespace_members_to_local_symbols() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-alias-exports-member-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.cjs");
        fs::write(
            &helper,
            "function localHelper() {}\nexports.helper = localHelper;\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./helper.cjs\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("aliased exports member should resolve");
        assert_eq!(member.imported_name, "localHelper");
        assert_eq!(
            member.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_require_destructured_member_binding_for_commonjs_exports_member_assignments() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-require-destructured-exports-member-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.cjs");
        fs::write(&helper, "exports.helper = function helper() {}\n").unwrap();
        fs::write(
            &caller,
            "const { helper } = require(\"./helper.cjs\");\nexport function caller() { return helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("destructured require binding should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn resolves_typescript_import_equals_namespace_member_binding() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-equals-member-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.cjs");
        fs::write(
            &helper,
            "function helper() {}\nmodule.exports = { helper };\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import ns = require(\"./helper.cjs\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("import-equals binding should resolve");
        assert_eq!(binding.imported_name, "<namespace>");
        assert!(!binding.unresolved);
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("import-equals namespace member should resolve");
        assert_eq!(member.imported_name, "helper");
        assert_eq!(
            member.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_typescript_import_equals_namespace_object_call_binding() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-equals-callable-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let helper = root.join("helper.cjs");
        fs::write(&helper, "module.exports = function helper() {}\n").unwrap();
        fs::write(
            &caller,
            "import fn = require(\"./helper.cjs\");\nexport function caller() { return fn(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "fn",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("import-equals binding should resolve");
        assert_eq!(binding.imported_name, "<namespace>");
        let callable = resolve_javascript_namespace_object_call_binding(
            binding.module_paths.iter().next().unwrap(),
            None,
            &mut BTreeMap::new(),
            None,
        )
        .unwrap()
        .expect("import-equals namespace-object call should resolve");
        assert_eq!(callable.imported_name, "helper");
        assert_eq!(
            callable.module_paths,
            BTreeSet::from([normalize_path(&helper)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_typescript_import_equals_missing_modules_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-import-equals-missing-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        fs::write(
            &caller,
            "import ns = require(\"./missing\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("missing import-equals binding still records a binding");
        assert!(binding.unresolved);
        assert!(binding.module_paths.is_empty());
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn resolves_require_namespace_member_binding_through_module_reexport_chain() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-module-reexport-member-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.cjs");
        let impl_path = root.join("impl.cjs");
        fs::write(&impl_path, "exports.helper = function helper() {}\n").unwrap();
        fs::write(&bridge, "module.exports = require(\"./impl.cjs\");\n").unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./bridge.cjs\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("wholesale re-export member should resolve");
        assert_eq!(member.imported_name, "helper");
        assert_eq!(
            member.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_require_destructured_member_binding_through_module_reexport_chain() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-module-reexport-destructured-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.cjs");
        let impl_path = root.join("impl.cjs");
        fs::write(&impl_path, "exports.helper = function helper() {}\n").unwrap();
        fs::write(&bridge, "module.exports = require(\"./impl.cjs\");\n").unwrap();
        fs::write(
            &caller,
            "const { helper } = require(\"./bridge.cjs\");\nexport function caller() { return helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("destructured require binding should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_named_import_through_module_reexport_chain() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-module-reexport-named-import-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.ts");
        let impl_path = root.join("impl.ts");
        fs::write(&impl_path, "export function helper() {}\n").unwrap();
        fs::write(&bridge, "module.exports = require(\"./impl\");\n").unwrap();
        fs::write(
            &caller,
            "import { helper } from \"./bridge\";\nexport function caller() { return helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("named import binding should resolve");
        assert_eq!(binding.imported_name, "helper");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_object_call_through_module_reexport_chain() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-module-reexport-callable-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.cjs");
        let impl_path = root.join("impl.cjs");
        fs::write(&impl_path, "module.exports = function helper() {}\n").unwrap();
        fs::write(&bridge, "module.exports = require(\"./impl.cjs\");\n").unwrap();
        fs::write(
            &caller,
            "import fn = require(\"./bridge.cjs\");\nexport function caller() { return fn(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "fn",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("import-equals binding should resolve");
        let callable = resolve_javascript_namespace_object_call_binding(
            binding.module_paths.iter().next().unwrap(),
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("wholesale re-export namespace-object call should resolve");
        assert_eq!(callable.imported_name, "helper");
        assert_eq!(
            callable.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_cyclic_module_reexport_chains_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-module-reexport-cycle-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let a = root.join("a.cjs");
        let b = root.join("b.cjs");
        fs::write(&a, "module.exports = require(\"./b.cjs\");\n").unwrap();
        fs::write(&b, "module.exports = require(\"./a.cjs\");\n").unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./a.cjs\");\nexport function caller() { return ns.helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap();
        assert!(
            member.is_none(),
            "cyclic wholesale re-export chains must fail closed"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_cjs_default_member_default_import_binding() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-default-member-import-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let impl_path = root.join("impl.cjs");
        fs::write(
            &impl_path,
            "function helper() {}\nexports.default = helper;\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import helper from \"./impl.cjs\";\nexport function caller() { return helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("default import binding should resolve");
        assert_eq!(binding.imported_name, "default");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        assert_eq!(
            resolve_javascript_default_import_local_name(&normalize_path(&impl_path), None, None,)
                .unwrap(),
            Some("helper".to_owned())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_cjs_callable_default_import_binding() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-callable-default-import-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let impl_path = root.join("server.cjs");
        fs::write(&impl_path, "module.exports = function app() {}\n").unwrap();
        fs::write(
            &caller,
            "import app from \"./server.cjs\";\nexport function caller() { return app(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "app",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("callable default import binding should resolve");
        assert_eq!(binding.imported_name, "default");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        assert_eq!(
            resolve_javascript_default_import_local_name(&normalize_path(&impl_path), None, None,)
                .unwrap(),
            Some("app".to_owned())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_cjs_callable_default_import_over_shadowed_default_member() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-callable-shadow-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let impl_path = root.join("server.cjs");
        // The `module.exports = ...` replacement shadows the earlier
        // `exports.default` member assignment, so the callable stays the
        // default import target.
        fs::write(
            &impl_path,
            "function helper() {}\nfunction app() {}\nexports.default = helper;\nmodule.exports = app;\n",
        )
        .unwrap();
        assert_eq!(
            resolve_javascript_default_import_local_name(&normalize_path(&impl_path), None, None,)
                .unwrap(),
            Some("app".to_owned())
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_ambiguous_cjs_default_imports_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-default-ambiguous-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        for (name, source) in [
            // Competing .default member assignments are ambiguous.
            (
                "conflict.cjs",
                "function a() {}\nfunction b() {}\nexports.default = a;\nexports.default = b;\n",
            ),
            // Anonymous .default values name no symbol.
            ("anonymous.cjs", "exports.default = function () {};\n"),
            // Non-symbol .default values name no module-level symbol.
            ("value.cjs", "exports.default = 42;\n"),
        ] {
            let module = root.join(name);
            fs::write(&module, source).unwrap();
            assert_eq!(
                resolve_javascript_default_import_local_name(&normalize_path(&module), None, None,)
                    .unwrap(),
                None,
                "source {source:?} must fail closed"
            );
        }
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_namespace_default_member_through_cjs_default_member() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-default-member-namespace-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let impl_path = root.join("impl.cjs");
        fs::write(
            &impl_path,
            "function helper() {}\nexports.default = helper;\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import * as ns from \"./impl.cjs\";\nexport function caller() { return ns.default(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "default",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("cjs default member should resolve");
        assert_eq!(member.imported_name, "helper");
        assert_eq!(
            member.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_namespace_default_member_through_cjs_callable_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-callable-default-member-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let impl_path = root.join("impl.cjs");
        fs::write(&impl_path, "module.exports = function helper() {}\n").unwrap();
        fs::write(
            &caller,
            "import * as ns from \"./impl.cjs\";\nexport function caller() { return ns.default(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "default",
            None,
            &mut contexts,
            None,
        )
        .unwrap();
        assert!(
            member.is_none(),
            "a callable module.exports does not expose a .default member"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_default_import_through_wholesale_chain_to_cjs_default_member() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-default-wholesale-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let bridge = root.join("bridge.cjs");
        let impl_path = root.join("impl.cjs");
        fs::write(
            &impl_path,
            "function helper() {}\nexports.default = helper;\n",
        )
        .unwrap();
        fs::write(&bridge, "module.exports = require(\"./impl.cjs\");\n").unwrap();
        fs::write(
            &caller,
            "import helper from \"./bridge.cjs\";\nexport function caller() { return helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("default import binding should resolve through the chain");
        assert_eq!(binding.imported_name, "default");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        assert_eq!(
            resolve_javascript_default_import_local_name(&normalize_path(&impl_path), None, None,)
                .unwrap(),
            Some("helper".to_owned())
        );
        let _ = fs::remove_dir_all(root);
    }
    #[test]
    fn keeps_shadowed_exports_alias_members_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-exports-shadow-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let impl_path = root.join("impl.cjs");
        // The `module.exports = app` replacement abandons the `exports`
        // alias, so `exports.helper` never reaches the exported object and
        // namespace member lookups for it must fail closed.
        fs::write(
            &impl_path,
            "function helper(value) { return value; }\nfunction app() {}\nexports.helper = helper;\nmodule.exports = app;\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./impl.cjs\");\nexport function caller() { return ns.helper(1); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap();
        assert!(
            member.is_none(),
            "shadowed exports alias members must fail closed, member: {member:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_module_exports_members_attached_after_replacement() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-attached-member-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let impl_path = root.join("impl.cjs");
        // The express-style pattern attaches members onto the final callable
        // `module.exports` object, so those members are real namespace exports.
        fs::write(
            &impl_path,
            "function app() {}\nfunction extraFn(value) { return value; }\nmodule.exports = app;\nmodule.exports.extra = extraFn;\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./impl.cjs\");\nexport function caller() { return ns.extra(1); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "extra",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("attached member should resolve");
        assert_eq!(member.imported_name, "extraFn");
        assert_eq!(
            member.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn keeps_pre_replacement_module_exports_members_fail_closed() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-pre-replacement-member-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let impl_path = root.join("impl.cjs");
        // `module.exports.extra` runs before the final replacement, so it
        // mutates an object that gets replaced and must fail closed.
        fs::write(
            &impl_path,
            "function app() {}\nfunction extraFn(value) { return value; }\nmodule.exports.extra = extraFn;\nmodule.exports = app;\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "const ns = require(\"./impl.cjs\");\nexport function caller() { return ns.extra(1); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "ns",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("require namespace binding should resolve");
        let member = resolve_javascript_namespace_member_binding(
            binding.module_paths.iter().next().unwrap(),
            "extra",
            None,
            &mut contexts,
            None,
        )
        .unwrap();
        assert!(
            member.is_none(),
            "pre-replacement member assignments must fail closed, member: {member:?}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_default_import_through_cjs_default_member_attached_after_replacement() {
        let root = std::env::temp_dir().join(format!(
            "arborist-javascript-cjs-default-attached-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ts");
        let impl_path = root.join("impl.cjs");
        fs::write(
            &impl_path,
            "function helper() {}\nfunction app() {}\nmodule.exports = app;\nmodule.exports.default = helper;\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import helper from \"./impl.cjs\";\nexport function caller() { return helper(); }\n",
        )
        .unwrap();
        let mut contexts = BTreeMap::new();
        let binding = resolve_javascript_named_import_binding_for_reference(
            &normalize_path(&caller),
            "helper",
            None,
            &mut contexts,
            None,
        )
        .unwrap()
        .expect("default import binding should resolve");
        assert_eq!(binding.imported_name, "default");
        assert!(!binding.unresolved);
        assert_eq!(
            binding.module_paths,
            BTreeSet::from([normalize_path(&impl_path)])
        );
        assert_eq!(
            resolve_javascript_default_import_local_name(&normalize_path(&impl_path), None, None,)
                .unwrap(),
            Some("helper".to_owned())
        );
        let _ = fs::remove_dir_all(root);
    }
}
