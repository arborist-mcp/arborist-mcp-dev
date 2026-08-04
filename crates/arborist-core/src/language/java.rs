use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path, parse_document};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JavaLocalTypeImport {
    pub(crate) local_name: String,
    pub(crate) semantic_path: String,
    pub(crate) source_path: PathBuf,
}

pub(crate) fn java_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    Ok(java_local_explicit_type_imports(path, root, source)?
        .into_iter()
        .map(|import| import.source_path)
        .collect())
}

pub(crate) fn java_local_explicit_type_imports(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<Vec<JavaLocalTypeImport>> {
    let normalized_path = normalize_absolute_path(path)?;
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "import_declaration" {
            continue;
        }
        let Some(import_path) = java_explicit_import_path(node, source)? else {
            continue;
        };
        let Some(source_path) = resolve_unique_java_source_path(path, &import_path) else {
            continue;
        };
        if source_path == normalized_path {
            continue;
        }
        let Some(local_name) = import_path
            .rsplit('.')
            .next()
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        imports.push(JavaLocalTypeImport {
            local_name: local_name.to_string(),
            semantic_path: import_path.replace('.', "::"),
            source_path,
        });
    }
    Ok(imports)
}

fn java_explicit_import_path(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let text = node_text(node, source)?.trim();
    if text.split_whitespace().nth(1) == Some("static") {
        return Ok(None);
    }

    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    if children.iter().any(|child| child.kind() == "asterisk") {
        return Ok(None);
    }
    let Some(name) = children
        .into_iter()
        .find(|child| matches!(child.kind(), "identifier" | "scoped_identifier"))
    else {
        return Ok(None);
    };
    let import_path = node_text(name, source)?.trim().to_string();
    if import_path.is_empty() || !is_safe_java_qualified_name(&import_path) {
        return Ok(None);
    }
    Ok(Some(import_path))
}

fn resolve_unique_java_source_path(path: &Path, import_path: &str) -> Option<PathBuf> {
    let segments = import_path.split('.').collect::<Vec<_>>();
    let mut candidates = BTreeSet::new();
    let mut source_root = path.parent()?.to_path_buf();
    loop {
        let mut candidate = source_root.clone();
        for segment in &segments {
            candidate.push(segment);
        }
        candidate.set_extension("java");
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
        .find(|node| node.kind() == "package_declaration")
    else {
        return false;
    };
    let mut cursor = package.walk();
    let Some(name) = package
        .named_children(&mut cursor)
        .find(|node| matches!(node.kind(), "identifier" | "scoped_identifier"))
    else {
        return false;
    };
    node_text(name, &source)
        .map(|name| name.trim() == expected_package)
        .unwrap_or(false)
}

fn is_safe_java_qualified_name(name: &str) -> bool {
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

    use super::{java_local_explicit_type_imports, java_local_file_dependency_paths};
    use crate::language::parse_document;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn resolves_unique_explicit_java_imports_from_ancestor_source_roots() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.java");
        let helper_path = root.join("src/com/example/Helper.java");
        let widget_path = root.join("src/com/example/types/Widget.java");
        let mismatched_helper_path = root.join("src/com/example/com/example/Helper.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(widget_path.parent().unwrap()).unwrap();
        fs::create_dir_all(mismatched_helper_path.parent().unwrap()).unwrap();
        fs::write(&helper_path, "package com.example; class Helper {}\n").unwrap();
        fs::write(&widget_path, "package com.example.types; class Widget {}\n").unwrap();
        fs::write(
            &mismatched_helper_path,
            "package unrelated; class Helper {}\n",
        )
        .unwrap();
        let source = "package com.example;\nimport com.example.Helper;\nimport com.example.types.Widget;\nimport static com.example.Helper.VALUE;\nimport com.example.Missing;\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();

        assert_eq!(
            dependencies,
            [helper_path.clone(), widget_path.clone()]
                .into_iter()
                .collect()
        );
        assert_eq!(
            java_local_explicit_type_imports(&source_path, document.tree.root_node(), source)
                .unwrap(),
            vec![
                super::JavaLocalTypeImport {
                    local_name: "Helper".to_string(),
                    semantic_path: "com::example::Helper".to_string(),
                    source_path: helper_path,
                },
                super::JavaLocalTypeImport {
                    local_name: "Widget".to_string(),
                    semantic_path: "com::example::types::Widget".to_string(),
                    source_path: widget_path,
                },
            ]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ignores_wildcard_and_ambiguous_java_imports() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.java");
        let first_helper = root.join("src/com/example/Helper.java");
        let second_helper = root.join("src/com/example/com/example/Helper.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(second_helper.parent().unwrap()).unwrap();
        fs::write(&first_helper, "package com.example; class Helper {}\n").unwrap();
        fs::write(&second_helper, "package com.example; class Helper {}\n").unwrap();
        let source = "package com.example;\nimport com.example.*;\nimport com.example.Helper;\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), source)
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
        let directory = std::env::temp_dir().join(format!("arborist-java-language-{suffix}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }
}
