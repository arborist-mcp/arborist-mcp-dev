use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{
    csharp_file_base_types, csharp_file_interface_parents, csharp_file_namespace_imports,
    csharp_file_static_type_imports, csharp_file_type_alias_imports,
    csharp_generic_type_semantic_path, csharp_global_namespace_imports,
    csharp_global_static_type_imports, csharp_global_type_alias_imports, detect_language,
    node_text, normalize_path, parse_document, parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::semantic::csharp::is_csharp_symbol_node;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpTypeAliasBinding {
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpStaticTypeImportBinding {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpNamespaceImportBinding {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_namespace_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpReceiverTypeBindings {
    types_by_name: BTreeMap<String, String>,
}

impl CSharpReceiverTypeBindings {
    /// Returns whether `name` is bound as a local receiver. Callers use this
    /// to distinguish "not bound" (a receiver may be a same-named type
    /// instead) from "bound but unusable" (fail closed).
    pub(in crate::symbol_dependency) fn contains(&self, name: &str) -> bool {
        self.types_by_name.contains_key(name)
    }

    /// Returns the declared type spelling for a uniquely bound name. Names
    /// bound without a usable declared type (`var` locals, lambda parameters,
    /// `foreach` variables, local functions, and type parameters) return
    /// `None`.
    pub(in crate::symbol_dependency) fn type_for(&self, name: &str) -> Option<String> {
        self.types_by_name
            .get(name)
            .filter(|type_name| !type_name.is_empty())
            .cloned()
    }
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpImportContext {
    type_alias_bindings: BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_type_alias_names: BTreeSet<(Option<String>, String)>,
    base_type_bindings_by_range: BTreeMap<(usize, usize), CSharpBaseTypeBinding>,
    interface_parent_bindings_by_range: BTreeMap<(usize, usize), Vec<CSharpInterfaceParentBinding>>,
    receiver_type_bindings_by_range: BTreeMap<(usize, usize), CSharpReceiverTypeBindings>,
    static_type_import_bindings: Vec<CSharpStaticTypeImportBinding>,
    namespace_import_bindings: Vec<CSharpNamespaceImportBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpBaseTypeBinding {
    pub(crate) semantic_type_path: String,
    pub(crate) is_global_qualified: bool,
    pub(crate) alias_name: Option<String>,
    pub(crate) namespace_import_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpInterfaceParentBinding {
    pub(crate) semantic_type_path: String,
    pub(crate) is_global_qualified: bool,
}

/// Outcome of resolving an interface declaration's parent list. `None` means
/// the interface has no `base_list`; `Blocked` means a parent spelling failed
/// to normalize or resolve in the interface's file scope; `Parents` carries
/// the resolved parent bindings.
#[derive(Debug, Clone)]
pub(in crate::symbol_dependency) enum CSharpInterfaceParents {
    None,
    Blocked,
    Parents(Vec<CSharpBaseTypeBinding>),
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpGlobalImportContext {
    type_alias_bindings: BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_type_alias_names: BTreeSet<(Option<String>, String)>,
    static_type_import_bindings: Vec<CSharpStaticTypeImportBinding>,
    namespace_import_bindings: Vec<CSharpNamespaceImportBinding>,
}

pub(in crate::symbol_dependency) fn csharp_global_import_context_for_files_with_overrides_and_deadline(
    source_file_paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpGlobalImportContext> {
    let mut type_alias_bindings = BTreeMap::new();
    let mut ambiguous_type_alias_names = BTreeSet::new();
    let mut static_type_import_bindings = Vec::new();
    let mut namespace_import_bindings = Vec::new();
    let mut visited_paths = BTreeSet::new();

    for source_file_path in source_file_paths {
        if let Some(deadline) = deadline {
            deadline.check("reading C# global import context")?;
        }
        let normalized_file_path = normalize_path(source_file_path);
        if !visited_paths.insert(normalized_file_path.clone()) {
            continue;
        }
        let path = Path::new(&normalized_file_path);
        if detect_language(path).ok() != Some(LanguageId::CSharp) {
            continue;
        }
        let source = file_overrides
            .and_then(|overrides| overrides.get(&normalized_file_path))
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| read_source(path))?;
        if let Some(deadline) = deadline {
            deadline.check("parsing C# global import context")?;
        }
        let document = if let Some(deadline) = deadline {
            parse_document_with_timeout(
                path,
                &source,
                deadline.remaining_timeout_micros("parsing C# global import context")?,
            )?
        } else {
            parse_document(path, &source)?
        };
        let root = document.tree.root_node();
        if root.has_error() {
            continue;
        }
        for (local_name, semantic_type_path) in csharp_global_type_alias_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global type alias bindings")?;
            }
            insert_unique_csharp_type_alias_binding(
                &mut type_alias_bindings,
                &mut ambiguous_type_alias_names,
                None,
                local_name,
                CSharpTypeAliasBinding { semantic_type_path },
            );
        }
        for semantic_type_path in csharp_global_static_type_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global static import bindings")?;
            }
            static_type_import_bindings.push(CSharpStaticTypeImportBinding {
                scope_path: None,
                semantic_type_path,
            });
        }
        for semantic_namespace_path in csharp_global_namespace_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global namespace import bindings")?;
            }
            namespace_import_bindings.push(CSharpNamespaceImportBinding {
                scope_path: None,
                semantic_namespace_path,
            });
        }
    }

    Ok(CSharpGlobalImportContext {
        type_alias_bindings,
        ambiguous_type_alias_names,
        static_type_import_bindings,
        namespace_import_bindings,
    })
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_type_alias_binding_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Option<(String, CSharpTypeAliasBinding)> {
    let (local_type_name, method_name) = reference_name.split_once('.')?;
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return None;
    }
    let binding = context
        .type_alias_bindings
        .get(&(None, local_type_name.to_string()))?
        .clone();
    Some((method_name.to_string(), binding))
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_nested_type_alias_binding_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Option<(String, String, CSharpTypeAliasBinding)> {
    let (local_type_name, nested_reference) = reference_name.split_once('.')?;
    let (nested_type_path, method_name) = nested_reference.rsplit_once('.')?;
    if local_type_name.is_empty()
        || nested_type_path.is_empty()
        || method_name.is_empty()
        || nested_type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return None;
    }
    let binding = context
        .type_alias_bindings
        .get(&(None, local_type_name.to_string()))?
        .clone();
    Some((
        nested_type_path.replace('.', "::"),
        method_name.to_string(),
        binding,
    ))
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_base_type_alias(
    local_type_name: &str,
    context: &CSharpGlobalImportContext,
) -> Option<CSharpTypeAliasBinding> {
    if local_type_name.is_empty() {
        return None;
    }
    context
        .type_alias_bindings
        .get(&(None, local_type_name.to_string()))
        .cloned()
}

pub(in crate::symbol_dependency) fn csharp_global_base_type_alias_is_ambiguous(
    local_type_name: &str,
    context: &CSharpGlobalImportContext,
) -> bool {
    !local_type_name.is_empty()
        && context
            .ambiguous_type_alias_names
            .contains(&(None, local_type_name.to_string()))
}

pub(in crate::symbol_dependency) fn csharp_global_base_namespace_import_paths(
    context: &CSharpGlobalImportContext,
) -> Vec<String> {
    context
        .namespace_import_bindings
        .iter()
        .map(|binding| binding.semantic_namespace_path.clone())
        .collect()
}

pub(in crate::symbol_dependency) fn csharp_global_type_alias_name_is_ambiguous(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> bool {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return false;
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return false;
    }
    context
        .ambiguous_type_alias_names
        .contains(&(None, local_type_name.to_string()))
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_static_type_imports_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Vec<CSharpStaticTypeImportBinding> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Vec::new();
    }
    context.static_type_import_bindings.clone()
}

fn csharp_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpImportContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::CSharp) {
        return Ok(CSharpImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading C# import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing C# import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing C# import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(CSharpImportContext::default());
    }

    let mut type_alias_bindings = BTreeMap::new();
    let mut ambiguous_alias_names = BTreeSet::new();
    let mut base_type_bindings_by_range = BTreeMap::new();
    let mut interface_parent_bindings_by_range: BTreeMap<
        (usize, usize),
        Vec<CSharpInterfaceParentBinding>,
    > = BTreeMap::new();
    for import in csharp_file_type_alias_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# type alias bindings")?;
        }
        insert_unique_csharp_type_alias_binding(
            &mut type_alias_bindings,
            &mut ambiguous_alias_names,
            import.scope_path,
            import.local_name,
            CSharpTypeAliasBinding {
                semantic_type_path: import.semantic_type_path,
            },
        );
    }
    for base_type in csharp_file_base_types(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# base type bindings")?;
        }
        if base_type_bindings_by_range
            .insert(
                base_type.type_range,
                CSharpBaseTypeBinding {
                    semantic_type_path: base_type.semantic_base_type_path,
                    is_global_qualified: base_type.is_global_qualified,
                    alias_name: None,
                    namespace_import_paths: Vec::new(),
                },
            )
            .is_some()
        {
            return Ok(CSharpImportContext::default());
        }
    }
    for interface_parent in csharp_file_interface_parents(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# interface parent bindings")?;
        }
        interface_parent_bindings_by_range
            .entry(interface_parent.declaration_range)
            .or_default()
            .push(CSharpInterfaceParentBinding {
                semantic_type_path: interface_parent.semantic_type_path,
                is_global_qualified: interface_parent.is_global_qualified,
            });
    }
    let mut static_type_import_bindings = Vec::new();
    for import in csharp_file_static_type_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# static type import bindings")?;
        }
        static_type_import_bindings.push(CSharpStaticTypeImportBinding {
            scope_path: import.scope_path,
            semantic_type_path: import.semantic_type_path,
        });
    }
    let mut namespace_import_bindings = Vec::new();
    for import in csharp_file_namespace_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# namespace import bindings")?;
        }
        namespace_import_bindings.push(CSharpNamespaceImportBinding {
            scope_path: import.scope_path,
            semantic_namespace_path: import.semantic_namespace_path,
        });
    }
    let mut receiver_type_bindings_by_range = BTreeMap::new();
    collect_csharp_receiver_type_bindings(root, &source, &mut receiver_type_bindings_by_range)?;
    Ok(CSharpImportContext {
        type_alias_bindings,
        ambiguous_type_alias_names: ambiguous_alias_names,
        base_type_bindings_by_range,
        interface_parent_bindings_by_range,
        receiver_type_bindings_by_range,
        static_type_import_bindings,
        namespace_import_bindings,
    })
}

