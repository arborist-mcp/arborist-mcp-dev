use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::deadline::DeadlineCheck;
use tree_sitter::Node;

use super::{
    node_text, normalize_absolute_path, parse_document, parse_document_with_timeout,
    path_is_inside_workspace, read_source,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoLocalPackageImport {
    pub(crate) explicit_local_name: Option<String>,
    pub(crate) source_paths: BTreeSet<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct GoLocalImportBindingStatuses {
    pub(crate) local_names: BTreeSet<String>,
    pub(crate) resolved_names: BTreeSet<String>,
    pub(crate) resolved_ranges: BTreeMap<String, (usize, usize)>,
}

pub(crate) fn go_local_package_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    go_local_package_dependency_paths_with_deadline(path, root, source, None)
}

pub(crate) fn go_local_package_dependency_paths_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    let imports = match deadline {
        Some(deadline) => {
            go_local_package_imports_with_deadline(path, root, source, Some(deadline))?
        }
        None => go_local_package_imports(path, root, source)?,
    };
    let mut dependencies = imports
        .into_iter()
        .flat_map(|import| import.source_paths)
        .collect::<BTreeSet<_>>();
    dependencies.extend(go_same_package_source_paths_with_deadline(
        path, root, source, deadline,
    )?);
    Ok(dependencies)
}

fn go_same_package_source_paths_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
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
    for candidate_path in
        go_production_source_files_in_directory_with_deadline(directory, deadline)?
    {
        if let Some(deadline) = deadline {
            deadline.check("validating Go same-package sources")?;
        }
        if candidate_path == current_path {
            continue;
        }
        let candidate_source = read_source(&candidate_path)?;
        let Some(document) = parse_go_source_with_deadline(
            &candidate_path,
            &candidate_source,
            deadline,
            "parsing Go same-package sources",
        )?
        else {
            continue;
        };
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

pub(crate) fn go_source_package_name(root: Node<'_>, source: &str) -> Result<Option<String>> {
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
) -> Result<GoLocalImportBindingStatuses> {
    let Some((module_root, module_path)) = find_go_module(path, deadline)? else {
        return Ok(GoLocalImportBindingStatuses::default());
    };

    let mut local_names = BTreeSet::new();
    let mut resolved_names = BTreeSet::new();
    let mut resolved_ranges = BTreeMap::new();
    let mut seen_binding_names = BTreeSet::new();
    let mut ambiguous_names = BTreeSet::new();
    for import in go_import_specs(root, source, deadline)? {
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
        let has_explicit_name = explicit_name.is_some();
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
            resolved_ranges.remove(explicit_name);
            ambiguous_names.insert(explicit_name.to_string());
            continue;
        }

        let Some(package_name) =
            go_unique_local_package_name(&module_root, &package_dir, deadline)?
        else {
            continue;
        };
        let resolved_name = explicit_name.unwrap_or(package_name);
        local_names.insert(resolved_name.clone());
        if ambiguous_names.contains(&resolved_name) {
            continue;
        }
        if !has_explicit_name && !seen_binding_names.insert(resolved_name.clone()) {
            resolved_names.remove(&resolved_name);
            resolved_ranges.remove(&resolved_name);
            ambiguous_names.insert(resolved_name);
            continue;
        }
        resolved_ranges.insert(resolved_name.clone(), (import.start_byte, import.end_byte));
        resolved_names.insert(resolved_name);
    }
    Ok(GoLocalImportBindingStatuses {
        local_names,
        resolved_names,
        resolved_ranges,
    })
}

