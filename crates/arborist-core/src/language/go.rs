use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path, path_is_inside_workspace};

pub(crate) fn go_local_package_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let Some((module_root, module_path)) = find_go_module(path) else {
        return Ok(BTreeSet::new());
    };

    let mut dependencies = BTreeSet::new();
    for import_path in go_import_paths(root, source)? {
        let Some(package_dir) =
            resolve_local_go_package_directory(&module_root, &module_path, &import_path)
        else {
            continue;
        };
        dependencies.extend(go_source_files_in_directory(&module_root, &package_dir));
    }
    Ok(dependencies)
}

fn find_go_module(path: &Path) -> Option<(PathBuf, String)> {
    let mut directory = path.parent()?.to_path_buf();
    loop {
        let module_file = directory.join("go.mod");
        if module_file.is_file() {
            let source = fs::read_to_string(module_file).ok()?;
            let module_path = go_module_path(&source)?;
            let module_root = normalize_absolute_path(&directory).ok()?;
            return Some((module_root, module_path));
        }
        if !directory.pop() {
            return None;
        }
    }
}

fn go_module_path(source: &str) -> Option<String> {
    let mut module_path = None;
    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        let Some(rest) = line.strip_prefix("module") else {
            continue;
        };
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }

        let candidate = rest
            .split_once("//")
            .map_or(rest, |(candidate, _)| candidate)
            .trim();
        if !is_valid_go_module_path(candidate) || module_path.is_some() {
            return None;
        }
        module_path = Some(candidate.to_string());
    }
    module_path
}

fn is_valid_go_module_path(module_path: &str) -> bool {
    !module_path.is_empty()
        && !module_path.starts_with('.')
        && module_path.split('/').all(is_safe_go_package_segment)
}

fn go_import_paths(root: Node<'_>, source: &str) -> Result<BTreeSet<String>> {
    if root.kind() != "source_file" {
        return Ok(BTreeSet::new());
    }

    let mut imports = BTreeSet::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "import_declaration" {
            collect_go_import_paths(child, source, &mut imports)?;
        }
    }
    Ok(imports)
}

fn collect_go_import_paths(
    node: Node<'_>,
    source: &str,
    imports: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "import_spec"
        && let Some(path) = node.child_by_field_name("path")
        && let Some(import_path) = go_import_path_literal(path, source)?
    {
        imports.insert(import_path);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_go_import_paths(child, source, imports)?;
    }
    Ok(())
}

fn go_import_path_literal(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let literal = node_text(node, source)?.trim();
    let import_path = match node.kind() {
        "interpreted_string_literal" => literal
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .filter(|value| !value.contains('\\')),
        "raw_string_literal" => literal
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
            .filter(|value| !value.contains(['\r', '\n'])),
        _ => None,
    };
    Ok(import_path
        .filter(|path| is_valid_go_import_path(path))
        .map(str::to_string))
}

fn is_valid_go_import_path(import_path: &str) -> bool {
    !import_path.is_empty() && import_path.split('/').all(is_safe_go_package_segment)
}