fn collect_csharp_receiver_type_bindings(
    node: tree_sitter::Node<'_>,
    source: &str,
    bindings_by_range: &mut BTreeMap<(usize, usize), CSharpReceiverTypeBindings>,
) -> Result<()> {
    if matches!(
        node.kind(),
        "method_declaration" | "constructor_declaration"
    ) && is_csharp_symbol_node(node)
    {
        bindings_by_range.insert(
            (node.start_byte(), node.end_byte()),
            csharp_receiver_type_bindings_for_node(node, source)?,
        );
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_csharp_receiver_type_bindings(child, source, bindings_by_range)?;
    }
    Ok(())
}

/// Collects locally bound receiver names with their declared type spellings
/// for a function. Parameters, typed locals, and enclosing-type fields and
/// properties carry their declared type; `var` locals, lambda parameters,
/// `foreach` variables, local functions, and type parameters are bound with an
/// empty type so they still suppress static type interpretation while failing
/// closed for instance dispatch. This mirrors the extractor's binding rules so
/// the resolver classifies `receiver.method(...)` facts the same way the
/// extractor recorded them.
fn csharp_receiver_type_bindings_for_node(
    function: tree_sitter::Node<'_>,
    source: &str,
) -> Result<CSharpReceiverTypeBindings> {
    let mut bindings = CSharpReceiverTypeBindings::default();
    collect_csharp_enclosing_type_bindings(function, source, &mut bindings)?;
    collect_csharp_function_bindings(function, source, &mut bindings)?;
    Ok(bindings)
}

