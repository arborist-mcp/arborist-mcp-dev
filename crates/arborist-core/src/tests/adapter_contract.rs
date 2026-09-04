use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::language::{
    LanguageCapabilities, MAX_SOURCE_FILE_BYTES, builtin_language_registry, parse_document,
    point_for_offset, position_from,
};
use crate::model::{LanguageId, TraceDirection};
use crate::symbol_index_model::symbol_base_name_ref;

/// Common adapter contract invariants (design doc §17.1).
///
/// These checks are language-agnostic and run against every registered
/// language so a new adapter cannot regress the shared safety gates.
fn registered_languages() -> Vec<LanguageId> {
    builtin_language_registry().language_ids().collect()
}

fn advertises_capability(language_id: LanguageId, required: LanguageCapabilities) -> bool {
    builtin_language_registry()
        .descriptor(language_id)
        .expect("every registered language must have a descriptor")
        .capabilities
        .contains(required)
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
        LanguageId::Lua => {
            "local function broken(
    return
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

#[test]
fn parse_deadlines_are_enforced_for_every_language() {
    use crate::language::parse_document_with_timeout;

    let source = "(".repeat(128 * 1024);
    for language_id in registered_languages() {
        let path = sample_path(language_id);
        let error = match parse_document_with_timeout(&path, &source, 1) {
            Ok(_) => panic!(
                "{language_id:?} large source must not outlive a one-microsecond parse budget"
            ),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("timed out"),
            "{language_id:?} parse timeout must be reported explicitly: {error}"
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
        LanguageId::Lua => "local function compute(value)\n    return value + 1\nend\n",
    }
}

fn patch_symbol_base_name(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::CSharp => "Compute",
        _ => "compute",
    }
}

fn successful_patch_replacement(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => "def compute(value: int) -> int:\n    return value + 2\n",
        LanguageId::C | LanguageId::Cpp => "int compute(int value) { return value + 2; }\n",
        LanguageId::CSharp => "public static int Compute(int value) => value + 2;\n",
        LanguageId::JavaScript => "export function compute(value) {\n    return value + 2;\n}\n",
        LanguageId::TypeScript => {
            "export function compute(value: number): number {\n    return value + 2;\n}\n"
        }
        LanguageId::Tsx => {
            "export function compute(value: number) {\n    return <span>{value + 2}</span>;\n}\n"
        }
        LanguageId::Rust => "pub fn compute(value: i32) -> i32 {\n    value + 2\n}\n",
        LanguageId::Go => "func compute(value int) int {\n\treturn value + 2\n}\n",
        LanguageId::Java => {
            "public static int compute(int value) {\n        return value + 2;\n    }\n"
        }
        LanguageId::Kotlin => "fun compute(value: Int): Int = value + 2\n",
        LanguageId::Lua => unreachable!("Lua does not advertise this capability"),
    }
}

fn unresolved_reference_patch_replacement(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => "def compute(value: int) -> int:\n    return missing(value)\n",
        LanguageId::C | LanguageId::Cpp => "int compute(int value) { return missing(value); }\n",
        LanguageId::CSharp => "public static int Compute(int value) => Missing(value);\n",
        LanguageId::JavaScript => {
            "export function compute(value) {\n    return missing(value);\n}\n"
        }
        LanguageId::TypeScript => {
            "export function compute(value: number): number {\n    return missing(value);\n}\n"
        }
        LanguageId::Tsx => {
            "export function compute(value: number) {\n    return <span>{missing(value)}</span>;\n}\n"
        }
        LanguageId::Rust => "pub fn compute(value: i32) -> i32 {\n    missing(value)\n}\n",
        LanguageId::Go => "func compute(value int) int {\n\treturn missing(value)\n}\n",
        LanguageId::Java => {
            "public static int compute(int value) {\n        return missing(value);\n    }\n"
        }
        LanguageId::Kotlin => "fun compute(value: Int): Int = missing(value)\n",
        LanguageId::Lua => unreachable!("Lua does not advertise this capability"),
    }
}