fn is_safe_go_package_segment(segment: &str) -> bool {
    !segment.is_empty()
        && !matches!(segment, "." | "..")
        && !segment.ends_with('.')
        && !segment.chars().any(|character| {
            character.is_control()
                || character.is_whitespace()
                || matches!(character, '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        })
        && !is_windows_reserved_file_name(segment)
}

fn is_windows_reserved_file_name(segment: &str) -> bool {
    let stem = segment.split_once('.').map_or(segment, |(stem, _)| stem);
    let stem = stem.to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|suffix| {
                matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn resolve_local_go_package_directory(
    module_root: &Path,
    module_path: &str,
    import_path: &str,
) -> Option<PathBuf> {
    let relative_path = import_path
        .strip_prefix(module_path)
        .and_then(|suffix| suffix.strip_prefix('/'))?;
    if relative_path.is_empty() {
        return None;
    }

    let mut directory = module_root.to_path_buf();
    for segment in relative_path.split('/') {
        if !is_safe_go_package_segment(segment) {
            return None;
        }
        directory.push(segment);
    }
    let directory = normalize_absolute_path(&directory).ok()?;
    if !path_is_inside_workspace(module_root, &directory).ok()? {
        return None;
    }
    Some(directory)
}

fn go_source_files_in_directory(module_root: &Path, directory: &Path) -> BTreeSet<PathBuf> {
    fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            path.is_file().then_some(path).filter(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("go"))
            })
        })
        .filter_map(|path| normalize_absolute_path(&path).ok())
        .filter(|path| path_is_inside_workspace(module_root, path).unwrap_or(false))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::go_local_package_dependency_paths;
    use crate::language::{normalize_absolute_path, parse_document};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn resolves_unambiguous_local_module_imports_to_all_package_source_files() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        let package_dir = root.join("internal").join("service");
        let first = package_dir.join("first.go");
        let second = package_dir.join("second.go");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = "package main\n\nimport (\n    \"fmt\"\n    \"example.com/project/internal/service\"\n)\n\nfunc main() { fmt.Println(service.Value()) }\n";
        fs::write(&command, source).unwrap();
        fs::write(&first, "package service\nfunc Value() int { return 1 }\n").unwrap();
        fs::write(&second, "package service\nfunc Other() int { return 2 }\n").unwrap();

        let document = parse_document(&command, source).unwrap();
        let dependencies =
            go_local_package_dependency_paths(&command, document.tree.root_node(), source).unwrap();

        assert_eq!(
            dependencies,
            [
                normalize_absolute_path(&first).unwrap(),
                normalize_absolute_path(&second).unwrap(),
            ]
            .into()
        );
    }

    #[test]
    fn ignores_external_and_escaped_imports() {
        let root = temporary_dir();
        let source_path = root.join("cmd").join("main.go");
        let package_dir = root.join("internal").join("service");
        let service = package_dir.join("service.go");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        fs::write(&service, "package service\nfunc Value() int { return 1 }\n").unwrap();
        let source = "package main\n\nimport (\n    \"fmt\"\n    \"example.com/project\\x2finternal/service\"\n)\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            go_local_package_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();
        assert!(dependencies.is_empty());
    }

    #[test]
    fn uses_the_nearest_valid_go_module_boundary() {
        let root = temporary_dir();
        let nested_root = root.join("tools");
        let source_path = nested_root.join("cmd").join("main.go");
        let nested_package = nested_root
            .join("internal")
            .join("service")
            .join("service.go");
        let outer_package = root.join("internal").join("service").join("service.go");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(nested_package.parent().unwrap()).unwrap();
        fs::create_dir_all(outer_package.parent().unwrap()).unwrap();
        fs::write(root.join("go.mod"), "module example.com/outer\n").unwrap();
        fs::write(nested_root.join("go.mod"), "module example.com/tools\n").unwrap();
        let source = "package main\n\nimport \"example.com/tools/internal/service\"\n";
        fs::write(&source_path, source).unwrap();
        fs::write(
            &nested_package,
            "package service\nfunc Value() int { return 1 }\n",
        )
        .unwrap();
        fs::write(
            &outer_package,
            "package service\nfunc Value() int { return 2 }\n",
        )
        .unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            go_local_package_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();

        assert_eq!(
            dependencies,
            [normalize_absolute_path(&nested_package).unwrap()].into()
        );
    }

    #[test]
    fn does_not_cross_an_invalid_nested_go_module_boundary() {
        let root = temporary_dir();
        let nested_root = root.join("tools");
        let source_path = nested_root.join("cmd").join("main.go");
        let outer_package = root.join("internal").join("service").join("service.go");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(outer_package.parent().unwrap()).unwrap();
        fs::write(root.join("go.mod"), "module example.com/outer\n").unwrap();
        fs::write(nested_root.join("go.mod"), "module\n").unwrap();
        let source = "package main\n\nimport \"example.com/outer/internal/service\"\n";
        fs::write(&source_path, source).unwrap();
        fs::write(
            &outer_package,
            "package service\nfunc Value() int { return 1 }\n",
        )
        .unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            go_local_package_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();

        assert!(dependencies.is_empty());
    }

    #[test]
    fn ignores_module_root_imports_and_missing_module_metadata() {
        let root = temporary_dir();
        let source_path = root.join("cmd").join("main.go");
        let root_package = root.join("value.go");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        fs::write(
            &root_package,
            "package project\nfunc Value() int { return 1 }\n",
        )
        .unwrap();
        let source = "package main\n\nimport \"example.com/project\"\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            go_local_package_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();
        assert!(dependencies.is_empty());

        let missing_module_path = temporary_dir().join("main.go");
        let missing_module_source =
            "package main\n\nimport \"example.com/project/internal/service\"\n";
        fs::write(&missing_module_path, missing_module_source).unwrap();
        let document = parse_document(&missing_module_path, missing_module_source).unwrap();
        let dependencies = go_local_package_dependency_paths(
            &missing_module_path,
            document.tree.root_node(),
            missing_module_source,
        )
        .unwrap();
        assert!(dependencies.is_empty());
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
        let directory = std::env::temp_dir().join(format!("arborist-go-language-{suffix}"));
        fs::create_dir_all(&directory).unwrap();
        directory
    }
}
