use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path, parse_document, parse_document_with_timeout};
use crate::deadline::DeadlineCheck;

pub(crate) fn kotlin_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    kotlin_local_file_dependency_paths_with_deadline(path, root, source, None)
}

pub(crate) fn kotlin_local_file_dependency_paths_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let normalized_path = normalize_absolute_path(path)?;
    let mut dependencies = BTreeSet::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        check_local_file_dependency_deadline(deadline)?;
        if node.kind() != "import" {
            continue;
        }
        if let Some(import_path) = kotlin_explicit_import_path(node, source)? {
            let Some(source_path) =
                resolve_unique_kotlin_source_path(path, &import_path, deadline)?
            else {
                continue;
            };
            if source_path != normalized_path {
                dependencies.insert(source_path);
            }
            continue;
        }

        let Some(package_name) = kotlin_wildcard_import_package(node, source)? else {
            continue;
        };
        dependencies.extend(
            resolve_unique_kotlin_package_source_paths(path, &package_name, deadline)?
                .into_iter()
                .filter(|source_path| source_path != &normalized_path),
        );
    }
    Ok(dependencies)
}

fn check_local_file_dependency_deadline(deadline: Option<&dyn DeadlineCheck>) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting local file dependencies")?;
    }
    Ok(())
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

fn kotlin_wildcard_import_package(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if !node_text(node, source)?.contains('*') {
        return Ok(None);
    }

    let mut cursor = node.walk();
    let Some(identifier) = node
        .named_children(&mut cursor)
        .find(|child| child.kind() == "qualified_identifier")
    else {
        return Ok(None);
    };
    let package_name = node_text(identifier, source)?.trim().to_string();
    if package_name.is_empty() || !is_safe_kotlin_qualified_name(&package_name) {
        return Ok(None);
    }
    Ok(Some(package_name))
}

fn resolve_unique_kotlin_source_path(
    path: &Path,
    import_path: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let segments = import_path.split('.').collect::<Vec<_>>();
    let mut candidates = BTreeSet::new();
    let Some(mut source_root) = path.parent().map(Path::to_path_buf) else {
        return Ok(None);
    };
    loop {
        check_local_file_dependency_deadline(deadline)?;
        let mut candidate = source_root.clone();
        for segment in &segments {
            candidate.push(segment);
        }
        for extension in ["kt", "kts"] {
            check_local_file_dependency_deadline(deadline)?;
            candidate.set_extension(extension);
            if candidate.is_file()
                && candidate_declares_import_package(&candidate, import_path, deadline)?
                && let Ok(candidate) = normalize_absolute_path(&candidate)
            {
                candidates.insert(candidate);
            }
            candidate.set_extension("");
        }

        if !source_root.pop() {
            break;
        }
    }
    Ok((candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten())
}

fn resolve_unique_kotlin_package_source_paths(
    path: &Path,
    package_name: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let segments = package_name.split('.').collect::<Vec<_>>();
    let mut package_directories = BTreeSet::new();
    let Some(mut source_root) = path.parent().map(Path::to_path_buf) else {
        return Ok(BTreeSet::new());
    };

    loop {
        check_local_file_dependency_deadline(deadline)?;
        let mut candidate = source_root.clone();
        for segment in &segments {
            candidate.push(segment);
        }
        if candidate.is_dir()
            && kotlin_package_directory_contains_source_file(&candidate, package_name, deadline)?
            && let Ok(candidate) = normalize_absolute_path(&candidate)
        {
            package_directories.insert(candidate);
        }

        if !source_root.pop() {
            break;
        }
    }

    if package_directories.len() != 1 {
        return Ok(BTreeSet::new());
    }
    let directory = package_directories.into_iter().next().unwrap();
    Ok(
        kotlin_source_files_in_package_directory(&directory, package_name, deadline)?
            .into_iter()
            .filter_map(|candidate| normalize_absolute_path(&candidate).ok())
            .collect(),
    )
}

fn kotlin_package_directory_contains_source_file(
    directory: &Path,
    package_name: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<bool> {
    check_local_file_dependency_deadline(deadline)?;
    for entry in fs::read_dir(directory).ok().into_iter().flatten().flatten() {
        check_local_file_dependency_deadline(deadline)?;
        let candidate = entry.path();
        if candidate.is_file()
            && candidate
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("kt") || extension.eq_ignore_ascii_case("kts")
                })
            && candidate_declares_package(&candidate, package_name, deadline)?
        {
            return Ok(true);
        }
    }
    check_local_file_dependency_deadline(deadline)?;
    Ok(false)
}

fn kotlin_source_files_in_package_directory(
    directory: &Path,
    package_name: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let mut source_paths = Vec::new();
    for entry in fs::read_dir(directory).ok().into_iter().flatten().flatten() {
        check_local_file_dependency_deadline(deadline)?;
        let candidate = entry.path();
        if candidate.is_file()
            && candidate
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    extension.eq_ignore_ascii_case("kt") || extension.eq_ignore_ascii_case("kts")
                })
            && candidate_declares_package(&candidate, package_name, deadline)?
        {
            source_paths.push(candidate);
        }
    }
    check_local_file_dependency_deadline(deadline)?;
    Ok(source_paths)
}