fn collect_csharp_enclosing_type_bindings(
    function: tree_sitter::Node<'_>,
    source: &str,
    bindings: &mut CSharpReceiverTypeBindings,
) -> Result<()> {
    fn collect(
        node: tree_sitter::Node<'_>,
        root: tree_sitter::Node<'_>,
        source: &str,
        bindings: &mut CSharpReceiverTypeBindings,
    ) -> Result<()> {
        if node != root && is_csharp_symbol_node(node) {
            return Ok(());
        }
        if node.kind() == "field_declaration" {
            let mut declaration_cursor = node.walk();
            for declaration in node
                .named_children(&mut declaration_cursor)
                .filter(|child| child.kind() == "variable_declaration")
            {
                let type_name = csharp_declared_type_name(declaration, source)?;
                let mut declarator_cursor = declaration.walk();
                for declarator in declaration
                    .named_children(&mut declarator_cursor)
                    .filter(|child| child.kind() == "variable_declarator")
                {
                    let Some(name) = declarator.child_by_field_name("name") else {
                        continue;
                    };
                    let name = node_text(name, source)?.trim();
                    csharp_insert_receiver_binding(bindings, name, type_name.clone());
                }
            }
        }
        if matches!(node.kind(), "property_declaration" | "event_declaration")
            && let Some(name) = node.child_by_field_name("name")
        {
            let name = node_text(name, source)?.trim();
            csharp_insert_receiver_binding(
                bindings,
                name,
                csharp_declared_type_name(node, source)?,
            );
        }
        if node.kind() == "type_parameter"
            && let Some(name) = node.child_by_field_name("name")
        {
            let name = node_text(name, source)?.trim();
            csharp_insert_receiver_binding(bindings, name, None);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, root, source, bindings)?;
        }
        Ok(())
    }

    let mut current = function.parent();
    while let Some(node) = current {
        if is_csharp_type_declaration(node) {
            return collect(node, node, source, bindings);
        }
        current = node.parent();
    }
    Ok(())
}