fn trace_contract_source(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/python/resolver_direct_calls.py"
        )),
        LanguageId::C => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/c/resolver_direct_calls.c"
        )),
        LanguageId::Cpp => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/cpp/resolver_direct_calls.cpp"
        )),
        LanguageId::CSharp => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/csharp/resolver_direct_calls.cs"
        )),
        LanguageId::JavaScript => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/javascript/resolver_direct_calls.js"
        )),
        LanguageId::TypeScript => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/typescript/resolver_direct_calls.ts"
        )),
        LanguageId::Tsx => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/tsx/resolver_direct_calls.tsx"
        )),
        LanguageId::Rust => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/rust/resolver_direct_calls.rs"
        )),
        LanguageId::Go => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/go/resolver_direct_calls.go"
        )),
        LanguageId::Java => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/java/resolver_direct_calls.java"
        )),
        LanguageId::Kotlin => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/kotlin/resolver_direct_calls.kt"
        )),
        LanguageId::Lua => unreachable!("Lua does not advertise this capability"),
    }
}

fn unresolved_trace_contract_source(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/python/resolver_unresolved_calls.py"
        )),
        LanguageId::C => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/c/resolver_unresolved_calls.c"
        )),
        LanguageId::Cpp => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/cpp/resolver_unresolved_calls.cpp"
        )),
        LanguageId::CSharp => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/csharp/resolver_unresolved_calls.cs"
        )),
        LanguageId::JavaScript => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/javascript/resolver_unresolved_calls.js"
        )),
        LanguageId::TypeScript => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/typescript/resolver_unresolved_calls.ts"
        )),
        LanguageId::Tsx => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/tsx/resolver_unresolved_calls.tsx"
        )),
        LanguageId::Rust => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/rust/resolver_unresolved_calls.rs"
        )),
        LanguageId::Go => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/go/resolver_unresolved_calls.go"
        )),
        LanguageId::Java => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/java/resolver_unresolved_calls.java"
        )),
        LanguageId::Kotlin => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/languages/kotlin/resolver_unresolved_calls.kt"
        )),
        LanguageId::Lua => unreachable!("Lua does not advertise this capability"),
    }
}

fn cross_language_trace_contract_source(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => "def caller(value: int) -> int:\n    return compute(value)\n",
        LanguageId::C | LanguageId::Cpp => "int caller(int value) { return compute(value); }\n",
        LanguageId::CSharp => {
            "namespace Demo { public static class DemoClass { public static int Orchestrate(int value) => Compute(value); } }\n"
        }
        LanguageId::JavaScript => "export function caller(value) { return compute(value); }\n",
        LanguageId::TypeScript => {
            "export function caller(value: number): number { return compute(value); }\n"
        }
        LanguageId::Tsx => {
            "export function caller(value: number) { return <div>{compute(value)}</div>; }\n"
        }
        LanguageId::Rust => "pub fn caller(value: i32) -> i32 { compute(value) }\n",
        LanguageId::Go => "package demo\n\nfunc caller(value int) int { return compute(value) }\n",
        LanguageId::Java => {
            "package demo; public final class Demo { public static int caller(int value) { return compute(value); } }\n"
        }
        LanguageId::Kotlin => "package demo\n\nfun caller(value: Int): Int = compute(value)\n",
        LanguageId::Lua => {
            "--[[ café ]] local function compute(value)\n    return value + 1\nend\n"
        }
    }
}

fn foreign_python_compute_source() -> &'static str {
    "def compute(value: int) -> int:\n    return value + 1\n"
}

fn foreign_c_compute_source() -> &'static str {
    "int compute(int value) { return value + 1; }\n"
}

fn ambiguous_trace_contract_files(
    language_id: LanguageId,
) -> Option<Vec<(&'static str, &'static str)>> {
    match language_id {
        LanguageId::JavaScript => Some(vec![
            (
                "ambiguity_left.js",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/javascript/ambiguity_left.js"
                )),
            ),
            (
                "ambiguity_right.js",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/javascript/ambiguity_right.js"
                )),
            ),
            (
                "ambiguity_reexport.js",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/javascript/ambiguity_reexport.js"
                )),
            ),
            (
                "resolver_ambiguous_calls.js",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/javascript/resolver_ambiguous_calls.js"
                )),
            ),
        ]),
        LanguageId::TypeScript => Some(vec![
            (
                "ambiguity_left.ts",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/typescript/ambiguity_left.ts"
                )),
            ),
            (
                "ambiguity_right.ts",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/typescript/ambiguity_right.ts"
                )),
            ),
            (
                "ambiguity_reexport.ts",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/typescript/ambiguity_reexport.ts"
                )),
            ),
            (
                "resolver_ambiguous_calls.ts",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/typescript/resolver_ambiguous_calls.ts"
                )),
            ),
        ]),
        LanguageId::Tsx => Some(vec![
            (
                "ambiguity_left.tsx",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/tsx/ambiguity_left.tsx"
                )),
            ),
            (
                "ambiguity_right.tsx",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/tsx/ambiguity_right.tsx"
                )),
            ),
            (
                "ambiguity_reexport.tsx",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/tsx/ambiguity_reexport.tsx"
                )),
            ),
            (
                "resolver_ambiguous_calls.tsx",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/tests/fixtures/languages/tsx/resolver_ambiguous_calls.tsx"
                )),
            ),
        ]),
        _ => None,
    }
}

