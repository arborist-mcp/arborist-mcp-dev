use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::language::{
    LanguageCapabilities, MAX_SOURCE_FILE_BYTES, builtin_language_registry, parse_document,
};
use crate::model::LanguageId;

/// Common adapter contract invariants (design doc §17.1).
///
/// These checks are language-agnostic and run against every registered
/// language so a new adapter cannot regress the shared safety gates.
fn registered_languages() -> Vec<LanguageId> {
    builtin_language_registry().language_ids().collect()
}

fn sample_path(language_id: LanguageId) -> std::path::PathBuf {
    let registry = builtin_language_registry();
    let descriptor = registry
        .descriptor(language_id)
        .expect("every registered language must have a descriptor");
    let extension = descriptor.extensions[0];
    Path::new("sample").with_extension(extension)
}

#[test]
fn every_registered_language_detects_back_from_its_declared_extensions() {
    let registry = builtin_language_registry();
    for language_id in registered_languages() {
        let descriptor = registry
            .descriptor(language_id)
            .expect("every registered language must have a descriptor");
        assert!(
            !descriptor.extensions.is_empty(),
            "{language_id:?} must declare at least one extension"
        );
        for extension in descriptor.extensions {
            let detected = registry
                .language_for_extension(extension)
                .expect("declared extension must route to a language");
            assert_eq!(
                detected, language_id,
                "extension {extension:?} must route to {language_id:?}"
            );
        }
    }
}

#[test]
fn declared_extensions_are_disjoint_across_languages() {
    let registry = builtin_language_registry();
    let mut seen = BTreeSet::new();
    for language_id in registered_languages() {
        let descriptor = registry
            .descriptor(language_id)
            .expect("every registered language must have a descriptor");
        for extension in descriptor.extensions {
            assert!(
                seen.insert(extension),
                "extension {extension:?} is declared by more than one language"
            );
        }
    }
}

fn malformed_source(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => {
            "def broken(:
    return
"
        }
        LanguageId::C => {
            "int broken( {
    return 0;
}
"
        }
        LanguageId::Cpp => {
            "class Broken {
    int value
};
"
        }
        LanguageId::CSharp => {
            "class Broken {
    int value
}
"
        }
        LanguageId::JavaScript => {
            "export function broken( {
    return;
}
"
        }
        LanguageId::TypeScript => {
            "export function broken(: number {
    return 0;
}
"
        }
        LanguageId::Tsx => {
            "export function Broken( {
    return <div>;
}
"
        }
        LanguageId::Rust => {
            "pub fn broken(i32 {
    value
}
"
        }
        LanguageId::Go => {
            "package demo

func broken( {
    return 0
}
"
        }
        LanguageId::Java => {
            "package demo;

public final class Broken {
    int value
}
"
        }
        LanguageId::Kotlin => {
            "fun broken( {
    return
}
"
        }
    }
}

#[test]
fn malformed_sources_parse_without_panicking_for_every_language() {
    for language_id in registered_languages() {
        let path = sample_path(language_id);
        let source = malformed_source(language_id);
        parse_document(&path, source).unwrap_or_else(|error| {
            panic!("{language_id:?} must tolerate malformed source: {error}")
        });
    }
}

#[test]
fn oversized_sources_are_rejected_before_parsing() {
    let oversized = "x".repeat(MAX_SOURCE_FILE_BYTES as usize + 1);
    for language_id in registered_languages() {
        let path = sample_path(language_id);
        let error = match parse_document(&path, &oversized) {
            Ok(_) => panic!("{language_id:?} must reject an oversized source"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("source text too large"),
            "{language_id:?} rejection must mention the size limit: {error}"
        );
    }
}

fn sample_source(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => "def compute(value: int) -> int:\n    return value + 1\n",
        LanguageId::C => "int compute(int value) { return value + 1; }\n",
        LanguageId::Cpp => "int compute(int value) { return value + 1; }\n",
        LanguageId::CSharp => {
            "namespace Demo {\n    public static class DemoClass {\n        public static int Compute(int value) => value + 1;\n    }\n}\n"
        }
        LanguageId::JavaScript => "export function compute(value) {\n    return value + 1;\n}\n",
        LanguageId::TypeScript => {
            "export function compute(value: number): number {\n    return value + 1;\n}\n"
        }
        LanguageId::Tsx => {
            "export function compute(value: number) {\n    return <div>{value}</div>;\n}\n"
        }
        LanguageId::Rust => "pub fn compute(value: i32) -> i32 {\n    value + 1\n}\n",
        LanguageId::Go => {
            "package demo\n\nfunc compute(value int) int {\n    return value + 1\n}\n"
        }
        LanguageId::Java => {
            "package demo;\n\npublic final class Demo {\n    public static int compute(int value) {\n        return value + 1;\n    }\n}\n"
        }
        LanguageId::Kotlin => "package demo\n\nfun compute(value: Int): Int = value + 1\n",
    }
}

#[test]
fn every_language_builds_a_valid_semantic_skeleton() {
    use crate::semantic::get_semantic_skeleton_with_deadline;

    for language_id in registered_languages() {
        let path = sample_path(language_id);
        let source = sample_source(language_id);
        let document = parse_document(&path, source)
            .unwrap_or_else(|error| panic!("{language_id:?} sample must parse: {error}"));
        let skeleton = get_semantic_skeleton_with_deadline(
            &path,
            language_id,
            source,
            &document.tree,
            64,
            &[],
            None,
        )
        .unwrap_or_else(|error| panic!("{language_id:?} must build a semantic skeleton: {error}"));
        skeleton
            .validate_public_output()
            .unwrap_or_else(|error| panic!("{language_id:?} skeleton must be valid: {error}"));
        assert!(
            !skeleton.available_symbols.is_empty(),
            "{language_id:?} sample must produce at least one skeleton symbol"
        );
        for symbol in &skeleton.available_symbols {
            let (start, end) = symbol.byte_range;
            assert!(
                start <= end && end <= source.len(),
                "{language_id:?} skeleton symbol {symbol:?} has out-of-bounds byte range {start}..{end} for {} bytes",
                source.len()
            );
        }
    }
}