fn collect_csharp_function_bindings(
    function: tree_sitter::Node<'_>,
    source: &str,
    bindings: &mut CSharpReceiverTypeBindings,
) -> Result<()> {
    fn declarator_name(node: tree_sitter::Node<'_>, source: &str) -> Result<Option<String>> {
        let name = if let Some(name) = node.child_by_field_name("name") {
            name
        } else {
            let mut cursor = node.walk();
            let Some(declarator) = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "variable_declarator")
            else {
                return Ok(None);
            };
            let Some(name) = declarator.child_by_field_name("name") else {
                return Ok(None);
            };
            name
        };
        let name = node_text(name, source)?.trim();
        Ok((!name.is_empty()).then(|| name.to_string()))
    }

    fn collect(
        node: tree_sitter::Node<'_>,
        source: &str,
        bindings: &mut CSharpReceiverTypeBindings,
    ) -> Result<()> {
        if is_csharp_symbol_node(node) && node.parent().is_some() {
            return Ok(());
        }
        if node.kind() == "implicit_parameter" {
            let name = node_text(node, source)?.trim();
            csharp_insert_receiver_binding(bindings, name, None);
        }
        if matches!(
            node.kind(),
            "parameter" | "catch_declaration" | "declaration_expression" | "declaration_pattern"
        ) && let Some(name) = declarator_name(node, source)?
        {
            csharp_insert_receiver_binding(
                bindings,
                &name,
                csharp_declared_type_name(node, source)?,
            );
        }
        if node.kind() == "local_function_statement"
            && let Some(name) = declarator_name(node, source)?
        {
            csharp_insert_receiver_binding(bindings, &name, None);
        }
        if node.kind() == "variable_declarator"
            && let Some(name) = node.child_by_field_name("name")
        {
            let name = node_text(name, source)?.trim();
            let type_name = match node.parent() {
                Some(parent) if parent.kind() == "variable_declaration" => {
                    match csharp_declared_type_name(parent, source)? {
                        Some(type_name) => Some(type_name),
                        // A `var` local infers its receiver type from a
                        // constructor initializer such as
                        // `var helper = new Helper()`; other initializers bind
                        // an empty type and fail closed.
                        None => csharp_constructor_type_from_declarator(node, source)?,
                    }
                }
                _ => None,
            };
            csharp_insert_receiver_binding(bindings, name, type_name);
        }
        if node.kind() == "foreach_statement"
            && let Some(left) = node.child_by_field_name("left")
            && left.kind() == "identifier"
        {
            let name = node_text(left, source)?.trim();
            csharp_insert_receiver_binding(bindings, name, None);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, source, bindings)?;
        }
        Ok(())
    }

    let mut cursor = function.walk();
    for child in function.named_children(&mut cursor) {
        collect(child, source, bindings)?;
    }
    Ok(())
}

