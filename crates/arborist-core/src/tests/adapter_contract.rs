use std::collections::BTreeSet;
use std::path::Path;

use crate::language::{MAX_SOURCE_FILE_BYTES, builtin_language_registry, parse_document};
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
