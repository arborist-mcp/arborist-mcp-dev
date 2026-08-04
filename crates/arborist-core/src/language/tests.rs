use std::path::Path;

use tree_sitter::Point;

use super::{
    LanguageCapabilities, MAX_SOURCE_FILE_BYTES, builtin_language_registry,
    c_companion_source_path, detect_language, is_c_header_path, normalize_absolute_path,
    offset_for_position, parse_document, point_for_offset, read_source, supported_languages,
};
use crate::model::{LanguageId, Position};

#[test]
fn detect_language_accepts_uppercase_extensions() {
    for (extension, expected_language) in [
        ("PY", LanguageId::Python),
        ("PYI", LanguageId::Python),
        ("C", LanguageId::C),
        ("H", LanguageId::C),
        ("CC", LanguageId::Cpp),
        ("CPP", LanguageId::Cpp),
        ("CXX", LanguageId::Cpp),
        ("C++", LanguageId::Cpp),
        ("TPP", LanguageId::Cpp),
        ("TCC", LanguageId::Cpp),
        ("IPP", LanguageId::Cpp),
        ("INL", LanguageId::Cpp),
        ("HPP", LanguageId::Cpp),
        ("HH", LanguageId::Cpp),
        ("HXX", LanguageId::Cpp),
        ("H++", LanguageId::Cpp),
        ("JS", LanguageId::JavaScript),
        ("JSX", LanguageId::JavaScript),
        ("MJS", LanguageId::JavaScript),
        ("CJS", LanguageId::JavaScript),
        ("TS", LanguageId::TypeScript),
        ("MTS", LanguageId::TypeScript),
        ("CTS", LanguageId::TypeScript),
        ("TSX", LanguageId::Tsx),
        ("RS", LanguageId::Rust),
    ] {
        assert_eq!(
            detect_language(Path::new(&format!("sample.{extension}"))).unwrap(),
            expected_language,
            "unexpected language for .{extension}",
        );
    }
}

#[test]
fn supported_languages_reports_cpp() {
    assert_eq!(
        supported_languages(),
        vec![
            "python",
            "c",
            "cpp",
            "javascript",
            "typescript",
            "tsx",
            "rust",
        ]
    );
}

#[test]
fn language_ids_use_stable_serde_names() {
    for (language_id, expected_name) in [
        (LanguageId::Python, "python"),
        (LanguageId::C, "c"),
        (LanguageId::Cpp, "cpp"),
        (LanguageId::JavaScript, "javascript"),
        (LanguageId::TypeScript, "typescript"),
        (LanguageId::Tsx, "tsx"),
        (LanguageId::Rust, "rust"),
    ] {
        assert_eq!(
            serde_json::to_string(&language_id).unwrap(),
            format!("\"{expected_name}\"")
        );
        assert_eq!(
            serde_json::from_str::<LanguageId>(&format!("\"{expected_name}\"")).unwrap(),
            language_id,
        );
    }
}

#[test]
fn builtin_registry_preserves_current_language_contracts() {
    let registry = builtin_language_registry();

    for (language_id, display_name, extensions, analysis_revision) in [
        (
            LanguageId::Python,
            "Python",
            &["py", "pyi"][..],
            "python-v1",
        ),
        (LanguageId::C, "C", &["c", "h"][..], "c-v1"),
        (
            LanguageId::Cpp,
            "C++",
            &[
                "cc", "cpp", "cxx", "c++", "tpp", "tcc", "ipp", "inl", "hpp", "hh", "hxx", "h++",
            ][..],
            "cpp-v1",
        ),
    ] {
        let descriptor = registry
            .descriptor(language_id)
            .expect("each supported language must have a descriptor");

        assert_eq!(descriptor.display_name, display_name);
        assert_eq!(descriptor.extensions, extensions);
        assert_eq!(descriptor.analysis_revision, analysis_revision);
        assert!(
            descriptor
                .capabilities
                .contains(LanguageCapabilities::FULL_CURRENT_SUPPORT),
            "{display_name} should retain all existing capabilities",
        );
        for extension in extensions {
            assert_eq!(
                registry.language_for_extension(&extension.to_ascii_uppercase()),
                Some(language_id),
                "registry should preserve case-insensitive .{extension} routing",
            );
        }
    }

    assert_eq!(registry.language_for_extension("txt"), None);
}