/// Infers a receiver type for `var` locals whose initializer is a constructor
/// call such as `var helper = new Helper()` or `var helper = new Outer.Inner()`.
/// Non-constructor initializers, target-typed creations, array creations, and
/// malformed type spellings return `None` and fail closed. Mirrors the
/// extractor's `var` binding rules so the resolver classifies constructor
/// initializers the same way the extractor recorded them.
fn csharp_constructor_type_from_declarator(
    declarator: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let Some(initializer) = csharp_declarator_initializer(declarator) else {
        return Ok(None);
    };
    if initializer.kind() != "object_creation_expression" {
        return Ok(None);
    }
    let Some(type_node) = initializer.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = node_text(type_node, source)?.trim();
    if type_name.is_empty() || type_name == "var" {
        return Ok(None);
    }
    Ok(Some(type_name.to_string()))
}

/// Returns the initializer expression of a `variable_declarator` such as
/// `helper = new Helper()`. The grammar does not name the `= expression` child,
/// so the last named child that is not the declared name or a tuple/indexer
/// suffix is the initializer.
fn csharp_declarator_initializer<'a>(
    declarator: tree_sitter::Node<'a>,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = declarator.walk();
    let mut initializer = None;
    for child in declarator.named_children(&mut cursor) {
        if !matches!(
            child.kind(),
            "identifier" | "tuple_pattern" | "bracketed_argument_list"
        ) {
            initializer = Some(child);
        }
    }
    initializer
}

fn csharp_insert_receiver_binding(
    bindings: &mut CSharpReceiverTypeBindings,
    name: &str,
    type_name: Option<String>,
) {
    if !name.is_empty() {
        bindings
            .types_by_name
            .insert(name.to_string(), type_name.unwrap_or_default());
    }
}

fn csharp_declared_type_name(node: tree_sitter::Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(type_node) = node.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = node_text(type_node, source)?.trim();
    if type_name.is_empty() || type_name == "var" {
        return Ok(None);
    }
    Ok(Some(type_name.to_string()))
}

fn is_csharp_type_declaration(node: tree_sitter::Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
    )
}

fn insert_unique_csharp_type_alias_binding(
    bindings: &mut BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_names: &mut BTreeSet<(Option<String>, String)>,
    scope_path: Option<String>,
    local_name: String,
    binding: CSharpTypeAliasBinding,
) {
    let key = (scope_path, local_name);
    if ambiguous_names.contains(&key) {
        return;
    }
    if bindings.insert(key.clone(), binding).is_some() {
        bindings.remove(&key);
        ambiguous_names.insert(key);
    }
}

pub(in crate::symbol_dependency) fn resolve_csharp_base_type_binding_for_reference(
    source_file_path: &str,
    source_type_range: (usize, usize),
    source_namespace_path: Option<&str>,
    global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(binding) = context
        .base_type_bindings_by_range
        .get(&source_type_range)
        .cloned()
    else {
        return Ok(None);
    };
    Ok(resolve_csharp_base_type_binding_parts(
        &context,
        global_import_context,
        binding,
        source_namespace_path,
    ))
}

/// Resolves the direct parent interfaces of an interface declaration as
/// base-type bindings in the interface's file scope (namespace imports and
/// aliases applied). A missing `base_list` yields `None`; a parent spelling
/// that fails to normalize or resolve in file scope yields `Blocked` so the
/// resolver fails closed instead of guessing.
pub(in crate::symbol_dependency) fn csharp_interface_parent_bindings_for_interface(
    source_file_path: &str,
    interface_range: (usize, usize),
    source_namespace_path: Option<&str>,
    global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpInterfaceParents> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(parents) = context
        .interface_parent_bindings_by_range
        .get(&interface_range)
    else {
        return Ok(CSharpInterfaceParents::None);
    };
    if parents.is_empty() {
        return Ok(CSharpInterfaceParents::Blocked);
    }
    let mut resolved_parents = Vec::with_capacity(parents.len());
    for parent in parents {
        let Some(binding) = resolve_csharp_base_type_binding_parts(
            &context,
            global_import_context,
            CSharpBaseTypeBinding {
                semantic_type_path: parent.semantic_type_path.clone(),
                is_global_qualified: parent.is_global_qualified,
                alias_name: None,
                namespace_import_paths: Vec::new(),
            },
            source_namespace_path,
        ) else {
            return Ok(CSharpInterfaceParents::Blocked);
        };
        resolved_parents.push(binding);
    }
    Ok(CSharpInterfaceParents::Parents(resolved_parents))
}

