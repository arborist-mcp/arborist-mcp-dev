use std::fs;
use std::path::{Path, PathBuf};

use crate::language::{detect_language, parse_document};
use crate::model::LanguageId;

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