#[test]
fn javascript_and_typescript_adapters_expose_dependency_capabilities() {
    let registry = builtin_language_registry();

    for (language_id, display_name, extensions, analysis_revision) in [
        (
            LanguageId::JavaScript,
            "JavaScript",
            &["js", "jsx", "mjs", "cjs"][..],
            "javascript-patching-v1",
        ),
        (
            LanguageId::TypeScript,
            "TypeScript",
            &["ts", "mts", "cts"][..],
            "typescript-patching-v1",
        ),
        (LanguageId::Tsx, "TSX", &["tsx"][..], "tsx-patching-v1"),
    ] {
        let descriptor = registry.descriptor(language_id).unwrap();
        assert_eq!(descriptor.display_name, display_name);
        assert_eq!(descriptor.extensions, extensions);
        assert_eq!(descriptor.analysis_revision, analysis_revision);
        assert!(
            descriptor
                .capabilities
                .contains(LanguageCapabilities::TREE_QUERY)
        );
        assert!(
            descriptor
                .capabilities
                .contains(LanguageCapabilities::SYMBOL_INDEX)
        );
        assert!(
            descriptor
                .capabilities
                .contains(LanguageCapabilities::SEMANTIC_SKELETON)
        );
        assert!(
            descriptor
                .capabilities
                .contains(LanguageCapabilities::REFERENCE_TRACE)
        );
        assert!(
            descriptor
                .capabilities
                .contains(LanguageCapabilities::FILE_DEPENDENCIES)
        );
        assert!(
            descriptor
                .capabilities
                .contains(LanguageCapabilities::PATCH_TARGETING)
        );
        assert!(
            descriptor
                .capabilities
                .contains(LanguageCapabilities::PATCH_VALIDATION)
        );
        for extension in extensions {
            assert_eq!(
                registry.language_for_extension(&extension.to_ascii_uppercase()),
                Some(language_id)
            );
        }
    }
}

#[test]
fn rust_adapter_exposes_skeleton_indexing_dependencies_and_tracing_without_patching() {
    let registry = builtin_language_registry();
    let descriptor = registry.descriptor(LanguageId::Rust).unwrap();

    assert_eq!(descriptor.display_name, "Rust");
    assert_eq!(descriptor.extensions, &["rs"]);
    assert_eq!(descriptor.analysis_revision, "rust-trace-v2");
    for capability in [
        LanguageCapabilities::TREE_QUERY,
        LanguageCapabilities::SEMANTIC_SKELETON,
        LanguageCapabilities::SYMBOL_INDEX,
        LanguageCapabilities::FILE_DEPENDENCIES,
        LanguageCapabilities::REFERENCE_TRACE,
    ] {
        assert!(descriptor.capabilities.contains(capability));
    }
    for capability in [
        LanguageCapabilities::PATCH_TARGETING,
        LanguageCapabilities::PATCH_VALIDATION,
    ] {
        assert!(!descriptor.capabilities.contains(capability));
    }
}

#[test]
fn parse_document_uses_rust_grammar_and_recovers_from_invalid_source() {
    let document = parse_document(
        Path::new("sample.rs"),
        "pub fn add(left: i32, right: i32) -> i32 { left + right }",
    )
    .unwrap();
    assert_eq!(document.language_id, LanguageId::Rust);
    assert!(!document.tree.root_node().has_error());

    let malformed = parse_document(Path::new("broken.rs"), "fn broken( {").unwrap();
    assert_eq!(malformed.language_id, LanguageId::Rust);
    assert!(malformed.tree.root_node().has_error());
}

#[test]
fn parse_document_uses_javascript_and_typescript_grammars() {
    for (path, source, language_id) in [
        (
            "sample.js",
            "export function add(left, right) { return left + right; }",
            LanguageId::JavaScript,
        ),
        (
            "sample.ts",
            "export function add(left: number, right: number): number { return left + right; }",
            LanguageId::TypeScript,
        ),
        (
            "sample.tsx",
            "export const App = () => <main>ready</main>;",
            LanguageId::Tsx,
        ),
    ] {
        let document = parse_document(Path::new(path), source).unwrap();
        assert_eq!(document.language_id, language_id);
        assert!(
            !document.tree.root_node().has_error(),
            "{path} should parse"
        );
    }
}