/// Resolves a declared type spelling written in a caller (such as a receiver
/// parameter or field type) to a base-type binding. Simple names resolve
/// through the caller's namespace and import scopes, aliases expand to their
/// targets, and `global::`-qualified names bind directly. Malformed or
/// alias-colliding spellings return `None`.
pub(in crate::symbol_dependency) fn resolve_csharp_declared_type_binding_for_reference(
    source_file_path: &str,
    type_name: &str,
    source_namespace_path: Option<&str>,
    global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let type_name = type_name.trim();
    if type_name.is_empty() {
        return Ok(None);
    }
    let is_global_qualified = type_name.starts_with("global::");
    let Some(semantic_type_path) = csharp_generic_type_semantic_path(type_name) else {
        return Ok(None);
    };
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(resolve_csharp_base_type_binding_parts(
        &context,
        global_import_context,
        CSharpBaseTypeBinding {
            semantic_type_path,
            is_global_qualified,
            alias_name: None,
            namespace_import_paths: Vec::new(),
        },
        source_namespace_path,
    ))
}

pub(in crate::symbol_dependency) fn csharp_receiver_type_bindings_for_function(
    source_file_path: &str,
    function_range: (usize, usize),
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpReceiverTypeBindings>> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(context
        .receiver_type_bindings_by_range
        .get(&function_range)
        .cloned())
}

fn resolve_csharp_base_type_binding_parts(
    context: &CSharpImportContext,
    global_import_context: Option<&CSharpGlobalImportContext>,
    mut binding: CSharpBaseTypeBinding,
    source_namespace_path: Option<&str>,
) -> Option<CSharpBaseTypeBinding> {
    if !binding.is_global_qualified && binding.semantic_type_path.contains("::") {
        let first_segment = binding.semantic_type_path.split("::").next()?;
        for scope_path in csharp_import_scope_paths(source_namespace_path) {
            let key = (scope_path, first_segment.to_string());
            if context.ambiguous_type_alias_names.contains(&key)
                || context.type_alias_bindings.contains_key(&key)
            {
                return None;
            }
        }
        if let Some(global_import_context) = global_import_context
            && (csharp_global_base_type_alias_is_ambiguous(first_segment, global_import_context)
                || resolve_csharp_global_base_type_alias(first_segment, global_import_context)
                    .is_some())
        {
            return None;
        }
    } else if !binding.is_global_qualified {
        let local_name = binding.semantic_type_path.clone();
        let scope_paths = csharp_import_scope_paths(source_namespace_path);
        for scope_path in &scope_paths {
            let key = (scope_path.clone(), local_name.clone());
            if context.ambiguous_type_alias_names.contains(&key) {
                return None;
            }
            if let Some(alias) = context.type_alias_bindings.get(&key) {
                binding.semantic_type_path = alias.semantic_type_path.clone();
                binding.is_global_qualified = true;
                binding.alias_name = Some(local_name.clone());
                break;
            }
        }
        if binding.alias_name.is_none() {
            binding.namespace_import_paths = scope_paths
                .into_iter()
                .flat_map(|scope_path| {
                    context
                        .namespace_import_bindings
                        .iter()
                        .filter(move |candidate| candidate.scope_path == scope_path)
                        .map(|candidate| candidate.semantic_namespace_path.clone())
                })
                .collect();
            if let Some(global_import_context) = global_import_context {
                if csharp_global_base_type_alias_is_ambiguous(&local_name, global_import_context) {
                    return None;
                }
                if let Some(alias) =
                    resolve_csharp_global_base_type_alias(&local_name, global_import_context)
                {
                    binding.semantic_type_path = alias.semantic_type_path;
                    binding.is_global_qualified = true;
                    binding.alias_name = Some(local_name.clone());
                    binding.namespace_import_paths.clear();
                } else {
                    binding.namespace_import_paths.extend(
                        csharp_global_base_namespace_import_paths(global_import_context),
                    );
                }
            }
        }
    }
    Some(binding)
}