fn trace_contract_symbol_base_name(language_id: LanguageId, caller: bool) -> &'static str {
    match (language_id, caller) {
        (LanguageId::CSharp, true) => "Orchestrate",
        (LanguageId::CSharp, false) => "Compute",
        (_, true) => "caller",
        (_, false) => "compute",
    }
}

fn overlay_source(language_id: LanguageId, source: &str) -> String {
    match language_id {
        LanguageId::Tsx => source.replace("{value}", "{value + 2}"),
        _ => source.replace("value + 1", "value + 2"),
    }
}

fn semantic_target_for_symbol_base(
    language_id: LanguageId,
    path: &Path,
    source: &str,
    base_name: &str,
) -> String {
    use crate::symbol_extractor::index_symbols_from_document;

    let document = parse_document(path, source)
        .unwrap_or_else(|error| panic!("{language_id:?} contract sample must parse: {error}"));
    index_symbols_from_document(path, source, &document)
        .unwrap_or_else(|error| panic!("{language_id:?} contract sample must index: {error}"))
        .into_iter()
        .find(|symbol| symbol.base_name == base_name)
        .map(|symbol| symbol.semantic_path)
        .unwrap_or_else(|| panic!("{language_id:?} contract sample must expose {base_name}"))
}

fn utf8_position_contract_source(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => "def compute(value: int) -> int:\n    return value + len(\"é\")\n",
        LanguageId::C | LanguageId::Cpp => {
            "/* café */ int compute(int value) { return value + 1; }\n"
        }
        LanguageId::CSharp => {
            "namespace Demo { public static class DemoClass { /* café */ public static int Compute(int value) => value + 1; } }\n"
        }
        LanguageId::JavaScript => {
            "/* café */ export function compute(value) { return value + 1; }\n"
        }
        LanguageId::TypeScript => {
            "/* café */ export function compute(value: number): number { return value + 1; }\n"
        }
        LanguageId::Tsx => {
            "/* café */ export function compute(value: number) { return <div>{value}</div>; }\n"
        }
        LanguageId::Rust => "/* café */ pub fn compute(value: i32) -> i32 { value + 1 }\n",
        LanguageId::Go => {
            "package demo\n\n/* café */ func compute(value int) int { return value + 1 }\n"
        }
        LanguageId::Java => {
            "/* café */ package demo; public final class Demo { public static int compute(int value) { return value + 1; } }\n"
        }
        LanguageId::Kotlin => {
            "package demo\n\n/* café */ fun compute(value: Int): Int = value + 1\n"
        }
        LanguageId::Lua => {
            "--[[ café ]] local function compute(value)\n    return value + 1\nend\n"
        }
    }
}

fn utf8_position_contract_offset(language_id: LanguageId, source: &str) -> usize {
    if language_id == LanguageId::Python {
        source
            .find("é")
            .expect("Python UTF-8 fixture must contain é")
            + "é".len()
    } else {
        source
            .rfind(utf8_position_contract_symbol_base_name(language_id))
            .expect("UTF-8 fixture must contain its symbol name")
    }
}

fn utf8_position_contract_symbol_base_name(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::CSharp => "Compute",
        _ => "compute",
    }
}

fn semantic_target_for_patch_contract(
    language_id: LanguageId,
    path: &Path,
    source: &str,
) -> String {
    semantic_target_for_symbol_base(
        language_id,
        path,
        source,
        patch_symbol_base_name(language_id),
    )
}

