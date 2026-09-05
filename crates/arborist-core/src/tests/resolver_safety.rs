use std::fs;
use std::path::{Path, PathBuf};

use crate::language::LanguageCapabilities;
use crate::language::builtin_language_registry;
use crate::model::{LanguageId, TraceDirection};
use crate::symbol_index_model::symbol_base_name_ref;
use crate::symbols::{
    rebuild_symbol_index, search_symbols, trace_symbol_graph, trace_symbol_graph_from_index,
};

/// Resolver safety invariants (design doc §17.3).
///
/// Every registered language that can index references must resolve a clear
/// same-language target, refuse to emit an edge for an unresolved name, and
/// never match a cross-language symbol. Live and persisted graph construction
/// must agree.
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
fn sample_path(language_id: LanguageId) -> PathBuf {
    let registry = builtin_language_registry();
    let descriptor = registry
        .descriptor(language_id)
        .expect("every registered language must have a descriptor");
    Path::new("caller").with_extension(descriptor.extensions[0])
}

fn caller_with_helper(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => {
            "def helper(value: int) -> int:\n    return value + 1\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n"
        }
        LanguageId::C | LanguageId::Cpp => {
            "int helper(int value) { return value + 1; }\n\nint orchestrate(int value) { return helper(value); }\n"
        }
        LanguageId::CSharp => {
            "namespace Demo {\n    public static class DemoClass {\n        public static int helper(int value) => value + 1;\n\n        public static int orchestrate(int value) => helper(value);\n    }\n}\n"
        }
        LanguageId::JavaScript => {
            "export function helper(value) {\n    return value + 1;\n}\n\nexport function orchestrate(value) {\n    return helper(value);\n}\n"
        }
        LanguageId::TypeScript | LanguageId::Tsx => {
            "export function helper(value: number): number {\n    return value + 1;\n}\n\nexport function orchestrate(value: number): number {\n    return helper(value);\n}\n"
        }
        LanguageId::Rust => {
            "fn helper(value: i32) -> i32 {\n    value + 1\n}\n\npub fn orchestrate(value: i32) -> i32 {\n    helper(value)\n}\n"
        }
        LanguageId::Go => {
            "package demo\n\nfunc helper(value int) int { return value + 1 }\n\nfunc orchestrate(value int) int { return helper(value) }\n"
        }
        LanguageId::Java => {
            "package demo;\n\npublic final class Demo {\n    static int helper(int value) {\n        return value + 1;\n    }\n\n    public static int orchestrate(int value) {\n        return helper(value);\n    }\n}\n"
        }
        LanguageId::Kotlin => {
            "package demo\n\nfun helper(value: Int): Int = value + 1\n\nfun orchestrate(value: Int): Int = helper(value)\n"
        }
        LanguageId::Lua => {
            "local function helper(value)
    return value + 1
end


function orchestrate(value)
    return helper(value)
end
"
        }
        LanguageId::Php => {
            "<?php\nfunction helper(int $value) {\n    return $value + 1;\n}\n\nfunction orchestrate(int $value) {\n    return helper($value);\n}\n"
        }
    }
}

fn caller_with_unresolved_call(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => {
            "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n"
        }
        LanguageId::C | LanguageId::Cpp => {
            "int orchestrate(int value) { return missing_helper(value); }\n"
        }
        LanguageId::CSharp => {
            "namespace Demo {\n    public static class DemoClass {\n        public static int orchestrate(int value) => missing_helper(value);\n    }\n}\n"
        }
        LanguageId::JavaScript => {
            "export function orchestrate(value) {\n    return missing_helper(value);\n}\n"
        }
        LanguageId::TypeScript | LanguageId::Tsx => {
            "export function orchestrate(value: number): number {\n    return missing_helper(value);\n}\n"
        }
        LanguageId::Rust => {
            "pub fn orchestrate(value: i32) -> i32 {\n    missing_helper(value)\n}\n"
        }
        LanguageId::Go => {
            "package demo\n\nfunc orchestrate(value int) int { return missing_helper(value) }\n"
        }
        LanguageId::Java => {
            "package demo;\n\npublic final class Demo {\n    public static int orchestrate(int value) {\n        return missing_helper(value);\n    }\n}\n"
        }
        LanguageId::Kotlin => {
            "package demo\n\nfun orchestrate(value: Int): Int = missing_helper(value)\n"
        }
        LanguageId::Lua => {
            "function orchestrate(value)
    return missing_helper(value)
end
"
        }
        LanguageId::Php => {
            "<?php\nfunction orchestrate(int $value) {\n    return missing_helper($value);\n}\n"
        }
    }
}

fn helper_only(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => "def helper(value: int) -> int:\n    return value + 1\n",
        LanguageId::C | LanguageId::Cpp => "int helper(int value) { return value + 1; }\n",
        LanguageId::CSharp => {
            "namespace Demo {\n    public static class DemoClass {\n        public static int helper(int value) => value + 1;\n    }\n}\n"
        }
        LanguageId::JavaScript => "export function helper(value) {\n    return value + 1;\n}\n",
        LanguageId::TypeScript | LanguageId::Tsx => {
            "export function helper(value: number): number {\n    return value + 1;\n}\n"
        }
        LanguageId::Rust => "fn helper(value: i32) -> i32 {\n    value + 1\n}\n",
        LanguageId::Go => "package demo\n\nfunc helper(value int) int { return value + 1 }\n",
        LanguageId::Php => "<?php\nfunction helper(int $value) {\n    return $value + 1;\n}\n",
        LanguageId::Java => {
            "package demo;\n\npublic final class Demo {\n    static int helper(int value) {\n        return value + 1;\n    }\n}\n"
        }
        LanguageId::Kotlin => "package demo\n\nfun helper(value: Int): Int = value + 1\n",
        LanguageId::Lua => "local function helper(value)\n    return value + 1\nend\n",
    }
}

