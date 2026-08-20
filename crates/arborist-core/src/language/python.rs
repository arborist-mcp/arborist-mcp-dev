use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};

pub(crate) fn python_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let normalized_path = normalize_absolute_path(path)?;
    let mut dependencies = BTreeSet::new();
    collect_python_import_dependencies(path, root, source, &mut dependencies)?;
    dependencies.remove(&normalized_path);
    Ok(dependencies)
}

fn collect_python_import_dependencies(
    path: &Path,
    node: Node<'_>,
    source: &str,
    dependencies: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    match node.kind() {
        "import_statement" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                let module_node = match child.kind() {
                    "aliased_import" => child.named_child(0),
                    "dotted_name" | "identifier" => Some(child),
                    _ => None,
                };
                let Some(module_node) = module_node else {
                    continue;
                };
                let module_name = node_text(module_node, source)?.trim();
                if let Some(candidate) = resolve_python_module_path(path, module_name) {
                    dependencies.insert(candidate);
                }
            }
        }
        "import_from_statement" => {
            let mut cursor = node.walk();
            let named_children = node.named_children(&mut cursor).collect::<Vec<_>>();
            let Some(module_node) = named_children.first().copied() else {
                return Ok(());
            };
            let module_name = node_text(module_node, source)?.trim();
            if let Some(candidate) = resolve_python_module_path(path, module_name) {
                dependencies.insert(candidate);
            }
            for child in named_children.into_iter().skip(1) {
                if child.kind() == "wildcard_import" {
                    if let Some(candidate) = resolve_python_module_path(path, module_name) {
                        dependencies.insert(candidate);
                    }
                    continue;
                }
                let imported_node = match child.kind() {
                    "aliased_import" => child.named_child(0),
                    "dotted_name" | "identifier" => Some(child),
                    _ => None,
                };
                let Some(imported_node) = imported_node else {
                    continue;
                };
                let imported_name = node_text(imported_node, source)?.trim();
                if imported_name.is_empty() {
                    continue;
                }
                let joined_name = join_python_module_name(module_name, imported_name);
                let candidate = resolve_python_module_path(path, &joined_name)
                    .or_else(|| resolve_python_module_path(path, module_name));
                if let Some(candidate) = candidate {
                    dependencies.insert(candidate);
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_python_import_dependencies(path, child, source, dependencies)?;
    }
    Ok(())
}

fn join_python_module_name(module_name: &str, imported_name: &str) -> String {
    if module_name.chars().all(|character| character == '.') {
        format!("{module_name}{imported_name}")
    } else if module_name.is_empty() {
        imported_name.to_string()
    } else {
        format!("{module_name}.{imported_name}")
    }
}

fn resolve_python_module_path(path: &Path, module_name: &str) -> Option<PathBuf> {
    let parent = path.parent()?;
    let relative_levels = module_name
        .chars()
        .take_while(|character| *character == '.')
        .count();
    let module_name = module_name.trim_start_matches('.');
    let module_parts = module_name
        .split('.')
        .filter(|part| is_safe_python_module_part(part))
        .collect::<Vec<_>>();
    if module_name.is_empty() && relative_levels == 0 {
        return None;
    }
    if module_name
        .split('.')
        .any(|part| !is_safe_python_module_part(part))
    {
        return None;
    }

    if relative_levels > 0 {
        let mut base = parent.to_path_buf();
        for _ in 0..relative_levels.saturating_sub(1) {
            base = base.parent()?.to_path_buf();
        }
        return resolve_python_module_candidate(base, &module_parts);
    }

    let mut search_root = Some(parent);
    while let Some(root) = search_root {
        if let Some(candidate) = resolve_python_module_candidate(root.to_path_buf(), &module_parts)
        {
            return Some(candidate);
        }
        search_root = root.parent();
    }
    None
}

fn is_safe_python_module_part(part: &str) -> bool {
    !part.is_empty()
        && part
            .chars()
            .all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn resolve_python_module_candidate(mut base: PathBuf, module_parts: &[&str]) -> Option<PathBuf> {
    for part in module_parts {
        base.push(part);
    }
    for extension in ["py", "pyi"] {
        let candidate = base.with_extension(extension);
        if candidate.is_file() {
            return normalize_absolute_path(&candidate).ok();
        }
    }
    for extension in ["py", "pyi"] {
        let candidate = base.join(format!("__init__.{extension}"));
        if candidate.is_file() {
            return normalize_absolute_path(&candidate).ok();
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{python_local_file_dependency_paths, resolve_python_module_path};
    use crate::language::parse_document;

    #[test]
    fn resolves_python_import_and_from_import_dependencies() {
        let root = temporary_dir();
        let caller = root.join("pkg/caller.py");
        let helper = root.join("pkg/helper.py");
        let package = root.join("pkg/subpkg/__init__.py");
        let nested = root.join("pkg/subpkg/nested.py");
        fs::create_dir_all(caller.parent().unwrap()).unwrap();
        fs::create_dir_all(package.parent().unwrap()).unwrap();
        fs::write(&helper, "def helper(): pass\n").unwrap();
        fs::write(&package, "\n").unwrap();
        fs::write(&nested, "def value(): pass\n").unwrap();
        let source = "import helper as local_helper\nfrom . import helper as imported_helper\nfrom .subpkg import (\n    nested,\n)\n";
        fs::write(&caller, source).unwrap();
        let document = parse_document(&caller, source).unwrap();

        let dependencies =
            python_local_file_dependency_paths(&caller, document.tree.root_node(), source).unwrap();
        assert_eq!(
            dependencies,
            [helper, package, nested].into_iter().collect()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_dynamic_and_external_python_imports() {
        let root = temporary_dir();
        let caller = root.join("caller.py");
        fs::write(&caller, "import os\nmodule = __import__('missing')\n").unwrap();
        assert!(resolve_python_module_path(&caller, "os").is_none());
        let document =
            parse_document(&caller, "import os\nmodule = __import__('missing')\n").unwrap();
        let dependencies = python_local_file_dependency_paths(
            &caller,
            document.tree.root_node(),
            "import os\nmodule = __import__('missing')\n",
        )
        .unwrap();
        assert!(dependencies.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_python_stub_and_package_dependencies() {
        let root = temporary_dir();
        let caller = root.join("pkg/caller.py");
        let stub = root.join("pkg/stubs.pyi");
        let package = root.join("pkg/local_package/__init__.py");
        fs::create_dir_all(caller.parent().unwrap()).unwrap();
        fs::create_dir_all(package.parent().unwrap()).unwrap();
        fs::write(&stub, "def typed() -> None: ...\n").unwrap();
        fs::write(&package, "def exported() -> None: pass\n").unwrap();
        let source = "from . import stubs\nfrom .local_package import exported\n";
        fs::write(&caller, source).unwrap();
        let document = parse_document(&caller, source).unwrap();

        let dependencies =
            python_local_file_dependency_paths(&caller, document.tree.root_node(), source).unwrap();
        assert_eq!(dependencies, [stub, package].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    fn temporary_dir() -> PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let directory = std::env::temp_dir().join(format!("arborist-python-language-{suffix}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }
}