#[test]
fn every_language_builds_a_valid_semantic_skeleton() {
    use crate::semantic::get_semantic_skeleton_with_deadline;

    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::SEMANTIC_SKELETON) {
            continue;
        }
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
fn c_symbol_family_anchor_parsing_rejects_phase_rejecting_deadlines() {
    use crate::semantic::get_semantic_skeleton_with_deadline;
    use crate::tests::temporary_dir;

    struct RejectPhase(&'static str);

    impl crate::deadline::DeadlineCheck for RejectPhase {
        fn check(&self, phase: &str) -> anyhow::Result<()> {
            if phase == self.0 {
                anyhow::bail!("deadline check reached {phase}");
            }
            Ok(())
        }

        fn remaining_timeout_micros(&self, phase: &str) -> anyhow::Result<Option<u64>> {
            anyhow::bail!("deadline budget requested during {phase}");
        }
    }

    let dir = temporary_dir();
    let header = dir.join("helper.h");
    let source_path = dir.join("helper.c");
    fs::write(&header, "int helper(int value);\n").unwrap();
    fs::write(
        &source_path,
        "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
    )
    .unwrap();

    let source = fs::read_to_string(&source_path).unwrap();
    let document = parse_document(&source_path, &source).unwrap();
    let deadline = RejectPhase("parsing C/C++ symbol identity");
    let error = get_semantic_skeleton_with_deadline(
        &source_path,
        document.language_id,
        &source,
        &document.tree,
        64,
        &[],
        Some(&deadline),
    )
    .expect_err("included-header symbol parsing must honor the deadline");

    assert!(
        error
            .to_string()
            .contains("deadline budget requested during parsing C/C++ symbol identity")
    );
}

#[test]
fn symbol_extraction_is_stable_across_unchanged_source() {
    use crate::symbol_extractor::index_symbols_from_document;

    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
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
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
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
fn utf8_positions_resolve_symbols_consistently_for_every_language() {
    use crate::{
        read_symbol_at_position, read_symbol_at_position_from_index, rebuild_symbol_index,
    };

    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
        let path = sample_path(language_id);
        let source = utf8_position_contract_source(language_id);
        let symbol_name = utf8_position_contract_symbol_base_name(language_id);
        let symbol_offset = utf8_position_contract_offset(language_id, source);
        let position = position_from(
            point_for_offset(source, symbol_offset).unwrap_or_else(|error| {
                panic!("{language_id:?} position conversion failed: {error}")
            }),
        );
        let line_start = source[..symbol_offset]
            .rfind('\n')
            .map_or(0, |index| index + 1);
        let line_prefix = &source[line_start..symbol_offset];
        assert!(
            position.column > line_prefix.chars().count(),
            "{language_id:?} position must use UTF-8 byte columns: byte {}, chars {}",
            position.column,
            line_prefix.chars().count()
        );

        let dir = super::support::temporary_dir();
        let file_path = dir.join(path.file_name().expect("sample path must have a file name"));
        fs::write(&file_path, source).unwrap();
        let live = read_symbol_at_position(&dir, &file_path, &position).unwrap_or_else(|error| {
            panic!("{language_id:?} live UTF-8 position read failed: {error}")
        });
        assert_eq!(
            symbol_base_name_ref(&live.symbol.semantic_path),
            symbol_name,
            "{language_id:?} live position read must resolve the expected symbol"
        );

        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path)
            .unwrap_or_else(|error| panic!("{language_id:?} index rebuild failed: {error}"));
        let persisted = read_symbol_at_position_from_index(&db_path, &file_path, &position)
            .unwrap_or_else(|error| {
                panic!("{language_id:?} persisted UTF-8 position read failed: {error}")
            });
        assert_eq!(
            persisted.symbol.symbol_id, live.symbol.symbol_id,
            "{language_id:?} live and persisted UTF-8 position reads must agree"
        );
        assert_eq!(
            persisted.source, live.source,
            "{language_id:?} live and persisted symbol source must agree"
        );
    }
}

