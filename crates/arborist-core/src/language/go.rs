use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::deadline::DeadlineCheck;
use tree_sitter::Node;

use super::{
    node_text, normalize_absolute_path, parse_document, path_is_inside_workspace, read_source,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoLocalPackageImport {
    pub(crate) explicit_local_name: Option<String>,
    pub(crate) source_paths: BTreeSet<PathBuf>,
}

pub(crate) fn go_local_package_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let mut dependencies = go_local_package_imports(path, root, source)?
        .into_iter()
        .flat_map(|import| import.source_paths)
        .collect::<BTreeSet<_>>();
    dependencies.extend(go_same_package_source_paths(path, root, source)?);
    Ok(dependencies)
}

fn go_same_package_source_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    if root.has_error() {
        return Ok(BTreeSet::new());
    }
    let Some(package_name) = go_source_package_name(root, source)? else {
        return Ok(BTreeSet::new());
    };
    let Some(directory) = path.parent() else {
        return Ok(BTreeSet::new());
    };

    let current_path = normalize_absolute_path(path)?;
    let mut dependencies = BTreeSet::new();
    for candidate_path in go_production_source_files_in_directory(directory) {
        if candidate_path == current_path {
            continue;
        }
        let candidate_source = read_source(&candidate_path)?;
        let document = parse_document(&candidate_path, &candidate_source)?;
        let candidate_root = document.tree.root_node();
        if candidate_root.has_error()
            || go_source_package_name(candidate_root, &candidate_source)?.as_deref()
                != Some(package_name.as_str())
        {
            continue;
        }
        dependencies.insert(candidate_path);
    }
    Ok(dependencies)
}

fn go_production_source_files_in_directory(directory: &Path) -> BTreeSet<PathBuf> {
    fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let path = entry.ok()?.path();
            path.is_file().then_some(path)
        })
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("go"))
                && !path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .is_some_and(|stem| stem.ends_with("_test"))
        })
        .filter_map(|path| normalize_absolute_path(&path).ok())
        .collect()
}

fn go_source_package_name(root: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut cursor = root.walk();
    let Some(package_clause) = root
        .named_children(&mut cursor)
        .find(|node| node.kind() == "package_clause")
    else {
        return Ok(None);
    };
    let Some(name) = package_clause.named_child(0) else {
        return Ok(None);
    };
    if name.kind() != "package_identifier" {
        return Ok(None);
    }
    let name = node_text(name, source)?.trim();
    Ok((!name.is_empty()).then(|| name.to_string()))
}

