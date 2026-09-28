use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};
use crate::deadline::DeadlineCheck;

pub(crate) fn shell_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    shell_local_file_dependency_paths_with_deadline(path, root, source, None)
}

pub(crate) fn shell_local_file_dependency_paths_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    let mut dependencies = BTreeSet::new();
    collect_shell_file_dependencies(path, root, source, &mut dependencies, deadline)?;
    if let Ok(normalized) = normalize_absolute_path(path) {
        dependencies.remove(&normalized);
    }
    Ok(dependencies)
}

fn check_shell_dependency_deadline(deadline: Option<&dyn DeadlineCheck>) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting local file dependencies")?;
    }
    Ok(())
}

fn collect_shell_file_dependencies(
    path: &Path,
    node: Node<'_>,
    source: &str,
    dependencies: &mut BTreeSet<PathBuf>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    check_shell_dependency_deadline(deadline)?;
    if node.kind() == "command"
        && let Some(command) = shell_command_name(node, source)?
        && (command == "." || command == "source")
        && let Some(specifier) = shell_source_specifier(node, source)?
        && let Some(candidate) = resolve_shell_dependency_path(path, &specifier)
    {
        dependencies.insert(candidate);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_shell_file_dependencies(path, child, source, dependencies, deadline)?;
    }
    Ok(())
}

fn shell_command_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(name_node) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    let literal = if name_node.kind() == "command_name" {
        match name_node.named_child(0) {
            Some(child) => child,
            None => return Ok(None),
        }
    } else {
        name_node
    };
    if literal.kind() != "word" {
        return Ok(None);
    }
    let name = node_text(literal, source)?.trim();
    if name.is_empty() {
        return Ok(None);
    }
    Ok(Some(name.to_string()))
}

fn shell_source_specifier(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(argument) = node.child_by_field_name("argument") else {
        return Ok(None);
    };
    if argument.kind() == "word" && argument.named_child_count() != 0 {
        return Ok(None);
    }
    let literal = node_text(argument, source)?.trim();
    let value = literal
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            literal
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(literal);
    if value.is_empty()
        || value
            .chars()
            .any(|character| matches!(character, '$' | '`' | '*' | '?' | '[' | ']' | '\\' | '\0'))
        || value.starts_with('-')
    {
        return Ok(None);
    }
    Ok(Some(value.to_string()))
}

fn resolve_shell_dependency_path(path: &Path, specifier: &str) -> Option<PathBuf> {
    let parent = path.parent()?;
    if specifier.starts_with('/') || specifier.starts_with('\\') {
        return None;
    }
    let candidates = if specifier.ends_with(".sh") || specifier.ends_with(".bash") {
        vec![specifier.to_string()]
    } else {
        vec![
            specifier.to_string(),
            format!("{specifier}.sh"),
            format!("{specifier}.bash"),
        ]
    };
    for candidate in candidates {
        if !is_safe_shell_path_component(&candidate) {
            continue;
        }
        let Ok(absolute) = normalize_absolute_path(&parent.join(&candidate)) else {
            continue;
        };
        if absolute.is_file()
            && matches!(
                absolute
                    .extension()
                    .and_then(|extension| extension.to_str()),
                Some("sh" | "bash")
            )
        {
            return Some(absolute);
        }
    }
    None
}

fn is_safe_shell_path_component(candidate: &str) -> bool {
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
    use std::fs;
    use std::path::Path;

    use anyhow::bail;

    use super::{
        normalize_absolute_path, shell_local_file_dependency_paths,
        shell_local_file_dependency_paths_with_deadline,
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
    fn shell_dependency_extraction_honors_deadline_during_tree_walk() {
        let source = ". ./helper.sh\nsource \"other.sh\"\n";
        let path = Path::new("sample.sh");
        let document = parse_document(path, source).expect("Shell source should parse");
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 2,
        };

        let error = shell_local_file_dependency_paths_with_deadline(
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
    fn resolves_literal_shell_source_dependencies() {
        let root = std::env::temp_dir().join(format!(
            "arborist-shell-dependencies-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.sh");
        let helper = root.join("helper.sh");
        let nested_dir = root.join("config");
        fs::create_dir_all(&nested_dir).unwrap();
        let nested = nested_dir.join("settings.bash");
        fs::write(&helper, "helper() { :; }\n").unwrap();
        fs::write(&nested, "settings() { :; }\n").unwrap();

        let source = ". ./helper.sh\nsource config/settings.bash\nsource \"missing.sh\"\nsource \"$DYNAMIC\"\n";
        fs::write(&caller, source).unwrap();
        let document = parse_document(&caller, source).unwrap();

        let dependencies =
            shell_local_file_dependency_paths(&caller, document.tree.root_node(), source).unwrap();

        assert_eq!(
            dependencies,
            std::collections::BTreeSet::from([
                normalize_absolute_path(&helper).unwrap(),
                normalize_absolute_path(&nested).unwrap()
            ])
        );
        let _ = fs::remove_dir_all(&root);
    }
}