#[test]
fn direct_and_unresolved_call_traces_match_live_and_persisted_indexes_for_traceable_languages() {
    use crate::{rebuild_symbol_index, trace_symbol_graph, trace_symbol_graph_from_index};

    let registry = builtin_language_registry();
    for language_id in registered_languages() {
        let descriptor = registry
            .descriptor(language_id)
            .expect("every registered language must have a descriptor");
        if !descriptor
            .capabilities
            .contains(LanguageCapabilities::REFERENCE_TRACE)
        {
            continue;
        }

        let dir = super::support::temporary_dir();
        let relative_path = sample_path(language_id);
        let path = dir.join(
            relative_path
                .file_name()
                .expect("sample path must have a file name"),
        );
        let source = trace_contract_source(language_id);
        fs::write(&path, source).unwrap();
        let caller_target = semantic_target_for_symbol_base(
            language_id,
            &path,
            source,
            trace_contract_symbol_base_name(language_id, true),
        );
        let callee_target = semantic_target_for_symbol_base(
            language_id,
            &path,
            source,
            trace_contract_symbol_base_name(language_id, false),
        );

        let live = trace_symbol_graph(&dir, &caller_target, TraceDirection::Both)
            .unwrap_or_else(|error| panic!("{language_id:?} live trace failed: {error}"));
        assert!(
            live.callees
                .iter()
                .any(|symbol| symbol.semantic_path == callee_target),
            "{language_id:?} live trace must resolve the direct call: {live:#?}"
        );

        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path)
            .unwrap_or_else(|error| panic!("{language_id:?} index rebuild failed: {error}"));
        let persisted =
            trace_symbol_graph_from_index(&db_path, &caller_target, TraceDirection::Both)
                .unwrap_or_else(|error| panic!("{language_id:?} persisted trace failed: {error}"));
        assert_eq!(
            persisted.symbol.symbol_id, live.symbol.symbol_id,
            "{language_id:?} live and persisted traces must resolve the same root"
        );
        assert_eq!(
            persisted
                .callees
                .iter()
                .map(|symbol| &symbol.symbol_id)
                .collect::<Vec<_>>(),
            live.callees
                .iter()
                .map(|symbol| &symbol.symbol_id)
                .collect::<Vec<_>>(),
            "{language_id:?} live and persisted direct-call traces must agree"
        );
        assert!(
            persisted
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == callee_target),
            "{language_id:?} persisted trace must resolve the direct call: {persisted:#?}"
        );

        let unresolved_source = unresolved_trace_contract_source(language_id);
        fs::write(&path, unresolved_source).unwrap();
        let live_unresolved = trace_symbol_graph(&dir, &caller_target, TraceDirection::Both)
            .unwrap_or_else(|error| {
                panic!("{language_id:?} unresolved live trace failed: {error}")
            });
        assert!(
            live_unresolved.callees.is_empty(),
            "{language_id:?} unresolved direct call must not create a live edge: {live_unresolved:#?}"
        );

        rebuild_symbol_index(&dir, &db_path).unwrap_or_else(|error| {
            panic!("{language_id:?} unresolved index rebuild failed: {error}")
        });
        let persisted_unresolved =
            trace_symbol_graph_from_index(&db_path, &caller_target, TraceDirection::Both)
                .unwrap_or_else(|error| {
                    panic!("{language_id:?} unresolved persisted trace failed: {error}")
                });
        assert!(
            persisted_unresolved.callees.is_empty(),
            "{language_id:?} unresolved direct call must not create a persisted edge: {persisted_unresolved:#?}"
        );
    }
}

#[test]
fn cross_language_references_fail_closed_for_traceable_languages() {
    use crate::{rebuild_symbol_index, trace_symbol_graph, trace_symbol_graph_from_index};

    for language_id in registered_languages() {
        let descriptor = builtin_language_registry()
            .descriptor(language_id)
            .expect("every registered language must have a descriptor");
        if !descriptor
            .capabilities
            .contains(LanguageCapabilities::REFERENCE_TRACE)
        {
            continue;
        }

        let dir = super::support::temporary_dir();
        let path = dir.join(sample_path(language_id).file_name().unwrap());
        let source = cross_language_trace_contract_source(language_id);
        fs::write(&path, source).unwrap();

        let foreign_path = if language_id == LanguageId::Python {
            dir.join("foreign.c")
        } else {
            dir.join("foreign.py")
        };
        let foreign_source = if language_id == LanguageId::Python {
            foreign_c_compute_source()
        } else {
            foreign_python_compute_source()
        };
        fs::write(&foreign_path, foreign_source).unwrap();

        let caller_target = semantic_target_for_symbol_base(
            language_id,
            &path,
            source,
            trace_contract_symbol_base_name(language_id, true),
        );
        let live = trace_symbol_graph(&dir, &caller_target, TraceDirection::Both).unwrap_or_else(
            |error| panic!("{language_id:?} cross-language live trace failed: {error}"),
        );
        assert!(
            live.callees.is_empty(),
            "{language_id:?} must not resolve a same-named foreign-language target: {live:#?}"
        );

        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path).unwrap_or_else(|error| {
            panic!("{language_id:?} cross-language index rebuild failed: {error}")
        });
        let persisted =
            trace_symbol_graph_from_index(&db_path, &caller_target, TraceDirection::Both)
                .unwrap_or_else(|error| {
                    panic!("{language_id:?} cross-language persisted trace failed: {error}")
                });
        assert!(
            persisted.callees.is_empty(),
            "{language_id:?} persisted trace must not resolve a same-named foreign-language target: {persisted:#?}"
        );
    }
}

