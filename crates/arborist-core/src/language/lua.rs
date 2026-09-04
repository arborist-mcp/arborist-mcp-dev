use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};
use crate::deadline::DeadlineCheck;

pub(crate) fn lua_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    lua_local_file_dependency_paths_with_deadline(path, root, source, None)
}

pub(crate) fn lua_local_file_dependency_paths_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    check_lua_dependency_deadline(deadline)?;
    let mut dependencies = BTreeSet::new();
    collect_lua_file_dependencies(path, root, source, &mut dependencies, deadline)?;
    if let Ok(normalized) = normalize_absolute_path(path) {
        dependencies.remove(&normalized);
    }
    Ok(dependencies)
}

fn check_lua_dependency_deadline(deadline: Option<&dyn DeadlineCheck>) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting local file dependencies")?;
    }
    Ok(())
}

fn collect_lua_file_dependencies(
    path: &Path,
    node: Node<'_>,
    source: &str,
    dependencies: &mut BTreeSet<PathBuf>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    check_lua_dependency_deadline(deadline)?;
    if matches!(node.kind(), "function_call")
        && let Some(specifier) = lua_function_call_string_argument(node, source, deadline)?
        && let Some(candidate) = resolve_lua_dependency_path(path, &specifier)
    {
        dependencies.insert(candidate);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_lua_file_dependencies(path, child, source, dependencies, deadline)?;
    }
    Ok(())
}

fn lua_function_call_string_argument(
    node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<String>> {
    check_lua_dependency_deadline(deadline)?;
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    if name_node.kind() != "identifier" {
        return Ok(None);
    }
    let function_name = node_text(name_node, source)?.trim();
    if !matches!(function_name, "require" | "dofile") {
        return Ok(None);
    }
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return Ok(None);
    };
    let mut cursor = arguments.walk();
    let Some(string_node) = arguments
        .named_children(&mut cursor)
        .find(|child| child.kind() == "string")
    else {
        return Ok(None);
    };
    lua_string_literal(string_node, source)
}

fn lua_string_literal(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let literal = node_text(node, source)?.trim();
    let value = literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            literal
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        });
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty() || value.contains('\\') {
        return Ok(None);
    }
    Ok(Some(value.to_string()))
}

fn resolve_lua_dependency_path(path: &Path, specifier: &str) -> Option<PathBuf> {
    let parent = path.parent()?;
    if specifier.contains('\0') {
        return None;
    }
    if specifier.starts_with('/') || specifier.starts_with('\\') {
        return None;
    }
    let candidates = if specifier.ends_with(".lua") {
        vec![specifier.to_string()]
    } else if specifier.contains('/') || specifier.contains('\\') {
        // Path-like require/dofile strings usually carry a real extension. For
        // conservative local-file support we accept the raw path and a `.lua`
        // fallback so missing extensions still resolve in common projects.
        vec![specifier.to_string(), format!("{specifier}.lua")]
    } else {
        // Lua `require("a.b")` denotes a module whose dotted name maps to a
        // `.lua` file under the current directory (`a/b.lua`).
        let module_path = specifier.replace('.', "/");
        vec![format!("{module_path}.lua")]
    };
    for candidate in candidates {
        if !is_safe_lua_path_component(&candidate) {
            continue;
        }
        let Ok(absolute) = normalize_absolute_path(&parent.join(candidate)) else {
            continue;
        };
        if absolute.is_file() {
            return Some(absolute);
        }
    }
    None
}

fn is_safe_lua_path_component(candidate: &str) -> bool {
    use std::path::Component;
    !candidate.is_empty()
        && Path::new(candidate).components().all(|component| {
            matches!(
                component,
                Component::Normal(_) | Component::CurDir | Component::ParentDir
            )
        })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use anyhow::bail;

    use super::{
        lua_local_file_dependency_paths, lua_local_file_dependency_paths_with_deadline,
        normalize_absolute_path,
    };
    use crate::deadline::DeadlineCheck;
    use crate::language::parse_document;

    struct RejectAfterChecks {
        checks: Cell<usize>,
        reject_after: usize,
    }

    impl DeadlineCheck for RejectAfterChecks {
        fn check(&self, phase: &str) -> anyhow::Result<()> {
            assert_eq!(phase, "extracting local file dependencies");
            let checks = self.checks.get();
            self.checks.set(checks + 1);
            if checks >= self.reject_after {
                bail!("deadline expired");
            }
            Ok(())
        }
    }

    #[test]
    fn lua_dependency_extraction_honors_deadline_during_tree_walk() {
        let source = "local a = require(\"./helper.lua\")\n";
        let path = Path::new("sample.lua");
        let document = parse_document(path, source).expect("Lua source should parse");
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 2,
        };

        let error = lua_local_file_dependency_paths_with_deadline(
            path,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("dependency tree walk should honor the deadline");

        assert_eq!(error.to_string(), "deadline expired");
        assert!(deadline.checks.get() >= 3);
    }

    #[test]
    fn resolves_lua_require_and_dofile_dependencies() {
        let root =
            std::env::temp_dir().join(format!("arborist-lua-dependencies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.lua");
        let helper = root.join("helper.lua");
        let nested_dir = root.join("config");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let nested = nested_dir.join("settings.lua");
        std::fs::write(&helper, "return 1\n").unwrap();
        std::fs::write(&nested, "return 2\n").unwrap();

        let source = "local h = require(\"helper\")\nlocal n = require(\"config.settings\")\nlocal d = dofile(\"helper.lua\")\nlocal missing = require(\"missing\")\n";
        fs::write(&caller, source).unwrap();
        let document = parse_document(&caller, source).unwrap();

        let dependencies =
            lua_local_file_dependency_paths(&caller, document.tree.root_node(), source).unwrap();

        assert_eq!(
            dependencies,
            BTreeSet::from([
                normalize_absolute_path(&helper).unwrap(),
                normalize_absolute_path(&nested).unwrap()
            ])
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