fn candidate_declares_import_package(
    candidate: &Path,
    import_path: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<bool> {
    let Some((expected_package, _)) = import_path.rsplit_once('.') else {
        return Ok(false);
    };
    candidate_declares_package(candidate, expected_package, deadline)
}

fn candidate_declares_package(
    candidate: &Path,
    expected_package: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<bool> {
    check_local_file_dependency_deadline(deadline)?;
    let Ok(source) = fs::read_to_string(candidate) else {
        return Ok(false);
    };
    check_local_file_dependency_deadline(deadline)?;
    let document = if let Some(deadline) = deadline {
        match deadline
            .remaining_timeout_micros("parsing Kotlin local file dependency candidates")?
        {
            Some(timeout_micros) => {
                parse_document_with_timeout(candidate, &source, timeout_micros)?
            }
            None => match parse_document(candidate, &source) {
                Ok(document) => document,
                Err(_) => return Ok(false),
            },
        }
    } else {
        match parse_document(candidate, &source) {
            Ok(document) => document,
            Err(_) => return Ok(false),
        }
    };
    let mut cursor = document.tree.root_node().walk();
    let Some(package) = document
        .tree
        .root_node()
        .named_children(&mut cursor)
        .find(|node| node.kind() == "package_header")
    else {
        return Ok(false);
    };
    let mut cursor = package.walk();
    let Some(name) = package
        .named_children(&mut cursor)
        .find(|node| node.kind() == "qualified_identifier")
    else {
        return Ok(false);
    };
    Ok(node_text(name, &source)
        .map(|name| name.trim() == expected_package)
        .unwrap_or(false))
}

fn is_safe_kotlin_qualified_name(name: &str) -> bool {
    name.split('.').all(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(['/', '\\'])
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use anyhow::bail;

    use super::{
        kotlin_local_file_dependency_paths, kotlin_local_file_dependency_paths_with_deadline,
        kotlin_package_directory_contains_source_file,
    };
    use crate::deadline::DeadlineCheck;
    use crate::language::parse_document;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

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
                bail!("test deadline expired during {phase}")
            }
            Ok(())
        }
    }

    #[test]
    fn package_directory_scan_checks_deadline_after_failed_directory_read() {
        let root = temporary_dir();
        let missing_directory = root.join("missing");
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 1,
        };

        let error = kotlin_package_directory_contains_source_file(
            &missing_directory,
            "com.example",
            Some(&deadline),
        )
        .expect_err("deadline should stop after a failed Kotlin directory read");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during extracting local file dependencies"),
            "{error:#}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_directory_scan_checks_deadline_after_opening_empty_directory() {
        let root = temporary_dir();
        let empty_directory = root.join("empty");
        fs::create_dir_all(&empty_directory).unwrap();
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 1,
        };

        let error = kotlin_package_directory_contains_source_file(
            &empty_directory,
            "com.example",
            Some(&deadline),
        )
        .expect_err("deadline should stop after opening an empty Kotlin directory");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during extracting local file dependencies"),
            "{error:#}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_file_dependency_extraction_honors_deadline_during_directory_scan() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.kt");
        let candidate_path = root.join("src/com/example/Helper.kt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(&candidate_path, "package com.example\n\nclass Helper\n").unwrap();
        let source = "package com.example\n\nimport com.example.*\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).expect("Kotlin source should parse");
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 4,
        };

        let error = kotlin_local_file_dependency_paths_with_deadline(
            &source_path,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("dependency extraction should stop while enumerating source candidates");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during extracting local file dependencies")
        );
        assert!(deadline.checks.get() >= 5);
        let _ = fs::remove_dir_all(root);
    }

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
    fn resolves_kotlin_script_imports_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.kt");
        let helper_path = root.join("src/com/example/Helper.kts");
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
    fn rejects_ambiguous_kotlin_script_and_source_imports() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.kt");
        let source_helper_path = root.join("src/com/example/Helper.kt");
        let script_helper_path = root.join("src/com/example/Helper.kts");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.example\n\nimport com.example.Helper\n\nclass Main\n",
        )
        .unwrap();
        fs::write(&source_helper_path, "package com.example\n\nclass Helper\n").unwrap();
        fs::write(&script_helper_path, "package com.example\n\nclass Helper\n").unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            kotlin_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert!(dependencies.is_empty());
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
    fn resolves_unique_wildcard_import_packages_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/app/Main.kt");
        let helper_path = root.join("src/com/example/Helper.kt");
        let other_path = root.join("src/com/example/Other.kts");
        let unrelated_path = root.join("src/com/other/Unrelated.kt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(helper_path.parent().unwrap()).unwrap();
        fs::create_dir_all(unrelated_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.app\n\nimport com.example.*\n\nclass Main\n",
        )
        .unwrap();
        fs::write(&helper_path, "package com.example\n\nclass Helper\n").unwrap();
        fs::write(&other_path, "package com.example\n\nclass Other\n").unwrap();
        fs::write(&unrelated_path, "package com.other\n\nclass Unrelated\n").unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            kotlin_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(
            dependencies,
            [helper_path, other_path].into_iter().collect()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_ambiguous_wildcard_import_packages() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.kt");
        let first_helper = root.join("src/com/example/Helper.kt");
        let second_helper = root.join("src/com/example/com/example/Helper.kt");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(second_helper.parent().unwrap()).unwrap();
        fs::write(&first_helper, "package com.example\n\nclass Helper\n").unwrap();
        fs::write(&second_helper, "package com.example\n\nclass Helper\n").unwrap();
        let source = "package com.example\n\nimport com.example.*\n";
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