pub(crate) fn go_local_import_binding_statuses(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<(BTreeSet<String>, BTreeSet<String>)> {
    let Some((module_root, module_path)) = find_go_module(path) else {
        return Ok((BTreeSet::new(), BTreeSet::new()));
    };

    let mut local_names = BTreeSet::new();
    let mut resolved_names = BTreeSet::new();
    let mut seen_binding_names = BTreeSet::new();
    let mut ambiguous_names = BTreeSet::new();
    for import in go_import_specs(root, source)? {
        if let Some(deadline) = deadline {
            deadline.check("validating Go local import bindings")?;
        }
        let Some(package_dir) =
            resolve_local_go_package_directory(&module_root, &module_path, &import.path)
        else {
            continue;
        };
        let default_name = import.path.rsplit('/').next().map(str::to_string);
        let explicit_name = import.explicit_local_name.clone();
        let candidate_name = explicit_name.clone().or(default_name);
        let Some(candidate_name) = candidate_name.filter(|name| is_valid_go_identifier(name))
        else {
            continue;
        };
        local_names.insert(candidate_name.clone());

        // Explicit aliases are the actual binding name even when the imported
        // package cannot be inspected, so register them before scanning files.
        if let Some(explicit_name) = explicit_name.as_deref()
            && !seen_binding_names.insert(explicit_name.to_string())
        {
            resolved_names.remove(explicit_name);
            ambiguous_names.insert(explicit_name.to_string());
            continue;
        }

        let source_paths = go_source_files_in_directory(&module_root, &package_dir);
        if source_paths.is_empty() {
            continue;
        }
        let mut package_names = BTreeSet::new();
        for source_path in source_paths {
            if let Some(deadline) = deadline {
                deadline.check("validating Go local import packages")?;
            }
            let candidate_source = match read_source(&source_path) {
                Ok(candidate_source) => candidate_source,
                Err(_) => continue,
            };
            let document = match parse_document(&source_path, &candidate_source) {
                Ok(document) => document,
                Err(_) => continue,
            };
            if let Ok(Some(package_name)) =
                go_source_package_name(document.tree.root_node(), &candidate_source)
            {
                package_names.insert(package_name);
            }
        }
        if package_names.len() == 1 {
            let resolved_name = explicit_name
                .or_else(|| package_names.into_iter().next())
                .expect("Go import package name is present");
            local_names.insert(resolved_name.clone());
            if ambiguous_names.contains(&resolved_name) {
                continue;
            }
            if !seen_binding_names.insert(resolved_name.clone()) {
                resolved_names.remove(&resolved_name);
                ambiguous_names.insert(resolved_name);
                continue;
            }
            resolved_names.insert(resolved_name);
        }
    }
    Ok((local_names, resolved_names))
}

pub(crate) fn go_local_package_imports(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<Vec<GoLocalPackageImport>> {
    let Some((module_root, module_path)) = find_go_module(path) else {
        return Ok(Vec::new());
    };

    let mut imports = Vec::new();
    for import in go_import_specs(root, source)? {
        let Some(package_dir) =
            resolve_local_go_package_directory(&module_root, &module_path, &import.path)
        else {
            continue;
        };
        let source_paths = go_source_files_in_directory(&module_root, &package_dir);
        if !source_paths.is_empty() {
            imports.push(GoLocalPackageImport {
                explicit_local_name: import.explicit_local_name,
                source_paths,
            });
        }
    }
    Ok(imports)
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoImportSpec {
    explicit_local_name: Option<String>,
    path: String,
}

fn go_import_specs(root: Node<'_>, source: &str) -> Result<Vec<GoImportSpec>> {
    if root.kind() != "source_file" {
        return Ok(Vec::new());
    }

    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "import_declaration" {
            collect_go_import_specs(child, source, &mut imports)?;
        }
    }
    Ok(imports)
}

fn collect_go_import_specs(
    node: Node<'_>,
    source: &str,
    imports: &mut Vec<GoImportSpec>,
) -> Result<()> {
    if node.kind() == "import_spec"
        && let Some(path) = node.child_by_field_name("path")
        && let Some(path) = go_import_path_literal(path, source)?
        && let Some(explicit_local_name) = go_explicit_import_local_name(node, source)?
    {
        imports.push(GoImportSpec {
            explicit_local_name,
            path,
        });
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_go_import_specs(child, source, imports)?;
    }
    Ok(())
}

/// Returns `None` for blank and dot imports because neither form introduces
/// a package-qualified identifier into the importing file's scope.
fn go_explicit_import_local_name(node: Node<'_>, source: &str) -> Result<Option<Option<String>>> {
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(Some(None));
    };
    if name.kind() != "package_identifier" {
        return Ok(None);
    }

    let name = node_text(name, source)?.trim();
    Ok((!name.is_empty()).then(|| Some(name.to_string())))
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

fn is_valid_go_identifier(name: &str) -> bool {
    let mut characters = name.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if !(first == '_' || first.is_alphabetic()) {
        return false;
    }
    characters.all(|character| character == '_' || character.is_alphanumeric())
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
    let candidates = go_production_source_files_in_directory(directory)
        .into_iter()
        .filter(|path| path_is_inside_workspace(module_root, path).unwrap_or(false))
        .filter_map(|path| {
            let source = read_source(&path).ok()?;
            let document = parse_document(&path, &source).ok()?;
            let root = document.tree.root_node();
            if root.has_error() {
                return None;
            }
            let package = go_source_package_name(root, &source).ok()??;
            Some((path, package))
        })
        .collect::<Vec<_>>();
    let package_names = candidates
        .iter()
        .map(|(_, package)| package.as_str())
        .collect::<BTreeSet<_>>();
    if package_names.len() != 1 {
        return BTreeSet::new();
    }
    let expected_package = package_names.into_iter().next().unwrap().to_string();
    candidates
        .into_iter()
        .filter(|(_, package)| package == &expected_package)
        .map(|(path, _)| path)
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
    fn imported_packages_ignore_test_and_invalid_sources() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        let package_dir = root.join("internal").join("service");
        let production = package_dir.join("service.go");
        let external_test = package_dir.join("service_test.go");
        let malformed = package_dir.join("broken.go");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = "package main\n\nimport \"example.com/project/internal/service\"\n\nfunc main() { service.Value() }\n";
        fs::write(&command, source).unwrap();
        fs::write(
            &production,
            "package service\nfunc Value() int { return 1 }\n",
        )
        .unwrap();
        fs::write(
            &external_test,
            "package service_test\nfunc TestValue() int { return 0 }\n",
        )
        .unwrap();
        fs::write(&malformed, "package service\nfunc Broken(\n").unwrap();

        let document = parse_document(&command, source).unwrap();
        let dependencies =
            go_local_package_dependency_paths(&command, document.tree.root_node(), source).unwrap();

        assert_eq!(
            dependencies,
            [normalize_absolute_path(&production).unwrap()].into()
        );
    }

    #[test]
    fn imported_packages_reject_mixed_production_package_names() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        let package_dir = root.join("internal").join("service");
        let production = package_dir.join("service.go");
        let mismatched = package_dir.join("foreign.go");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = "package main\n\nimport \"example.com/project/internal/service\"\n\nfunc main() { service.Value() }\n";
        fs::write(&command, source).unwrap();
        fs::write(
            &production,
            "package service\nfunc Value() int { return 1 }\n",
        )
        .unwrap();
        fs::write(
            &mismatched,
            "package foreign\nfunc Foreign() int { return 0 }\n",
        )
        .unwrap();

        let document = parse_document(&command, source).unwrap();
        let dependencies =
            go_local_package_dependency_paths(&command, document.tree.root_node(), source).unwrap();

        assert!(dependencies.is_empty());
    }

    #[test]
    fn includes_unambiguous_same_package_production_sources_as_dependencies() {
        let root = temporary_dir();
        let caller = root.join("caller.go");
        let helper = root.join("helper.go");
        let other_package = root.join("external.go");
        let test_source = root.join("helper_test.go");
        let source = "package metrics\nfunc Caller() int { return Helper() }\n";
        fs::write(&caller, source).unwrap();
        fs::write(&helper, "package metrics\nfunc Helper() int { return 1 }\n").unwrap();
        fs::write(
            &other_package,
            "package metrics_test\nfunc External() int { return 0 }\n",
        )
        .unwrap();
        fs::write(
            &test_source,
            "package metrics_test\nfunc TestHelper() int { return 0 }\n",
        )
        .unwrap();

        let document = parse_document(&caller, source).unwrap();
        let dependencies =
            go_local_package_dependency_paths(&caller, document.tree.root_node(), source).unwrap();

        assert_eq!(
            dependencies,
            [normalize_absolute_path(&helper).unwrap()].into()
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