fn orchestrate_semantic_path(workspace: &Path, base_name: &str) -> String {
    let results = search_symbols(workspace, base_name, 10)
        .unwrap_or_else(|error| panic!("search for {base_name} must succeed: {error}"));
    results
        .matches
        .iter()
        .find(|symbol| symbol_base_name_ref(&symbol.semantic_path) == base_name)
        .unwrap_or_else(|| {
            panic!(
                "expected a {base_name} symbol; got: {:?}",
                results
                    .matches
                    .iter()
                    .map(|symbol| symbol.semantic_path.clone())
                    .collect::<Vec<_>>()
            )
        })
        .semantic_path
        .clone()
}

#[test]
fn clear_same_language_targets_resolve_and_persisted_graphs_agree() {
    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::REFERENCE_TRACE) {
            continue;
        }
        let dir = super::support::temporary_dir();
        let file_name = sample_path(language_id)
            .file_name()
            .expect("sample path must have a file name")
            .to_owned();
        fs::write(dir.join(&file_name), caller_with_helper(language_id))
            .unwrap_or_else(|error| panic!("{language_id:?} sample must be writable: {error}"));
        let db_path = dir.join("symbols.db");
        rebuild_symbol_index(&dir, &db_path)
            .unwrap_or_else(|error| panic!("{language_id:?} index rebuild must succeed: {error}"));

        let semantic_path = orchestrate_semantic_path(&dir, "orchestrate");

        let live = trace_symbol_graph(&dir, &semantic_path, TraceDirection::Both)
            .unwrap_or_else(|error| panic!("{language_id:?} live trace must succeed: {error}"));
        let persisted =
            trace_symbol_graph_from_index(&db_path, &semantic_path, TraceDirection::Both)
                .unwrap_or_else(|error| {
                    panic!("{language_id:?} persisted trace must succeed: {error}")
                });

        let live_callees: Vec<String> = live
            .callees
            .iter()
            .map(|symbol| symbol.semantic_path.clone())
            .collect();
        let persisted_callees: Vec<String> = persisted
            .callees
            .iter()
            .map(|symbol| symbol.semantic_path.clone())
            .collect();
        assert_eq!(
            live_callees, persisted_callees,
            "{language_id:?} live and persisted trace graphs must agree"
        );
        assert!(
            live_callees.iter().any(|path| path.ends_with("helper")),
            "{language_id:?} must resolve the same-language helper callee, got {live_callees:?}"
        );
    }
}

#[test]
fn unresolved_names_produce_no_accidental_edges_for_every_language() {
    for language_id in registered_languages() {
        if !advertises_capability(language_id, LanguageCapabilities::REFERENCE_TRACE) {
            continue;
        }
        let dir = super::support::temporary_dir();
        let file_name = sample_path(language_id)
            .file_name()
            .expect("sample path must have a file name")
            .to_owned();
        fs::write(
            dir.join(&file_name),
            caller_with_unresolved_call(language_id),
        )
        .unwrap_or_else(|error| panic!("{language_id:?} sample must be writable: {error}"));

        let semantic_path = orchestrate_semantic_path(&dir, "orchestrate");
        let trace = trace_symbol_graph(&dir, &semantic_path, TraceDirection::Both)
            .unwrap_or_else(|error| panic!("{language_id:?} trace must succeed: {error}"));
        let callees: Vec<String> = trace
            .callees
            .iter()
            .map(|symbol| symbol.semantic_path.clone())
            .collect();
        assert!(
            callees.is_empty(),
            "{language_id:?} unresolved call must not produce a callee edge, got {callees:?}"
        );
    }
}

#[test]
fn cross_language_matching_is_disabled_for_every_language() {
    for callee_language in registered_languages() {
        if !advertises_capability(callee_language, LanguageCapabilities::SYMBOL_INDEX) {
            continue;
        }
        let dir = super::support::temporary_dir();
        fs::write(
            dir.join("caller.py"),
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();
        let helper_file_name =
            Path::new("helper").with_extension(callee_extension(callee_language));
        fs::write(dir.join(helper_file_name), helper_only(callee_language))
            .unwrap_or_else(|error| panic!("{callee_language:?} helper must be writable: {error}"));

        let semantic_path = orchestrate_semantic_path(&dir, "orchestrate");
        let trace = trace_symbol_graph(&dir, &semantic_path, TraceDirection::Both)
            .unwrap_or_else(|error| panic!("{callee_language:?} trace must succeed: {error}"));
        let callees: Vec<String> = trace
            .callees
            .iter()
            .map(|symbol| symbol.semantic_path.clone())
            .collect();
        if callee_language == LanguageId::Python {
            assert!(
                !callees.is_empty(),
                "same-language Python helper must resolve"
            );
        } else {
            assert!(
                callees.is_empty(),
                "{callee_language:?} helper must not be linked from a Python caller, got {callees:?}"
            );
        }
    }
}

fn callee_extension(language_id: LanguageId) -> &'static str {
    let registry = builtin_language_registry();
    registry
        .descriptor(language_id)
        .expect("every registered language must have a descriptor")
        .extensions[0]
}
