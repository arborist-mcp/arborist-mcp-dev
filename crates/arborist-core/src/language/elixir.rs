use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};
use crate::deadline::DeadlineCheck;

pub(crate) fn elixir_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    elixir_local_file_dependency_paths_with_deadline(path, root, source, None)
}

pub(crate) fn elixir_local_file_dependency_paths_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    check_elixir_dependency_deadline(deadline)?;
    let mut dependencies = BTreeSet::new();
    collect_elixir_file_dependencies(path, root, source, &mut dependencies, deadline)?;
    if let Ok(normalized) = normalize_absolute_path(path) {
        dependencies.remove(&normalized);
    }
    Ok(dependencies)
}

fn check_elixir_dependency_deadline(deadline: Option<&dyn DeadlineCheck>) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting local file dependencies")?;
    }
    Ok(())
}

fn collect_elixir_file_dependencies(
    path: &Path,
    node: Node<'_>,
    source: &str,
    dependencies: &mut BTreeSet<PathBuf>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    check_elixir_dependency_deadline(deadline)?;
    if node.kind() == "call"
        && let Some(specifier) = elixir_file_call_specifier(node, source)?
        && let Some(candidate) = resolve_elixir_dependency_path(path, &specifier)
    {
        dependencies.insert(candidate);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_elixir_file_dependencies(path, child, source, dependencies, deadline)?;
    }
    Ok(())
}

fn elixir_file_call_specifier(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(target) = node.child_by_field_name("target") else {
        return Ok(None);
    };
    let function_name = match target.kind() {
        "identifier" => node_text(target, source)?.trim().to_owned(),
        "dot" => {
            let Some(left) = target.child_by_field_name("left") else {
                return Ok(None);
            };
            let Some(right) = target.child_by_field_name("right") else {
                return Ok(None);
            };
            if left.kind() != "alias" || right.kind() != "identifier" {
                return Ok(None);
            }
            if node_text(left, source)?.trim() != "Code" {
                return Ok(None);
            }
            format!("Code.{}", node_text(right, source)?.trim())
        }
        _ => return Ok(None),
    };
    if !matches!(
        function_name.as_str(),
        "require_file" | "Code.require_file" | "Code.compile_file"
    ) {
        return Ok(None);
    }
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return Ok(None);
    };
    let mut cursor = arguments.walk();
    let Some(first_argument) = arguments.named_children(&mut cursor).next() else {
        return Ok(None);
    };
    elixir_string_literal(first_argument, source)
}

fn elixir_string_literal(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() != "string" {
        return Ok(None);
    }
    let literal = node_text(node, source)?.trim();
    let Some(value) = literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Ok(None);
    };
    if value.is_empty() || value.contains('\\') || value.contains('#') {
        return Ok(None);
    }
    Ok(Some(value.to_owned()))
}

fn resolve_elixir_dependency_path(path: &Path, specifier: &str) -> Option<PathBuf> {
    if specifier.contains('\0')
        || specifier.starts_with('/')
        || specifier.starts_with('\\')
        || !matches!(
            Path::new(specifier)
                .extension()
                .and_then(|ext| ext.to_str()),
            Some("ex" | "exs")
        )
        || !is_safe_elixir_path(specifier)
    {
        return None;
    }
    let parent = path.parent()?;
    let absolute = normalize_absolute_path(&parent.join(specifier)).ok()?;
    absolute.is_file().then_some(absolute)
}

fn is_safe_elixir_path(specifier: &str) -> bool {
    use std::path::Component;
    Path::new(specifier).components().all(|component| {
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
    use std::path::Path;

    use anyhow::bail;

    use super::{
        elixir_local_file_dependency_paths, elixir_local_file_dependency_paths_with_deadline,
        normalize_absolute_path,
    };
    use crate::deadline::DeadlineCheck;
    use crate::language::parse_document;

    struct RejectAfterChecks {
        checks: Cell<usize>,
    }

    impl DeadlineCheck for RejectAfterChecks {
        fn check(&self, phase: &str) -> anyhow::Result<()> {
            assert_eq!(phase, "extracting local file dependencies");
            let checks = self.checks.get();
            self.checks.set(checks + 1);
            if checks >= 2 {
                bail!("deadline expired");
            }
            Ok(())
        }
    }

    #[test]
    fn resolves_static_local_file_calls_only() {
        let root = std::env::temp_dir().join(format!(
            "arborist-elixir-dependencies-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.ex");
        let helper = root.join("helper.ex");
        let script = root.join("script.exs");
        std::fs::write(&helper, "defmodule Helper do\nend\n").unwrap();
        std::fs::write(&script, "IO.puts(1)\n").unwrap();
        let source = r#"
Code.require_file("helper.ex")
Code.compile_file("script.exs", [])
require_file("helper.ex")
Code.require_file(path)
Other.require_file("helper.ex")
"#;
        let document = parse_document(&caller, source).unwrap();
        let dependencies =
            elixir_local_file_dependency_paths(&caller, document.tree.root_node(), source).unwrap();
        assert_eq!(
            dependencies,
            BTreeSet::from([
                normalize_absolute_path(&helper).unwrap(),
                normalize_absolute_path(&script).unwrap()
            ])
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn honors_deadline_during_tree_walk() {
        let source = "Code.require_file(\"helper.ex\")\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
        };
        let error = elixir_local_file_dependency_paths_with_deadline(
            path,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("dependency walk should honor deadline");
        assert_eq!(error.to_string(), "deadline expired");
    }
}