#[test]
fn ambiguous_named_reexports_fail_closed_for_javascript_family_adapters() {
    use crate::{rebuild_symbol_index, trace_symbol_graph, trace_symbol_graph_from_index};

    for language_id in [
        LanguageId::JavaScript,
        LanguageId::TypeScript,
        LanguageId::Tsx,
    ] {
        let files = ambiguous_trace_contract_files(language_id)
            .expect("every JavaScript family language must have ambiguity fixtures");
        let dir = super::support::temporary_dir();
        for (file_name, source) in files {
            fs::write(dir.join(file_name), source).unwrap();
        }

        let caller_path = dir.join(sample_path(language_id).file_name().unwrap());
        let caller_source = fs::read_to_string(dir.join(match language_id {
            LanguageId::JavaScript => "resolver_ambiguous_calls.js",
            LanguageId::TypeScript => "resolver_ambiguous_calls.ts",
            LanguageId::Tsx => "resolver_ambiguous_calls.tsx",
            _ => unreachable!(),
        }))
        .unwrap();
        let caller_target =
            semantic_target_for_symbol_base(language_id, &caller_path, &caller_source, "caller");

        let live = trace_symbol_graph(&dir, &caller_target, TraceDirection::Both)
            .unwrap_or_else(|error| panic!("{language_id:?} ambiguous live trace failed: {error}"));
        assert!(
            live.callees.is_empty(),
            "{language_id:?} ambiguous named re-export must not produce a live edge: {live:#?}"
        );

        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path).unwrap_or_else(|error| {
            panic!("{language_id:?} ambiguous index rebuild failed: {error}")
        });
        let persisted =
            trace_symbol_graph_from_index(&db_path, &caller_target, TraceDirection::Both)
                .unwrap_or_else(|error| {
                    panic!("{language_id:?} ambiguous persisted trace failed: {error}")
                });
        assert!(
            persisted.callees.is_empty(),
            "{language_id:?} ambiguous named re-export must not produce a persisted edge: {persisted:#?}"
        );
    }
}

#[test]
fn persisted_indexes_reload_and_reject_stale_language_revisions_for_every_language() {
    use rusqlite::Connection;

    use crate::{read_symbol_from_index, rebuild_symbol_index};

    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
        let dir = super::support::temporary_dir();
        let relative_path = sample_path(language_id);
        let path = dir.join(
            relative_path
                .file_name()
                .expect("sample path must have a file name"),
        );
        let source = sample_source(language_id);
        fs::write(&path, source).unwrap();
        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path)
            .unwrap_or_else(|error| panic!("{language_id:?} index rebuild failed: {error}"));

        let semantic_target = semantic_target_for_patch_contract(language_id, &path, source);
        let reloaded = read_symbol_from_index(&db_path, &semantic_target)
            .unwrap_or_else(|error| panic!("{language_id:?} persisted reload failed: {error}"));
        assert_eq!(
            reloaded.symbol.semantic_path, semantic_target,
            "{language_id:?} persisted reload must preserve the semantic target"
        );

        let language_key: String = serde_json::from_value(
            serde_json::to_value(language_id).expect("language ID must serialize"),
        )
        .expect("language ID must serialize as a string");
        let expected = crate::index_schema::current_analysis_provenance_json().unwrap();
        let mut stale: serde_json::Value = serde_json::from_str(&expected).unwrap();
        stale["language_analysis_revisions"][&language_key] =
            serde_json::Value::String(format!("{language_key}-stale-contract-revision"));

        let connection = Connection::open(&db_path).unwrap();
        connection
            .execute(
                "UPDATE metadata SET value = ?1 WHERE key = 'analysis_provenance'",
                [serde_json::to_string(&stale).unwrap()],
            )
            .unwrap();
        drop(connection);

        let error = read_symbol_from_index(&db_path, &semantic_target)
            .expect_err("stale language provenance must reject persisted reads");
        assert!(
            error
                .to_string()
                .contains("does not match current analysis behavior"),
            "{language_id:?} stale provenance error must explain the rebuild requirement: {error}"
        );
    }
}