pub(in crate::symbol_dependency) fn resolve_csharp_type_alias_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, CSharpTypeAliasBinding)>> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(None);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    for scope_path in csharp_import_scope_paths(source_namespace_path) {
        let key = (scope_path, local_type_name.to_string());
        if context.ambiguous_type_alias_names.contains(&key) {
            return Ok(None);
        }
        if let Some(binding) = context.type_alias_bindings.get(&key) {
            return Ok(Some((method_name.to_string(), binding.clone())));
        }
    }
    Ok(None)
}

pub(in crate::symbol_dependency) fn resolve_csharp_nested_type_alias_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, String, CSharpTypeAliasBinding)>> {
    let Some((local_type_name, nested_reference)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    let Some((nested_type_path, method_name)) = nested_reference.rsplit_once('.') else {
        return Ok(None);
    };
    if local_type_name.is_empty()
        || nested_type_path.is_empty()
        || method_name.is_empty()
        || nested_type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return Ok(None);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    for scope_path in csharp_import_scope_paths(source_namespace_path) {
        let key = (scope_path, local_type_name.to_string());
        if context.ambiguous_type_alias_names.contains(&key) {
            return Ok(None);
        }
        if let Some(binding) = context.type_alias_bindings.get(&key) {
            return Ok(Some((
                nested_type_path.replace('.', "::"),
                method_name.to_string(),
                binding.clone(),
            )));
        }
    }
    Ok(None)
}

pub(in crate::symbol_dependency) fn csharp_type_alias_name_is_declared_for_reference(
    source_file_path: &str,
    local_type_name: &str,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<bool> {
    if local_type_name.is_empty() {
        return Ok(false);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    if csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .map(|scope_path| (scope_path, local_type_name.to_string()))
        .any(|key| {
            context.type_alias_bindings.contains_key(&key)
                || context.ambiguous_type_alias_names.contains(&key)
        })
    {
        return Ok(true);
    }

    Ok(csharp_global_import_context.is_some_and(|context| {
        let key = (None, local_type_name.to_string());
        context.type_alias_bindings.contains_key(&key)
            || context.ambiguous_type_alias_names.contains(&key)
    }))
}

pub(in crate::symbol_dependency) fn csharp_type_alias_name_is_ambiguous_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<bool> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(false);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(false);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .map(|scope_path| (scope_path, local_type_name.to_string()))
        .any(|key| context.ambiguous_type_alias_names.contains(&key)))
}

fn is_safe_csharp_identifier(identifier: &str) -> bool {
    let mut characters = identifier.chars();
    matches!(characters.next(), Some(character) if character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn csharp_import_scope_paths(source_namespace_path: Option<&str>) -> Vec<Option<String>> {
    let mut scope_paths = Vec::new();
    let mut current_scope_path = source_namespace_path;
    while let Some(scope_path) = current_scope_path {
        scope_paths.push(Some(scope_path.to_string()));
        current_scope_path = scope_path
            .rsplit_once("::")
            .map(|(parent_path, _)| parent_path);
    }
    scope_paths.push(None);
    scope_paths
}

pub(in crate::symbol_dependency) fn resolve_csharp_static_type_imports_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<CSharpStaticTypeImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(Vec::new());
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .flat_map(|scope_path| {
            context
                .static_type_import_bindings
                .iter()
                .filter(move |binding| binding.scope_path == scope_path)
                .cloned()
        })
        .collect())
}

pub(in crate::symbol_dependency) fn resolve_csharp_namespace_imports_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<CSharpNamespaceImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(Vec::new());
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .flat_map(|scope_path| {
            context
                .namespace_import_bindings
                .iter()
                .filter(move |binding| binding.scope_path == scope_path)
                .cloned()
        })
        .collect())
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_namespace_imports_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Vec<CSharpNamespaceImportBinding> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Vec::new();
    }
    context.namespace_import_bindings.clone()
}

fn csharp_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = csharp_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}