#[test]
fn symbol_extraction_is_stable_across_unchanged_source() {
    use crate::symbol_extractor::index_symbols_from_document;

    for language_id in registered_languages() {
        let path = sample_path(language_id);
        let source = sample_source(language_id);
        let document = parse_document(&path, source)
            .unwrap_or_else(|error| panic!("{language_id:?} sample must parse: {error}"));
        let first = index_symbols_from_document(&path, source, &document)
            .unwrap_or_else(|error| panic!("{language_id:?} must extract symbols: {error}"));
        let second = index_symbols_from_document(&path, source, &document)
            .unwrap_or_else(|error| panic!("{language_id:?} must extract symbols: {error}"));
        assert!(
            !first.is_empty(),
            "{language_id:?} sample must produce at least one indexed symbol"
        );
        let first_ids: Vec<&str> = first
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect();
        let second_ids: Vec<&str> = second
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect();
        assert_eq!(
            first_ids, second_ids,
            "{language_id:?} symbol IDs must be stable across unchanged source"
        );
    }
}

#[test]
fn extracted_symbols_satisfy_range_and_name_invariants() {
    use crate::symbol_extractor::index_symbols_from_document;

    for language_id in registered_languages() {
        let path = sample_path(language_id);
        let source = sample_source(language_id);
        let document = parse_document(&path, source)
            .unwrap_or_else(|error| panic!("{language_id:?} sample must parse: {error}"));
        let symbols = index_symbols_from_document(&path, source, &document)
            .unwrap_or_else(|error| panic!("{language_id:?} must extract symbols: {error}"));
        for symbol in &symbols {
            assert!(
                !symbol.semantic_path.is_empty(),
                "{language_id:?} symbol semantic path must not be blank"
            );
            assert!(
                !symbol.base_name.is_empty(),
                "{language_id:?} symbol base name must not be blank"
            );
            assert!(
                !symbol.file_path.is_empty(),
                "{language_id:?} symbol file path must not be blank"
            );
            let (start, end) = symbol.byte_range;
            assert!(
                start <= end && end <= source.len(),
                "{language_id:?} symbol {symbol:?} has out-of-bounds byte range {start}..{end} for {} bytes",
                source.len()
            );
            if let Some(signature) = &symbol.signature {
                assert!(
                    !signature.is_empty(),
                    "{language_id:?} symbol signature must not be blank"
                );
            }
        }
    }
}

#[test]
fn capability_denial_is_enforced_for_unadvertised_capabilities() {
    let registry = builtin_language_registry();
    let capabilities = [
        LanguageCapabilities::TREE_QUERY,
        LanguageCapabilities::SEMANTIC_SKELETON,
        LanguageCapabilities::SYMBOL_INDEX,
        LanguageCapabilities::FILE_DEPENDENCIES,
        LanguageCapabilities::REFERENCE_TRACE,
        LanguageCapabilities::PATCH_TARGETING,
        LanguageCapabilities::PATCH_VALIDATION,
    ];

    for language_id in registered_languages() {
        let descriptor = registry
            .descriptor(language_id)
            .expect("every registered language must have a descriptor");
        for capability in capabilities {
            let advertised = descriptor.capabilities.contains(capability);
            match registry.require_capability(language_id, capability, "contract probe") {
                Ok(()) => {
                    assert!(
                        advertised,
                        "{language_id:?} must deny an unadvertised capability"
                    );
                }
                Err(error) => {
                    assert!(
                        !advertised,
                        "{language_id:?} must accept an advertised capability"
                    );
                    assert!(
                        error.to_string().contains("does not support"),
                        "{language_id:?} denial must explain the missing capability: {error}"
                    );
                }
            }
        }
    }
}
#[test]
fn indexed_symbols_are_reused_across_rebuilds_for_every_language() {
    use crate::symbols::rebuild_symbol_index;

    let dir = super::support::temporary_dir();
    let mut expected_files = 0;
    for language_id in registered_languages() {
        let path = sample_path(language_id);
        let source = sample_source(language_id);
        let file_name = path.file_name().expect("sample path must have a file name");
        fs::write(dir.join(file_name), source)
            .unwrap_or_else(|error| panic!("{language_id:?} sample must be writable: {error}"));
        expected_files += 1;
    }

    let db_path = dir.join("symbols.db");
    let first = rebuild_symbol_index(&dir, &db_path)
        .unwrap_or_else(|error| panic!("first rebuild must succeed: {error}"));
    assert_eq!(first.indexed_files, expected_files);
    assert_eq!(first.rebuilt_files, expected_files);
    assert_eq!(first.reused_files, 0);
    assert!(
        first.indexed_symbols >= expected_files,
        "expected at least one symbol per language sample, got {}",
        first.indexed_symbols
    );

    let second = rebuild_symbol_index(&dir, &db_path)
        .unwrap_or_else(|error| panic!("second rebuild must succeed: {error}"));
    assert_eq!(second.indexed_files, expected_files);
    assert_eq!(second.rebuilt_files, 0);
    assert_eq!(second.reused_files, expected_files);
    assert_eq!(
        second.indexed_symbols, first.indexed_symbols,
        "unchanged sources must produce identical symbol counts"
    );
}