fn go_unique_local_package_name(
    module_root: &Path,
    directory: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<String>> {
    let source_paths = go_production_source_files_in_directory_with_deadline(directory, deadline)?
        .into_iter()
        .filter(|path| path_is_inside_workspace(module_root, path).unwrap_or(false))
        .collect::<BTreeSet<_>>();
    if source_paths.is_empty() {
        return Ok(None);
    }

    let mut package_name = None;
    for source_path in source_paths {
        if let Some(deadline) = deadline {
            deadline.check("validating Go local import packages")?;
        }
        let candidate_source = match read_source(&source_path) {
            Ok(candidate_source) => candidate_source,
            Err(_) => return Ok(None),
        };
        let Some(candidate_name) =
            go_source_package_name_with_deadline(&source_path, &candidate_source, deadline)?
        else {
            return Ok(None);
        };
        if package_name
            .as_ref()
            .is_some_and(|package_name| package_name != &candidate_name)
        {
            return Ok(None);
        }
        package_name = Some(candidate_name);
    }
    Ok(package_name)
}

fn go_source_package_name_with_deadline(
    path: &Path,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<String>> {
    let Some(document) =
        parse_go_source_with_deadline(path, source, deadline, "parsing Go local import packages")?
    else {
        return Ok(None);
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(None);
    }
    go_source_package_name(root, source)
}

fn parse_go_source_with_deadline(
    path: &Path,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
    phase: &str,
) -> Result<Option<super::ParsedDocument>> {
    let document = if let Some(deadline) = deadline {
        match deadline.remaining_timeout_micros(phase)? {
            Some(timeout_micros) => parse_document_with_timeout(path, source, timeout_micros)?,
            None => match parse_document(path, source) {
                Ok(document) => document,
                Err(_) => return Ok(None),
            },
        }
    } else {
        match parse_document(path, source) {
            Ok(document) => document,
            Err(_) => return Ok(None),
        }
    };
    Ok(Some(document))
}
fn go_production_source_files_in_directory_with_deadline(
    directory: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return Ok(BTreeSet::new()),
    };
    let mut paths = BTreeSet::new();
    for entry in entries {
        if let Some(deadline) = deadline {
            deadline.check("scanning Go local import package files")?;
        }
        let Ok(path) = entry.map(|entry| entry.path()) else {
            continue;
        };
        if !path.is_file()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("go"))
            || path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.ends_with("_test"))
        {
            continue;
        }
        if let Ok(path) = normalize_absolute_path(&path) {
            paths.insert(path);
        }
    }
    Ok(paths)
}

pub(crate) fn go_local_package_imports(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<Vec<GoLocalPackageImport>> {
    go_local_package_imports_with_deadline(path, root, source, None)
}

pub(crate) fn go_local_package_imports_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<GoLocalPackageImport>> {
    let Some((module_root, module_path)) = find_go_module(path, deadline)? else {
        return Ok(Vec::new());
    };

    let mut imports = Vec::new();
    for import in go_import_specs(root, source, deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving Go local package imports")?;
        }
        let Some(package_dir) =
            resolve_local_go_package_directory(&module_root, &module_path, &import.path)
        else {
            continue;
        };
        let source_paths =
            go_source_files_in_directory_with_deadline(&module_root, &package_dir, deadline)?;
        if !source_paths.is_empty() {
            imports.push(GoLocalPackageImport {
                explicit_local_name: import.explicit_local_name,
                source_paths,
            });
        }
    }
    Ok(imports)
}