#[test]
fn vfs_overlay_reads_match_persisted_index_source_overlays_for_every_language() {
    use crate::{VirtualFileSystem, read_symbol_from_index_with_source, rebuild_symbol_index};

    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
        let dir = super::support::temporary_dir();
        let relative_path = sample_path(language_id);
        let path = dir.join(
            relative_path
                .file_name()
                .expect("sample path must have a file name"),
        );
        let source = sample_source(language_id);
        let dirty_source = overlay_source(language_id, source);
        assert_ne!(
            source, dirty_source,
            "{language_id:?} contract source must contain the overlay marker"
        );
        fs::write(&path, source).unwrap();
        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path)
            .unwrap_or_else(|error| panic!("{language_id:?} index rebuild failed: {error}"));

        let semantic_target = semantic_target_for_patch_contract(language_id, &path, source);
        let mut vfs = VirtualFileSystem::new();
        vfs.open_file(&path, Some(&dirty_source))
            .unwrap_or_else(|error| panic!("{language_id:?} VFS overlay failed: {error}"));

        let vfs_read = vfs
            .read_symbol(&dir, &semantic_target)
            .unwrap_or_else(|error| panic!("{language_id:?} VFS read failed: {error}"));
        let persisted_read =
            read_symbol_from_index_with_source(&db_path, &path, &dirty_source, &semantic_target)
                .unwrap_or_else(|error| {
                    panic!("{language_id:?} persisted overlay read failed: {error}")
                });

        assert_eq!(
            vfs_read.symbol.symbol_id, persisted_read.symbol.symbol_id,
            "{language_id:?} overlay symbol IDs must match"
        );
        assert_eq!(
            vfs_read.symbol.semantic_path, persisted_read.symbol.semantic_path,
            "{language_id:?} overlay semantic paths must match"
        );
        assert_eq!(
            vfs_read.source, persisted_read.source,
            "{language_id:?} VFS and persisted source overlays must match"
        );
        assert!(
            vfs_read.source.contains("value + 2"),
            "{language_id:?} overlay read must expose the virtual source"
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            source,
            "{language_id:?} VFS overlay queries must not mutate disk source"
        );
    }
}