#[test]
fn detect_language_reports_original_unsupported_extension() {
    let error = detect_language(Path::new("sample.TXT"))
        .expect_err("unsupported extensions should be reported");

    assert!(error.to_string().contains(r#"Some("TXT")"#));
}

#[test]
fn rejects_cpp_module_extensions_until_the_grammar_supports_module_declarations() {
    let error = detect_language(Path::new("sample.cppm"))
        .expect_err("C++ module extensions must remain unsupported until module syntax parses");

    assert!(error.to_string().contains(r#"Some("cppm")"#));
}

#[test]
fn c_header_detection_accepts_uppercase_extensions() {
    assert!(is_c_header_path(Path::new("sample.h")));
    assert!(is_c_header_path(Path::new("sample.H")));
    assert!(is_c_header_path(Path::new("sample.HPP")));
    assert!(is_c_header_path(Path::new("sample.HH")));
    assert!(!is_c_header_path(Path::new("sample.c")));
}

#[test]
fn parse_document_uses_cpp_grammar_for_cpp_extensions() {
    let source = "class Counter { public: int value() const { return 1; } };";
    for extension in ["hpp", "tpp", "tcc", "ipp", "inl"] {
        let document = parse_document(Path::new(&format!("counter.{extension}")), source).unwrap();

        assert_eq!(document.language_id, LanguageId::Cpp);
        assert!(!document.tree.root_node().has_error());
    }
}

#[test]
fn companion_c_source_prefers_header_case_style() {
    let dir = std::env::temp_dir().join(format!(
        "arborist-language-companion-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let uppercase_header = dir.join("helper.H");
    let uppercase_source = dir.join("helper.C");
    std::fs::write(&uppercase_header, "int helper(int value);\n").unwrap();
    std::fs::write(
        &uppercase_source,
        "int helper(int value) { return value + 1; }\n",
    )
    .unwrap();

    assert_eq!(
        c_companion_source_path(&uppercase_header).unwrap(),
        uppercase_source
    );

    let mixed_header = dir.join("mixed.HPP");
    let lowercase_source = dir.join("mixed.c");
    std::fs::write(&mixed_header, "int mixed(int value);\n").unwrap();
    std::fs::write(
        &lowercase_source,
        "int mixed(int value) { return value + 1; }\n",
    )
    .unwrap();

    assert_eq!(
        c_companion_source_path(&mixed_header).unwrap(),
        lowercase_source
    );

    let template_header = dir.join("template.hpp");
    let template_implementation = dir.join("template.tpp");
    std::fs::write(
        &template_header,
        "template <typename T> T value(T input);\n",
    )
    .unwrap();
    std::fs::write(
        &template_implementation,
        "template <typename T> T value(T input) { return input; }\n",
    )
    .unwrap();

    assert_eq!(c_companion_source_path(&template_header), None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn normalize_absolute_path_rejects_empty_paths() {
    let error = normalize_absolute_path(Path::new(""))
        .expect_err("empty paths should be rejected before normalization");

    assert!(error.to_string().contains("path"));
    assert!(error.to_string().contains("empty"));
}

#[test]
fn point_for_offset_uses_tree_sitter_byte_columns() {
    let source = "é\nx";

    assert_eq!(
        point_for_offset(source, "é".len()).unwrap(),
        Point { row: 0, column: 2 }
    );
    assert_eq!(
        point_for_offset(source, "é\n".len()).unwrap(),
        Point { row: 1, column: 0 }
    );
}

#[test]
fn offset_for_position_uses_tree_sitter_byte_columns() {
    let source = "é\nx";

    assert_eq!(
        offset_for_position(source, &Position { row: 0, column: 2 }).unwrap(),
        "é".len()
    );
    assert_eq!(
        offset_for_position(source, &Position { row: 1, column: 1 }).unwrap(),
        source.len()
    );
}

#[test]
fn offset_for_position_rejects_non_boundary_byte_columns() {
    let source = "é\nx";

    let error = offset_for_position(source, &Position { row: 0, column: 1 })
        .expect_err("positions inside a UTF-8 character should be rejected");

    assert!(
        error
            .to_string()
            .contains("does not align to a UTF-8 character boundary")
    );
}

#[test]
fn read_source_rejects_oversized_files_before_loading_contents() {
    let path = std::env::temp_dir().join(format!(
        "arborist-language-oversized-source-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(MAX_SOURCE_FILE_BYTES + 1).unwrap();
    drop(file);

    let error = read_source(&path).expect_err("oversized source files should be rejected");
    assert!(error.to_string().contains("source file too large"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn parse_document_rejects_oversized_source_overlays() {
    let source = "x".repeat((MAX_SOURCE_FILE_BYTES + 1) as usize);
    let error = parse_document(Path::new("source.py"), &source)
        .err()
        .expect("oversized source overlays should be rejected");
    assert!(error.to_string().contains("source text too large"));
}