fn find_go_module(
    path: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<(PathBuf, String)>> {
    let Some(mut directory) = path.parent().map(Path::to_path_buf) else {
        return Ok(None);
    };
    loop {
        if let Some(deadline) = deadline {
            deadline.check("locating Go module root")?;
        }
        let module_file = directory.join("go.mod");
        if module_file.is_file() {
            let Ok(source) = read_source(&module_file) else {
                return Ok(None);
            };
            let Some(module_path) = go_module_path(&source) else {
                return Ok(None);
            };
            let Ok(module_root) = normalize_absolute_path(&directory) else {
                return Ok(None);
            };
            return Ok(Some((module_root, module_path)));
        }
        if !directory.pop() {
            return Ok(None);
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
    start_byte: usize,
    end_byte: usize,
}

fn go_import_specs(
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<GoImportSpec>> {
    if root.kind() != "source_file" {
        return Ok(Vec::new());
    }

    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "import_declaration" {
            collect_go_import_specs(child, source, &mut imports, deadline)?;
        }
    }
    Ok(imports)
}

fn check_go_import_specs_deadline(deadline: Option<&dyn DeadlineCheck>) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("scanning Go import specs")?;
    }
    Ok(())
}

fn collect_go_import_specs(
    node: Node<'_>,
    source: &str,
    imports: &mut Vec<GoImportSpec>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    check_go_import_specs_deadline(deadline)?;
    if node.kind() == "import_spec" {
        if let Some(path) = node.child_by_field_name("path")
            && let Some(path) = go_import_path_literal(path, source)?
            && let Some(explicit_local_name) = go_explicit_import_local_name(node, source)?
        {
            imports.push(GoImportSpec {
                explicit_local_name,
                path,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
            });
        }
        return Ok(());
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_go_import_specs(child, source, imports, deadline)?;
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

fn go_source_files_in_directory_with_deadline(
    module_root: &Path,
    directory: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    let mut candidates = Vec::new();
    for path in go_production_source_files_in_directory_with_deadline(directory, deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("validating Go local import package sources")?;
        }
        if !path_is_inside_workspace(module_root, &path).unwrap_or(false) {
            continue;
        }
        let Ok(source) = read_source(&path) else {
            continue;
        };
        let Some(document) = parse_go_source_with_deadline(
            &path,
            &source,
            deadline,
            "parsing Go local import package sources",
        )?
        else {
            continue;
        };
        let root = document.tree.root_node();
        if root.has_error() {
            continue;
        }
        let Some(package) = go_source_package_name(root, &source)? else {
            continue;
        };
        candidates.push((path, package));
    }
    let package_names = candidates
        .iter()
        .map(|(_, package)| package.as_str())
        .collect::<BTreeSet<_>>();
    if package_names.len() != 1 {
        return Ok(BTreeSet::new());
    }
    let expected_package = package_names.into_iter().next().unwrap().to_string();
    Ok(candidates
        .into_iter()
        .filter(|(_, package)| package == &expected_package)
        .map(|(path, _)| path)
        .collect())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use anyhow::Result;

    use super::{
        go_local_import_binding_statuses, go_local_package_dependency_paths,
        go_local_package_dependency_paths_with_deadline, go_local_package_imports,
        go_local_package_imports_with_deadline,
    };
    use crate::deadline::DeadlineCheck;
    use crate::language::{MAX_SOURCE_FILE_BYTES, normalize_absolute_path, parse_document};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct RejectAfterChecks {
        allowed_checks: usize,
        checks: Cell<usize>,
    }

    impl RejectAfterChecks {
        fn new(allowed_checks: usize) -> Self {
            Self {
                allowed_checks,
                checks: Cell::new(0),
            }
        }
    }

    impl DeadlineCheck for RejectAfterChecks {
        fn check(&self, phase: &str) -> Result<()> {
            let checks = self.checks.get();
            self.checks.set(checks + 1);
            if checks >= self.allowed_checks {
                anyhow::bail!("deadline check reached {phase}");
            }
            Ok(())
        }
    }

    struct RejectPhase(&'static str);

    impl DeadlineCheck for RejectPhase {
        fn check(&self, phase: &str) -> Result<()> {
            if phase == self.0 {
                anyhow::bail!("deadline check reached {phase}");
            }
            Ok(())
        }
    }

    struct RejectRemainingTimeout;

    impl DeadlineCheck for RejectRemainingTimeout {
        fn check(&self, _phase: &str) -> Result<()> {
            Ok(())
        }

        fn remaining_timeout_micros(&self, phase: &str) -> Result<Option<u64>> {
            anyhow::bail!("deadline budget requested during {phase}");
        }
    }

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
    fn local_import_binding_validation_checks_deadline_while_scanning_package_files() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        let package_dir = root.join("internal").join("service");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = "package main\n\nimport \"example.com/project/internal/service\"\n";
        fs::write(&command, source).unwrap();
        fs::write(
            package_dir.join("service.go"),
            "package service\nfunc Value() int { return 1 }\n",
        )
        .unwrap();
        fs::write(
            package_dir.join("other.go"),
            "package service\nfunc Other() int { return 2 }\n",
        )
        .unwrap();

        let document = parse_document(&command, source).unwrap();
        let deadline = RejectPhase("scanning Go local import package files");
        let error = go_local_import_binding_statuses(
            &command,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("deadline should interrupt package-directory scanning");

        assert!(
            error
                .to_string()
                .contains("deadline check reached scanning Go local import package files"),
            "{error:#}"
        );
    }

    #[test]
    fn local_package_import_resolution_checks_deadline_while_locating_module_root() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = "package main\n";
        fs::write(&command, source).unwrap();

        let document = parse_document(&command, source).unwrap();
        let deadline = RejectAfterChecks::new(0);
        let error = go_local_package_imports_with_deadline(
            &command,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("deadline should interrupt Go module-root discovery");

        assert!(
            error
                .to_string()
                .contains("deadline check reached locating Go module root"),
            "{error:#}"
        );
    }

    #[test]
    fn local_package_import_resolution_propagates_parser_deadline_errors() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        let package_dir = root.join("internal").join("service");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = "package main\n\nimport \"example.com/project/internal/service\"\n";
        fs::write(&command, source).unwrap();
        fs::write(
            package_dir.join("service.go"),
            "package service\nfunc Value() int { return 1 }\n",
        )
        .unwrap();

        let document = parse_document(&command, source).unwrap();
        let error = go_local_package_imports_with_deadline(
            &command,
            document.tree.root_node(),
            source,
            Some(&RejectRemainingTimeout),
        )
        .expect_err("parser deadline budget errors should propagate");

        assert!(
            error.to_string().contains(
                "deadline budget requested during parsing Go local import package sources"
            ),
            "{error:#}"
        );
    }

    #[test]
    fn local_import_binding_validation_propagates_parser_deadline_errors() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        let package_dir = root.join("internal").join("service");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = "package main\n\nimport \"example.com/project/internal/service\"\n";
        fs::write(&command, source).unwrap();
        fs::write(
            package_dir.join("service.go"),
            "package service\nfunc Value() int { return 1 }\n",
        )
        .unwrap();

        let document = parse_document(&command, source).unwrap();
        let error = go_local_import_binding_statuses(
            &command,
            document.tree.root_node(),
            source,
            Some(&RejectRemainingTimeout),
        )
        .expect_err("parser deadline budget errors should propagate");

        assert!(
            error
                .to_string()
                .contains("deadline budget requested during parsing Go local import packages"),
            "{error:#}"
        );
    }
    #[test]
    fn local_package_import_resolution_checks_deadline_while_validating_sources() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        let package_dir = root.join("internal").join("service");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::create_dir_all(&package_dir).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = "package main\n\nimport \"example.com/project/internal/service\"\n";
        fs::write(&command, source).unwrap();
        fs::write(
            package_dir.join("service.go"),
            "package service\nfunc Value() int { return 1 }\n",
        )
        .unwrap();
        fs::write(
            package_dir.join("other.go"),
            "package service\nfunc Other() int { return 2 }\n",
        )
        .unwrap();

        let document = parse_document(&command, source).unwrap();
        let deadline = RejectPhase("validating Go local import package sources");
        let error = go_local_package_imports_with_deadline(
            &command,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("deadline should interrupt local package source validation");

        assert!(
            error
                .to_string()
                .contains("deadline check reached validating Go local import package sources"),
            "{error:#}"
        );
    }

    #[test]
    fn local_import_binding_validation_checks_deadline_while_scanning_import_specs() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        fs::write(root.join("go.mod"), "module example.com/project\n").unwrap();
        let source = r#"package main

import (
    "example.com/project/internal/first"
    "example.com/project/internal/second"
)
"#;
        fs::write(&command, source).unwrap();

        let document = parse_document(&command, source).unwrap();
        let deadline = RejectPhase("scanning Go import specs");
        let error = go_local_import_binding_statuses(
            &command,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("deadline should interrupt import-spec scanning");

        assert!(
            error
                .to_string()
                .contains("deadline check reached scanning Go import specs"),
            "{error:#}"
        );
    }

    #[test]
    fn local_file_dependency_scan_checks_deadline_for_same_package_sources() {
        let root = temporary_dir();
        let source_path = root.join("cmd").join("main.go");
        let sibling_path = root.join("cmd").join("helper.go");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        let source = "package main\n\nfunc Main() { Helper() }\n";
        fs::write(&source_path, source).unwrap();
        fs::write(&sibling_path, "package main\nfunc Helper() {}\n").unwrap();

        let document = parse_document(&source_path, source).unwrap();
        let deadline = RejectPhase("validating Go same-package sources");
        let error = go_local_package_dependency_paths_with_deadline(
            &source_path,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("deadline should interrupt same-package source validation");

        assert!(
            error
                .to_string()
                .contains("deadline check reached validating Go same-package sources"),
            "{error:#}"
        );
    }

    #[test]
    fn local_file_dependency_scan_propagates_parser_deadline_errors() {
        let root = temporary_dir();
        let source_path = root.join("cmd").join("main.go");
        let sibling_path = root.join("cmd").join("helper.go");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        let source = "package main\n\nfunc Main() { Helper() }\n";
        fs::write(&source_path, source).unwrap();
        fs::write(&sibling_path, "package main\nfunc Helper() {}\n").unwrap();

        let document = parse_document(&source_path, source).unwrap();
        let error = go_local_package_dependency_paths_with_deadline(
            &source_path,
            document.tree.root_node(),
            source,
            Some(&RejectRemainingTimeout),
        )
        .expect_err("parser deadline budget errors should propagate");

        assert!(
            error
                .to_string()
                .contains("deadline budget requested during parsing Go same-package sources"),
            "{error:#}"
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
    fn ignores_oversized_go_module_metadata() {
        let root = temporary_dir();
        let command = root.join("cmd").join("main.go");
        fs::create_dir_all(command.parent().unwrap()).unwrap();
        let module_file = root.join("go.mod");
        let file = fs::File::create(&module_file).unwrap();
        file.set_len(MAX_SOURCE_FILE_BYTES + 1).unwrap();
        let source = "package main\n\nimport \"example.com/project/internal/service\"\n";
        fs::write(&command, source).unwrap();

        let document = parse_document(&command, source).unwrap();
        let imports =
            go_local_package_imports(&command, document.tree.root_node(), source).unwrap();

        assert!(imports.is_empty());
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