#[test]
fn patch_previews_succeed_and_reject_unresolved_references_for_patchable_languages() {
    use crate::{patch_ast_node, preview_patch_ast_node};

    let registry = builtin_language_registry();
    for language_id in registered_languages() {
        let descriptor = registry
            .descriptor(language_id)
            .expect("every registered language must have a descriptor");
        if !descriptor
            .capabilities
            .contains(LanguageCapabilities::PATCH_TARGETING)
            || !descriptor
                .capabilities
                .contains(LanguageCapabilities::PATCH_VALIDATION)
        {
            continue;
        }

        let path = sample_path(language_id);
        let source = sample_source(language_id);
        let semantic_target = semantic_target_for_patch_contract(language_id, &path, source);

        let preview = preview_patch_ast_node(
            &path,
            source,
            &semantic_target,
            successful_patch_replacement(language_id),
            None,
        )
        .unwrap_or_else(|error| panic!("{language_id:?} patch preview must succeed: {error}"));
        assert!(
            preview.changed,
            "{language_id:?} preview must produce a diff"
        );
        assert!(
            !preview.unified_diff.is_empty(),
            "{language_id:?} preview must include a unified diff"
        );
        assert!(
            preview.patch.applied,
            "{language_id:?} preview: {preview:#?}"
        );
        assert!(
            preview.patch.validation.syntax_errors.is_empty(),
            "{language_id:?} preview: {preview:#?}"
        );
        assert!(
            preview.patch.validation.unresolved_identifiers.is_empty(),
            "{language_id:?} preview: {preview:#?}"
        );

        let rejected = patch_ast_node(
            &path,
            source,
            &semantic_target,
            unresolved_reference_patch_replacement(language_id),
            None,
        )
        .unwrap_or_else(|error| {
            panic!("{language_id:?} rejected patch must return validation: {error}")
        });
        assert!(
            !rejected.applied,
            "{language_id:?} unresolved reference must reject the patch: {rejected:#?}"
        );
        assert!(
            !rejected.validation.unresolved_identifiers.is_empty(),
            "{language_id:?} rejected patch must report an unresolved reference: {rejected:#?}"
        );
        assert_eq!(
            rejected.validation.commit_gate.status, "rejected",
            "{language_id:?} unresolved reference must close the commit gate"
        );
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
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
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

#[test]
fn persisted_symbol_ids_remain_stable_across_unchanged_rebuilds_for_every_language() {
    use crate::{read_symbol_from_index, rebuild_symbol_index};

    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
        let dir = super::support::temporary_dir();
        let path = dir.join(sample_path(language_id).file_name().unwrap());
        let source = sample_source(language_id);
        fs::write(&path, source).unwrap();
        let semantic_target = semantic_target_for_patch_contract(language_id, &path, source);
        let db_path = dir.join("symbols.db");

        rebuild_symbol_index(&dir, &db_path).unwrap_or_else(|error| {
            panic!("{language_id:?} initial index rebuild failed: {error}")
        });
        let first = read_symbol_from_index(&db_path, &semantic_target)
            .unwrap_or_else(|error| panic!("{language_id:?} initial symbol read failed: {error}"));

        let stats = rebuild_symbol_index(&dir, &db_path).unwrap_or_else(|error| {
            panic!("{language_id:?} unchanged index rebuild failed: {error}")
        });
        assert_eq!(
            stats.rebuilt_files, 0,
            "{language_id:?} unchanged rebuild must reuse the indexed source"
        );
        assert_eq!(
            stats.reused_files, 1,
            "{language_id:?} unchanged rebuild must reuse exactly one source file"
        );

        let second = read_symbol_from_index(&db_path, &semantic_target)
            .unwrap_or_else(|error| panic!("{language_id:?} reloaded symbol read failed: {error}"));
        assert_eq!(
            second.symbol.symbol_id, first.symbol.symbol_id,
            "{language_id:?} unchanged rebuild must preserve the public symbol ID"
        );
        assert_eq!(
            second.source, first.source,
            "{language_id:?} unchanged rebuild must preserve the symbol source"
        );
    }
}

#[test]
fn incremental_refresh_reindexes_changed_source_for_every_language() {
    use crate::{read_symbol_from_index, rebuild_symbol_index, refresh_symbol_index_for_file};

    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
        let dir = super::support::temporary_dir();
        let path = dir.join(sample_path(language_id).file_name().unwrap());
        let source = sample_source(language_id);
        let refreshed_source = overlay_source(language_id, source);
        fs::write(&path, source).unwrap();

        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path).unwrap_or_else(|error| {
            panic!("{language_id:?} initial index rebuild failed: {error}")
        });
        let semantic_target = semantic_target_for_patch_contract(language_id, &path, source);

        fs::write(&path, &refreshed_source).unwrap();
        let stats = refresh_symbol_index_for_file(&dir, &db_path, &path)
            .unwrap_or_else(|error| panic!("{language_id:?} incremental refresh failed: {error}"));
        assert_eq!(
            stats.indexed_files, 1,
            "{language_id:?} refresh must retain the single indexed source file"
        );
        assert_eq!(
            stats.rebuilt_files, 1,
            "{language_id:?} refresh must rebuild the changed source file"
        );
        assert_eq!(
            stats.reused_files, 0,
            "{language_id:?} refresh must not reuse the changed source file"
        );

        let refreshed =
            read_symbol_from_index(&db_path, &semantic_target).unwrap_or_else(|error| {
                panic!("{language_id:?} refreshed symbol read failed: {error}")
            });
        assert_eq!(
            refreshed.symbol.semantic_path, semantic_target,
            "{language_id:?} refresh must preserve the semantic target"
        );
        assert!(
            refreshed.source.contains("value + 2"),
            "{language_id:?} refreshed symbol must expose the updated source: {refreshed:#?}"
        );
    }
}

#[test]
fn symbol_extraction_respects_expired_deadlines_for_every_language() {
    use std::time::{Duration, Instant};

    use crate::symbol_extractor::index_symbols_from_document_with_deadline;
    use crate::workspace_scan::WorkspaceScanDeadline;

    let deadline = WorkspaceScanDeadline {
        deadline: Some(Instant::now() - Duration::from_millis(1)),
        timeout_ms: Some(1),
    };
    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
        let path = sample_path(language_id);
        let source = sample_source(language_id);
        let document = parse_document(&path, source)
            .unwrap_or_else(|error| panic!("{language_id:?} sample must parse: {error}"));
        let error = match index_symbols_from_document_with_deadline(
            &path,
            source,
            &document,
            Some(&deadline),
        ) {
            Ok(_) => panic!("{language_id:?} must reject an expired extraction deadline"),
            Err(error) => error,
        };
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded"),
            "{language_id:?} extraction must fail with the workspace scan timeout: {error}"
        );
    }
}
