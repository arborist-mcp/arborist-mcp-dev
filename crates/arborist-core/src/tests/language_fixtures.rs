use std::fs;
use std::path::{Path, PathBuf};

use crate::language::{detect_language, parse_document};
use crate::model::{LanguageId, TraceDirection};
use crate::symbol_index_model::symbol_base_name_ref;
use crate::symbols::{rebuild_symbol_index, search_symbols, trace_symbol_graph};

fn fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/languages")
        .canonicalize()
        .expect("language fixtures tree must exist")
}

fn expected_language_for_directory(name: &str) -> LanguageId {
    match name {
        "python" => LanguageId::Python,
        "c" => LanguageId::C,
        "cpp" => LanguageId::Cpp,
        "javascript" => LanguageId::JavaScript,
        "typescript" => LanguageId::TypeScript,
        "rust" => LanguageId::Rust,
        "go" => LanguageId::Go,
        "java" => LanguageId::Java,
        other => panic!("unexpected language fixture directory {other:?}"),
    }
}

#[test]
fn language_fixture_tree_has_expected_directories() {
    let root = fixtures_root();
    for directory in [
        "python",
        "c",
        "cpp",
        "javascript",
        "typescript",
        "rust",
        "go",
        "java",
    ] {
        let path = root.join(directory);
        assert!(
            path.is_dir(),
            "expected language fixture directory {}",
            path.display()
        );
    }
}

#[test]
fn language_fixtures_parse_as_expected_language() {
    let root = fixtures_root();
    for directory in fs::read_dir(&root).expect("fixtures root must be readable") {
        let directory = directory.expect("directory entry must be readable");
        if !directory.path().is_dir() {
            continue;
        }
        let directory_name = directory.file_name();
        let directory_name = directory_name
            .to_str()
            .expect("directory name must be UTF-8");
        let expected = expected_language_for_directory(directory_name);

        let mut parsed_any = false;
        let mut malformed_any = false;
        for entry in fs::read_dir(directory.path()).expect("fixture directory must be readable") {
            let entry = entry.expect("fixture entry must be readable");
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if file_name == "README.md" {
                continue;
            }
            let source = fs::read_to_string(&path).expect("fixture must be readable");
            let detected = detect_language(&path).expect("fixture extension must be supported");
            assert_eq!(
                detected,
                expected,
                "fixture {} detected as {detected:?}, expected {expected:?}",
                path.display()
            );
            if file_name.starts_with("malformed.") {
                malformed_any = true;
                continue;
            }
            parse_document(&path, &source)
                .unwrap_or_else(|error| panic!("fixture {} must parse: {error}", path.display()));
            parsed_any = true;
        }

        assert!(
            parsed_any,
            "fixture directory {directory_name:?} must contain at least one parseable fixture"
        );
        assert!(
            malformed_any,
            "fixture directory {directory_name:?} must contain a malformed fixture"
        );
    }
}

fn fixture_file(directory: &Path, prefix: &str) -> Option<PathBuf> {
    fs::read_dir(directory)
        .expect("fixture directory must be readable")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(prefix))
        })
}

fn primary_function_base_names() -> [&'static str; 3] {
    ["orchestrate", "Orchestrate", "compute"]
}

#[test]
fn language_fixtures_direct_calls_are_indexable_and_traceable() {
    let root = fixtures_root();
    for directory in fs::read_dir(&root).expect("fixtures root must be readable") {
        let directory = directory.expect("directory entry must be readable");
        if !directory.path().is_dir() {
            continue;
        }
        let directory_path = directory.path();
        let directory_name = directory
            .file_name()
            .to_str()
            .expect("directory name must be UTF-8")
            .to_owned();
        let Some(direct_calls) = fixture_file(&directory_path, "direct_calls.") else {
            panic!("{directory_name:?} must contain a direct_calls fixture");
        };
        let source = fs::read_to_string(&direct_calls)
            .unwrap_or_else(|error| panic!("direct_calls fixture must be readable: {error}"));

        let dir = super::support::temporary_dir();
        fs::write(dir.join(direct_calls.file_name().unwrap()), source).unwrap();
        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path)
            .unwrap_or_else(|error| panic!("{directory_name:?} direct_calls must index: {error}"));

        let mut function_path = None;
        for base_name in primary_function_base_names() {
            let results = search_symbols(&dir, base_name, 10)
                .unwrap_or_else(|error| panic!("{directory_name:?} search must succeed: {error}"));
            if let Some(function) = results.matches.iter().find(|symbol| {
                symbol_base_name_ref(&symbol.semantic_path).eq_ignore_ascii_case(base_name)
            }) {
                function_path = Some(function.semantic_path.clone());
                break;
            }
        }
        let function_path = function_path.unwrap_or_else(|| {
            panic!("{directory_name:?} direct_calls must yield a primary function")
        });

        let trace = trace_symbol_graph(&dir, &function_path, TraceDirection::Both)
            .unwrap_or_else(|error| panic!("{directory_name:?} trace must succeed: {error}"));
        assert_eq!(
            trace.symbol.semantic_path, function_path,
            "{directory_name:?} trace must target the primary function"
        );
    }
}

#[test]
fn language_fixtures_shadowing_and_overloads_are_indexable() {
    let root = fixtures_root();
    for directory in fs::read_dir(&root).expect("fixtures root must be readable") {
        let directory = directory.expect("directory entry must be readable");
        if !directory.path().is_dir() {
            continue;
        }
        let directory_path = directory.path();
        let directory_name = directory
            .file_name()
            .to_str()
            .expect("directory name must be UTF-8")
            .to_owned();

        if let Some(shadowing) = fixture_file(&directory_path, "shadowing.") {
            let source = fs::read_to_string(&shadowing)
                .unwrap_or_else(|error| panic!("shadowing fixture must be readable: {error}"));
            let dir = super::support::temporary_dir();
            fs::write(dir.join(shadowing.file_name().unwrap()), source).unwrap();
            rebuild_symbol_index(&dir, &dir.join("symbols.db"))
                .unwrap_or_else(|error| panic!("{directory_name:?} shadowing must index: {error}"));
            let found = primary_function_base_names().iter().any(|base_name| {
                search_symbols(&dir, base_name, 10)
                    .expect("search must succeed")
                    .matches
                    .iter()
                    .any(|symbol| {
                        symbol_base_name_ref(&symbol.semantic_path).eq_ignore_ascii_case(base_name)
                    })
            });
            assert!(
                found,
                "{directory_name:?} shadowing fixture must index its primary function"
            );
        }

        if let Some(overloads) = fixture_file(&directory_path, "overloads.") {
            let source = fs::read_to_string(&overloads)
                .unwrap_or_else(|error| panic!("overloads fixture must be readable: {error}"));
            let dir = super::support::temporary_dir();
            fs::write(dir.join(overloads.file_name().unwrap()), source).unwrap();
            rebuild_symbol_index(&dir, &dir.join("symbols.db"))
                .unwrap_or_else(|error| panic!("{directory_name:?} overloads must index: {error}"));
            let results = search_symbols(&dir, "helper", 10)
                .unwrap_or_else(|error| panic!("{directory_name:?} search must succeed: {error}"));
            let helpers = results
                .matches
                .iter()
                .filter(|symbol| symbol_base_name_ref(&symbol.semantic_path) == "helper")
                .count();
            assert!(
                helpers >= 2,
                "{directory_name:?} overloads fixture must index multiple helpers, got {helpers}"
            );
        }
    }
}
