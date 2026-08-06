use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path, parse_document};

pub(crate) fn kotlin_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let normalized_path = normalize_absolute_path(path)?;
    let mut dependencies = BTreeSet::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "import" {
            continue;
        }
        let Some(import_path) = kotlin_explicit_import_path(node, source)? else {
            continue;
        };
        let Some(source_path) = resolve_unique_kotlin_source_path(path, &import_path) else {
            continue;
        };
        if source_path == normalized_path {
            continue;
        }
        dependencies.insert(source_path);
    }
    Ok(dependencies)
}

fn kotlin_explicit_import_path(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let text = node_text(node, source)?.trim();
    if text.contains('*') {
        return Ok(None);
    }
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    let Some(identifier) = children
        .iter()
        .find(|child| child.kind() == "qualified_identifier")
    else {
        return Ok(None);
    };
    let import_path = node_text(*identifier, source)?.trim().to_string();
    if import_path.is_empty() || !is_safe_kotlin_qualified_name(&import_path) {
        return Ok(None);
    }
    Ok(Some(import_path))
}

fn resolve_unique_kotlin_source_path(path: &Path, import_path: &str) -> Option<PathBuf> {
    let segments = import_path.split('.').collect::<Vec<_>>();
    let mut candidates = BTreeSet::new();
    let mut source_root = path.parent()?.to_path_buf();
    loop {
        let mut candidate = source_root.clone();
        for segment in &segments {
            candidate.push(segment);
        }
        candidate.set_extension("kt");
        if candidate.is_file() && candidate_declares_import_package(&candidate, import_path) {
            candidates.insert(normalize_absolute_path(&candidate).ok()?);
        }

        if !source_root.pop() {
            break;
        }
    }
    (candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten()
}

fn candidate_declares_import_package(candidate: &Path, import_path: &str) -> bool {
    let Some((expected_package, _)) = import_path.rsplit_once('.') else {
        return false;
    };
    let Ok(source) = fs::read_to_string(candidate) else {
        return false;
    };
    let Ok(document) = parse_document(candidate, &source) else {
        return false;
    };
    let mut cursor = document.tree.root_node().walk();
    let Some(package) = document
        .tree
        .root_node()
        .named_children(&mut cursor)
        .find(|node| node.kind() == "package_header")
    else {
        return false;
    };
    let mut cursor = package.walk();
    let Some(name) = package
        .named_children(&mut cursor)
        .find(|node| node.kind() == "qualified_identifier")
    else {
        return false;
    };
    node_text(name, &source)
        .map(|name| name.trim() == expected_package)
        .unwrap_or(false)
}

fn is_safe_kotlin_qualified_name(name: &str) -> bool {
    name.split('.').all(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(['/', '\\'])
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::kotlin_local_file_dependency_paths;
    use crate::language::parse_document;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn resolves_unique_explicit_imports_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.kt");
        let helper_path = root.join("src/com/example/Helper.kt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.example\n\nimport com.example.Helper\n\nclass Main\n",
        )
        .unwrap();
        fs::write(&helper_path, "package com.example\n\nclass Helper\n").unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            kotlin_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [helper_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_unique_imports_across_package_directories() {
        let root = temporary_dir();
        let source_path = root.join("src/com/child/Main.kt");
        let base_path = root.join("src/com/base/Base.kt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(base_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.child\n\nimport com.base.Base\n\nclass Main\n",
        )
        .unwrap();
        fs::write(&base_path, "package com.base\n\nclass Base\n").unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            kotlin_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [base_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_alias_imports_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.kt");
        let helper_path = root.join("src/com/example/Helper.kt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.example\n\nimport com.example.Helper as H\n\nclass Main\n",
        )
        .unwrap();
        fs::write(&helper_path, "package com.example\n\nclass Helper\n").unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            kotlin_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [helper_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_top_level_function_imports_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.kt");
        let helper_path = root.join("src/com/example/utils/helper.kt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(helper_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.example\n\nimport com.example.utils.helper\n\nclass Main\n",
        )
        .unwrap();
        fs::write(
            &helper_path,
            "package com.example.utils\n\nfun helper(): Int = 1\n",
        )
        .unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            kotlin_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [helper_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ignores_wildcard_and_ambiguous_kotlin_imports() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.kt");
        let first_helper = root.join("src/com/example/Helper.kt");
        let second_helper = root.join("src/com/example/com/example/Helper.kt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(second_helper.parent().unwrap()).unwrap();
        fs::write(&first_helper, "package com.example\n\nclass Helper\n").unwrap();
        fs::write(&second_helper, "package com.example\n\nclass Helper\n").unwrap();
        let source = "package com.example\n\nimport com.example.*\nimport com.example.Helper\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            kotlin_local_file_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();

        assert!(dependencies.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    fn temporary_dir() -> PathBuf {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let directory = std::env::temp_dir().join(format!("arborist-kotlin-language-{suffix}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }
}
