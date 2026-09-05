use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};
use crate::deadline::DeadlineCheck;

pub(crate) fn php_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    php_local_file_dependency_paths_with_deadline(path, root, source, None)
}

pub(crate) fn php_local_file_dependency_paths_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    check_php_dependency_deadline(deadline)?;
    let mut dependencies = BTreeSet::new();
    collect_php_file_dependencies(path, root, source, &mut dependencies, deadline)?;
    if let Ok(normalized) = normalize_absolute_path(path) {
        dependencies.remove(&normalized);
    }
    Ok(dependencies)
}

fn check_php_dependency_deadline(deadline: Option<&dyn DeadlineCheck>) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting local file dependencies")?;
    }
    Ok(())
}

fn collect_php_file_dependencies(
    path: &Path,
    node: Node<'_>,
    source: &str,
    dependencies: &mut BTreeSet<PathBuf>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    check_php_dependency_deadline(deadline)?;
    if matches!(
        node.kind(),
        "include_expression"
            | "include_once_expression"
            | "require_expression"
            | "require_once_expression"
    ) && let Some(specifier) = php_include_specifier(node, source)?
        && let Some(candidate) = resolve_php_dependency_path(path, &specifier)
    {
        dependencies.insert(candidate);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_php_file_dependencies(path, child, source, dependencies, deadline)?;
    }
    Ok(())
}

fn php_include_specifier(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut cursor = node.walk();
    let Some(string_node) = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "string")
    else {
        return Ok(None);
    };
    php_string_literal(string_node, source)
}

fn php_string_literal(node: Node<'_>, source: &str) -> Result<Option<String>> {
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
    if value.is_empty() {
        return Ok(None);
    }
    Ok(Some(value.to_string()))
}

fn resolve_php_dependency_path(path: &Path, specifier: &str) -> Option<PathBuf> {
    let parent = path.parent()?;
    if specifier.contains('\0') {
        return None;
    }
    if specifier.starts_with('/') || specifier.starts_with('\\') {
        return None;
    }
    let mut candidates = Vec::new();
    if !specifier.ends_with(".php") {
        candidates.push(specifier.to_string());
        candidates.push(format!("{specifier}.php"));
    } else {
        candidates.push(specifier.to_string());
    }
    for candidate in candidates {
        if !is_safe_php_path_component(&candidate) {
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

fn is_safe_php_path_component(candidate: &str) -> bool {
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
    use std::path::Path;

    use anyhow::bail;
    use std::fs;

    use super::{
        normalize_absolute_path, php_local_file_dependency_paths,
        php_local_file_dependency_paths_with_deadline,
    };
    use crate::deadline::DeadlineCheck;
    use crate::language::parse_document;

    struct RejectAfterChecks {
        checks: std::cell::Cell<usize>,
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
    fn php_dependency_extraction_honors_deadline_during_tree_walk() {
        let source = "<?php\ninclude 'helper.php';\n";
        let path = Path::new("sample.php");
        let document = parse_document(path, source).expect("PHP source should parse");
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 2,
        };

        let error = php_local_file_dependency_paths_with_deadline(
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
    fn resolves_php_include_and_require_dependencies() {
        let root =
            std::env::temp_dir().join(format!("arborist-php-dependencies-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let caller = root.join("caller.php");
        let helper = root.join("helper.php");
        let config = root.join("config.php");
        std::fs::write(&helper, "<?php return 1;\n").unwrap();
        std::fs::write(&config, "<?php return 2;\n").unwrap();

        let source =
            "<?php\ninclude 'helper.php';\nrequire_once './config.php';\nrequire 'missing.php';\n";
        fs::write(&caller, source).unwrap();
        let document = parse_document(&caller, source).unwrap();

        let dependencies =
            php_local_file_dependency_paths(&caller, document.tree.root_node(), source).unwrap();

        assert_eq!(
            dependencies,
            BTreeSet::from([
                normalize_absolute_path(&helper).unwrap(),
                normalize_absolute_path(&config).unwrap()
            ])
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
