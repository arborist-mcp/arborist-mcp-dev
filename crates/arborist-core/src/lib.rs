mod language;
mod model;
mod patching;
mod query;
mod semantic;
mod symbols;
mod vfs;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Result;

pub use model::{
    LanguageId, PatchAstNodeResult, PatchTraceValidationResult, PatchValidationReport, Position,
    PositionEdit, QueryCaptureResult, RegisteredSymbolIndex, SemanticSkeleton,
    SemanticSkeletonSymbol, SymbolIndexStats, SymbolMeta, SymbolSummary, TraceBackedPatchResult,
    TraceDirection, TracePatchEvidenceReplayItem, TracePatchEvidenceReplayResult,
    TraceSymbolGraphResult, ValidationAmbiguity, ValidationBinding, ValidationIssue,
    VirtualEditResult, VirtualFileSnapshot, VirtualFileStatus,
};

pub use language::{read_source, supported_languages};
pub use patching::{patch_ast_node, patch_ast_node_from_path};
pub use query::{execute_tree_query, execute_tree_query_from_path};
pub use symbols::{
    rebuild_symbol_index, refresh_symbol_index_for_file, trace_symbol_graph,
    trace_symbol_graph_from_index,
};
pub use vfs::VirtualFileSystem;

pub fn get_semantic_skeleton_from_path(
    path: &Path,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    let path = language::normalize_absolute_path(path)?;
    let source = read_source(&path)?;
    get_semantic_skeleton(&path, &source, depth_limit, expand_nodes)
}

pub fn get_semantic_skeleton(
    path: &Path,
    source: &str,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    let document = language::parse_document(path, source)?;
    semantic::get_semantic_skeleton(
        path,
        document.language_id,
        source,
        &document.tree,
        depth_limit,
        expand_nodes,
    )
}

pub fn replay_patch_evidence_against_trace(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
) -> TracePatchEvidenceReplayResult {
    let trace_callers = trace
        .evidence_keys
        .callers
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let trace_callees = trace
        .evidence_keys
        .callees
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let trace_symbol = trace.evidence_keys.symbol.clone();

    let items = patch
        .validation
        .commit_gate
        .evidence_invariants
        .iter()
        .map(|invariant| {
            let (matched_in_trace, trace_match_scope) =
                if let Some(selected) = &invariant.selected_evidence_key {
                    if trace_callees.contains(selected) {
                        (true, "callees".to_string())
                    } else if trace_callers.contains(selected) {
                        (true, "callers".to_string())
                    } else if trace_symbol == *selected {
                        (true, "symbol".to_string())
                    } else {
                        (false, "none".to_string())
                    }
                } else {
                    (false, "none".to_string())
                };

            let status = match invariant.status.as_str() {
                "passed" if matched_in_trace => "matched",
                "passed" => "missing",
                "blocked" => "blocked",
                _ => "failed",
            }
            .to_string();

            TracePatchEvidenceReplayItem {
                name: invariant.name.clone(),
                status,
                selected_evidence_key: invariant.selected_evidence_key.clone(),
                matched_in_trace,
                trace_match_scope,
                candidate_evidence_keys: invariant.candidate_evidence_keys.clone(),
            }
        })
        .collect::<Vec<_>>();

    let matched_items = items.iter().filter(|item| item.status == "matched").count();
    let blocked_items = items.iter().filter(|item| item.status == "blocked").count();
    let consistent = items
        .iter()
        .all(|item| matches!(item.status.as_str(), "matched" | "blocked"));

    TracePatchEvidenceReplayResult {
        consistent,
        matched_items,
        blocked_items,
        items,
    }
}

pub fn validate_patch_commit_with_trace(
    patch: &PatchAstNodeResult,
    trace: &TraceSymbolGraphResult,
) -> PatchTraceValidationResult {
    let replay = replay_patch_evidence_against_trace(patch, trace);
    let replay_status = summarize_replay_status(&replay);
    let patch_gate_status = patch.validation.commit_gate.status.clone();

    if !patch.validation.commit_gate.allowed {
        return PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_patch_gate".to_string(),
            reason: patch.validation.commit_gate.reason.clone(),
            patch_gate_status,
            replay_status,
            replay,
        };
    }

    if matches!(replay_status.as_str(), "missing" | "failed") {
        return PatchTraceValidationResult {
            allowed: false,
            status: "rejected_by_trace_replay".to_string(),
            reason: "trace replay did not confirm the patch evidence".to_string(),
            patch_gate_status,
            replay_status,
            replay,
        };
    }

    let (status, reason) = if patch.validation.commit_gate.status == "allowed_with_bypass" {
        (
            "allowed_with_bypass".to_string(),
            "patch gate allowed the write with bypass and trace replay did not contradict the evidence".to_string(),
        )
    } else {
        (
            "allowed".to_string(),
            "patch gate and trace replay both accepted the evidence".to_string(),
        )
    };

    PatchTraceValidationResult {
        allowed: true,
        status,
        reason,
        patch_gate_status,
        replay_status,
        replay,
    }
}

pub fn validate_patch_with_trace_context_from_path(
    workspace_root: &Path,
    path: &Path,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
) -> Result<TraceBackedPatchResult> {
    let workspace_root = language::normalize_absolute_path(workspace_root)?;
    let path = language::normalize_absolute_path(path)?;
    let source = read_source(&path)?;
    validate_patch_with_trace_context(
        &workspace_root,
        &path,
        &source,
        semantic_target,
        new_code,
        bypass_reason,
        direction,
    )
}

pub fn validate_patch_with_trace_context(
    workspace_root: &Path,
    path: &Path,
    source: &str,
    semantic_target: &str,
    new_code: &str,
    bypass_reason: Option<&str>,
    direction: TraceDirection,
) -> Result<TraceBackedPatchResult> {
    let patch = patch_ast_node(path, source, semantic_target, new_code, bypass_reason)?;
    let trace_target = patch.resolved_symbol_id.clone();

    if !patch.validation.syntax_errors.is_empty() {
        return Ok(TraceBackedPatchResult {
            patch,
            trace_target,
            trace: None,
            trace_validation: None,
            trace_error: Some(
                "trace skipped because patch validation reported syntax errors".to_string(),
            ),
        });
    }

    let mut overrides = BTreeMap::new();
    overrides.insert(patch.file.clone(), patch.updated_source.clone());
    let trace = symbols::trace_symbol_graph_with_overrides(
        workspace_root,
        &overrides,
        &trace_target,
        direction,
    )?;
    let trace_validation = validate_patch_commit_with_trace(&patch, &trace);

    Ok(TraceBackedPatchResult {
        patch,
        trace_target,
        trace: Some(trace),
        trace_validation: Some(trace_validation),
        trace_error: None,
    })
}

fn summarize_replay_status(replay: &TracePatchEvidenceReplayResult) -> String {
    if replay.items.iter().any(|item| item.status == "failed") {
        return "failed".to_string();
    }
    if replay.items.iter().any(|item| item.status == "missing") {
        return "missing".to_string();
    }
    if replay.items.iter().any(|item| item.status == "blocked") {
        return "blocked".to_string();
    }
    "matched".to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use rusqlite::Connection;

    use super::{
        TraceDirection, execute_tree_query, execute_tree_query_from_path, get_semantic_skeleton,
        get_semantic_skeleton_from_path, patch_ast_node, patch_ast_node_from_path,
        rebuild_symbol_index, refresh_symbol_index_for_file, replay_patch_evidence_against_trace,
        trace_symbol_graph, trace_symbol_graph_from_index, validate_patch_commit_with_trace,
        validate_patch_with_trace_context_from_path,
    };

    #[test]
    fn builds_python_skeleton_with_nested_members() {
        let source = r#"
class Greeter:
    """Helpful greeter."""

    def greet(self, name: str) -> str:
        """Return a greeting."""
        return f"hello, {name}"

def top_level(value: int) -> int:
    """Top level orchestration."""

    def nested(inner: int) -> int:
        """Inner increment helper."""
        return inner + 1

    return nested(value)
"#;

        let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 2, &[]).unwrap();

        assert!(skeleton.skeleton.contains("class Greeter: ..."));
        assert!(
            skeleton
                .skeleton
                .contains("def top_level(value: int) -> int: ...")
        );
        assert!(
            skeleton
                .skeleton
                .contains("def nested(inner: int) -> int: ...")
        );
        assert_eq!(
            skeleton.available_paths,
            vec!["Greeter", "Greeter.greet", "top_level", "top_level.nested"]
        );
        assert_eq!(skeleton.available_symbols.len(), 4);
        assert_eq!(skeleton.available_symbols[0].symbol_id, "Greeter");
        assert_eq!(skeleton.available_symbols[0].semantic_path, "Greeter");
        assert_eq!(skeleton.available_symbols[0].scope_path, None);
        assert_eq!(skeleton.available_symbols[0].node_kind, "class_definition");
        assert_eq!(
            skeleton.available_symbols[0].signature.as_deref(),
            Some("class Greeter:")
        );
        assert!(skeleton.available_symbols[0].parameters.is_empty());
        assert_eq!(skeleton.available_symbols[0].return_type, None);
        assert_eq!(
            skeleton.available_symbols[0].docstring.as_deref(),
            Some("\"\"\"Helpful greeter.\"\"\"")
        );
        assert_eq!(skeleton.available_symbols[3].symbol_id, "top_level.nested");
        assert_eq!(
            skeleton.available_symbols[3].scope_path.as_deref(),
            Some("top_level")
        );
        assert_eq!(
            skeleton.available_symbols[3].signature.as_deref(),
            Some("def nested(inner: int) -> int:")
        );
        assert_eq!(
            skeleton.available_symbols[3].parameters,
            vec!["inner: int".to_string()]
        );
        assert_eq!(
            skeleton.available_symbols[3].return_type.as_deref(),
            Some("int")
        );
        assert_eq!(
            skeleton.available_symbols[3].docstring.as_deref(),
            Some("\"\"\"Inner increment helper.\"\"\"")
        );
    }

    #[test]
    fn builds_python_skeleton_with_async_members() {
        let source = r#"
async def top_level(value: int) -> int:
    """Top level async orchestration."""

    async def nested(inner: int) -> int:
        """Inner async helper."""
        return inner + 1

    return await nested(value)
"#;

        let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 2, &[]).unwrap();

        assert!(
            skeleton
                .skeleton
                .contains("async def top_level(value: int) -> int: ...")
        );
        assert!(
            skeleton
                .skeleton
                .contains("async def nested(inner: int) -> int: ...")
        );
        assert_eq!(
            skeleton.available_paths,
            vec!["top_level", "top_level.nested"]
        );
        assert_eq!(skeleton.available_symbols.len(), 2);
        assert_eq!(
            skeleton.available_symbols[0].node_kind,
            "function_definition"
        );
        assert_eq!(
            skeleton.available_symbols[0].signature.as_deref(),
            Some("async def top_level(value: int) -> int:")
        );
        assert_eq!(
            skeleton.available_symbols[1].scope_path.as_deref(),
            Some("top_level")
        );
        assert_eq!(
            skeleton.available_symbols[1].signature.as_deref(),
            Some("async def nested(inner: int) -> int:")
        );
    }

    #[test]
    fn builds_python_skeleton_with_decorated_members() {
        let source = r#"
def decorator(func):
    return func

@decorator
def top_level(value: int) -> int:
    return value
"#;

        let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 1, &[]).unwrap();

        assert!(skeleton.skeleton.contains("@decorator"));
        assert!(
            skeleton
                .skeleton
                .contains("def top_level(value: int) -> int: ...")
        );
        let top_level = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "top_level")
            .unwrap();
        assert_eq!(
            top_level.signature.as_deref(),
            Some("@decorator\ndef top_level(value: int) -> int:")
        );
    }

    #[test]
    fn uses_decorated_python_member_ranges_in_skeleton_metadata() {
        let source = r#"
def decorator(func):
    return func

@decorator
def top_level(value: int) -> int:
    return value
"#;

        let skeleton = get_semantic_skeleton(Path::new("sample.py"), source, 1, &[]).unwrap();
        let top_level = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "top_level")
            .unwrap();
        let decorated_symbol = "@decorator\ndef top_level(value: int) -> int:\n    return value";
        let start = source.find(decorated_symbol).unwrap();
        let end = start + decorated_symbol.len();

        assert_eq!(top_level.byte_range, (start, end));
    }

    #[test]
    fn expands_selected_python_nodes_without_duplicating_children() {
        let source = r#"
class Greeter:
    def greet(self, name: str) -> str:
        return f"hello, {name}"

def top_level(value: int) -> int:
    def nested(inner: int) -> int:
        return inner + 1

    return nested(value)
"#;

        let skeleton = get_semantic_skeleton(
            Path::new("sample.py"),
            source,
            2,
            &["Greeter".to_string(), "top_level.nested".to_string()],
        )
        .unwrap();

        assert!(skeleton.skeleton.contains("class Greeter:\n    def greet"));
        assert!(!skeleton.skeleton.contains("class Greeter: ..."));
        assert_eq!(skeleton.skeleton.matches("def greet").count(), 1);
        assert!(
            skeleton
                .skeleton
                .contains("def nested(inner: int) -> int:\n        return inner + 1")
        );
    }

    #[test]
    fn expands_decorated_python_nodes_with_decorators() {
        let source = r#"
def decorator(func):
    return func

@decorator
def top_level(value: int) -> int:
    return value + 1
"#;

        let skeleton = get_semantic_skeleton(
            Path::new("sample.py"),
            source,
            1,
            &["top_level".to_string()],
        )
        .unwrap();

        assert!(skeleton.skeleton.contains("@decorator\ndef top_level"));
        assert!(skeleton.skeleton.contains("return value + 1"));
    }

    #[test]
    fn expands_selected_c_function_definitions() {
        let source = r#"
typedef struct item {
    int value;
} item;

int helper(int value) {
    return value + 1;
}
"#;

        let skeleton =
            get_semantic_skeleton(Path::new("sample.c"), source, 1, &["helper".to_string()])
                .unwrap();

        assert!(skeleton.skeleton.contains("typedef struct item"));
        assert!(
            skeleton
                .skeleton
                .contains("int helper(int value) {\n    return value + 1;\n}")
        );
        assert_eq!(skeleton.available_symbols.len(), 2);
        assert_eq!(skeleton.available_symbols[1].semantic_path, "helper");
        assert_eq!(skeleton.available_symbols[1].scope_path, None);
        assert_eq!(
            skeleton.available_symbols[1].node_kind,
            "function_definition"
        );
        assert_eq!(
            skeleton.available_symbols[1].signature.as_deref(),
            Some("int helper(int value);")
        );
        assert_eq!(
            skeleton.available_symbols[1].parameters,
            vec!["int value".to_string()]
        );
        assert_eq!(
            skeleton.available_symbols[1].return_type.as_deref(),
            Some("int")
        );
        assert_eq!(skeleton.available_symbols[1].docstring, None);
    }

    #[test]
    fn expands_c_function_definition_by_precise_symbol_id() {
        let dir = temporary_dir();
        let header = dir.join("helper.h");
        let source = dir.join("helper.c");

        fs::write(&header, "int helper(int value);\n").unwrap();
        fs::write(
            &source,
            "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let source_text = fs::read_to_string(&source).unwrap();
        let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
        let precise_symbol_id = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "helper")
            .map(|symbol| symbol.symbol_id.clone())
            .unwrap();

        let expanded =
            get_semantic_skeleton(&source, &source_text, 1, &[precise_symbol_id]).unwrap();

        assert!(
            expanded
                .skeleton
                .contains("int helper(int value) {\n    return value + 1;\n}")
        );
    }

    #[test]
    fn traces_c_symbol_graph_across_header_declaration_and_source_definition() {
        let dir = temporary_dir();
        let header = dir.join("helper.h");
        let helper = dir.join("helper.c");
        let caller = dir.join("caller.c");
        let db_path = dir.join("symbols.db");

        fs::write(&header, "int helper(int value);\n").unwrap();
        fs::write(
            &helper,
            "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );

        let stats = rebuild_symbol_index(&dir, &db_path).unwrap();
        assert_eq!(stats.indexed_files, 3);

        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn isolates_static_c_symbols_per_file() {
        let dir = temporary_dir();
        let a = dir.join("a.c");
        let b = dir.join("b.c");
        let db_path = dir.join("symbols.db");

        fs::write(
            &a,
            "static int helper(int value) {\n    return value + 1;\n}\n\nint use_a(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();
        fs::write(
            &b,
            "static int helper(int value) {\n    return value + 2;\n}\n\nint use_b(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

        let trace_a = trace_symbol_graph(&dir, "use_a", TraceDirection::Both).unwrap();
        let trace_b = trace_symbol_graph(&dir, "use_b", TraceDirection::Both).unwrap();

        assert_eq!(trace_a.callees.len(), 1);
        assert_eq!(trace_b.callees.len(), 1);
        assert_eq!(
            trace_a.callees[0].file_path,
            a.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(
            trace_b.callees[0].file_path,
            b.to_string_lossy().replace('\\', "/")
        );
        assert_ne!(
            trace_a.callees[0].semantic_path,
            trace_b.callees[0].semantic_path
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace_b =
            trace_symbol_graph_from_index(&db_path, "use_b", TraceDirection::Both).unwrap();
        assert_eq!(persisted_trace_b.callees.len(), 1);
        assert_eq!(
            persisted_trace_b.callees[0].file_path,
            b.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn prefers_callee_from_included_header_family_when_names_collide() {
        let dir = temporary_dir();
        let alpha_header = dir.join("alpha.h");
        let alpha_source = dir.join("alpha.c");
        let zeta_header = dir.join("zeta.h");
        let zeta_source = dir.join("zeta.c");
        let caller = dir.join("caller.c");
        let db_path = dir.join("symbols.db");

        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(trace.callees.len(), 1);
        assert_eq!(
            trace.callees[0].file_path,
            zeta_source.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(trace.callees[0].origin_type, "companion_source");
        assert_eq!(
            trace.evidence_keys.callees,
            vec![trace.callees[0].evidence_key.clone()]
        );
        assert_eq!(trace.symbol.origin_type, "trace_root");
        assert_eq!(trace.symbol.evidence_key, trace.evidence_keys.symbol);
        assert!(trace.symbol.evidence_key.contains("trace_root"));

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(persisted_trace.callees.len(), 1);
        assert_eq!(
            persisted_trace.callees[0].file_path,
            zeta_source.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(persisted_trace.callees[0].origin_type, "companion_source");
        assert_eq!(
            persisted_trace.evidence_keys.callees,
            vec![persisted_trace.callees[0].evidence_key.clone()]
        );
        assert_eq!(persisted_trace.symbol.origin_type, "trace_root");
        assert_eq!(
            persisted_trace.symbol.evidence_key,
            persisted_trace.evidence_keys.symbol
        );
        let zeta_source_text = fs::read_to_string(&zeta_source).unwrap();
        let zeta_start = zeta_source_text.find("int helper(int value) {").unwrap();
        let zeta_end = zeta_source_text.find('}').map(|index| index + 1).unwrap();
        assert_eq!(persisted_trace.callees[0].node_kind, "function_definition");
        assert_eq!(
            persisted_trace.callees[0].byte_range,
            (zeta_start, zeta_end)
        );
        assert_eq!(
            persisted_trace.callees[0].signature.as_deref(),
            Some("int helper(int value);")
        );
        assert!(
            persisted_trace.callees[0]
                .evidence_key
                .contains(&persisted_trace.callees[0].symbol_id)
        );
        assert!(
            persisted_trace.callees[0]
                .evidence_key
                .contains("function_definition|companion_source")
        );
        assert!(
            persisted_trace.callees[0]
                .evidence_key
                .contains(&format!("{zeta_start}..{zeta_end}"))
        );
    }

    #[test]
    fn allows_c_patch_when_symbol_is_declared_in_included_header() {
        let dir = temporary_dir();
        let header = dir.join("helper.h");
        let caller = dir.join("caller.c");

        fs::write(&header, "int helper(int value);\n").unwrap();
        fs::write(
            &caller,
            "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let result = patch_ast_node_from_path(
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.unresolved_identifiers.is_empty());
        assert!(result.validation.commit_gate.allowed);
        assert_eq!(result.validation.commit_gate.status, "allowed");
        assert_eq!(
            result.validation.commit_gate.reason,
            "syntax and symbol binding validation passed"
        );
        assert_eq!(result.validation.commit_gate.syntax_error_count, 0);
        assert!(result.validation.commit_gate.blocking_decisions.is_empty());
        assert_eq!(result.validation.commit_gate.evidence_invariants.len(), 1);
        assert_eq!(
            result.validation.commit_gate.evidence_invariants[0].status,
            "passed"
        );
        assert_eq!(result.validation.ambiguous_identifiers.len(), 0);
        assert_eq!(result.validation.resolved_identifiers.len(), 1);
        assert_eq!(result.validation.binding_decisions.len(), 1);
        assert_eq!(result.validation.binding_decisions[0].name, "helper");
        assert_eq!(result.validation.binding_decisions[0].status, "resolved");
        assert_eq!(result.validation.resolved_identifiers[0].name, "helper");
        assert_eq!(
            result.validation.resolved_identifiers[0].symbol.symbol_id,
            format!("{}::helper", header.to_string_lossy().replace('\\', "/"))
        );
        assert_eq!(
            result.validation.binding_decisions[0]
                .selected_symbol_id
                .as_deref(),
            Some(
                result.validation.resolved_identifiers[0]
                    .symbol
                    .symbol_id
                    .as_str()
            )
        );
        assert_eq!(result.validation.binding_decisions[0].candidates.len(), 1);
        assert_eq!(
            result.validation.commit_gate.evidence_invariants[0]
                .selected_evidence_key
                .as_deref(),
            Some(
                result.validation.binding_decisions[0].candidates[0]
                    .evidence_key
                    .as_str()
            )
        );
        let header_text = fs::read_to_string(&header).unwrap();
        assert_eq!(
            result.validation.resolved_identifiers[0].symbol.node_kind,
            "declaration"
        );
        assert_eq!(
            result.validation.resolved_identifiers[0].symbol.origin_type,
            "include_header"
        );
        assert!(
            result.validation.resolved_identifiers[0]
                .symbol
                .evidence_key
                .contains("declaration|include_header")
        );
        assert_eq!(
            result.validation.resolved_identifiers[0].symbol.byte_range,
            (0, header_text.find(';').map(|index| index + 1).unwrap())
        );
        assert_eq!(
            result.validation.resolved_identifiers[0]
                .symbol
                .signature
                .as_deref(),
            Some("int helper(int value);")
        );
        let updated = fs::read_to_string(&caller).unwrap();
        assert!(updated.contains("return helper(value);"));
    }

    #[test]
    fn patches_c_definition_when_declaration_and_definition_share_name() {
        let dir = temporary_dir();
        let file = dir.join("helper.c");

        fs::write(
            &file,
            "int helper(int value);\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let result = patch_ast_node_from_path(
            &file,
            "helper",
            "int helper(int value) {\n    return value + 9;\n}\n",
            None,
        )
        .unwrap();

        let updated = fs::read_to_string(&file).unwrap();
        assert!(result.applied);
        assert_eq!(result.resolved_path, "helper");
        assert_eq!(
            result.resolved_symbol_id,
            format!("{}::helper", file.to_string_lossy().replace('\\', "/"))
        );
        assert!(updated.starts_with("int helper(int value);\n\n"));
        assert!(updated.contains("int helper(int value) {\n    return value + 9;\n}"));
        assert!(updated.contains("return value + 9;"));
        assert_eq!(updated.matches("int helper(int value);").count(), 1);
    }

    #[test]
    fn allows_c_patch_targeting_precise_symbol_id() {
        let dir = temporary_dir();
        let header = dir.join("helper.h");
        let source = dir.join("helper.c");

        fs::write(&header, "int helper(int value);\n").unwrap();
        fs::write(
            &source,
            "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let symbol_id = format!("{}::helper", header.to_string_lossy().replace('\\', "/"));
        let result = patch_ast_node_from_path(
            &source,
            &symbol_id,
            "int helper(int value) {\n    return value + 5;\n}\n",
            None,
        )
        .unwrap();

        let updated = fs::read_to_string(&source).unwrap();
        assert!(result.applied);
        assert_eq!(result.target_path, symbol_id);
        assert_eq!(result.resolved_path, "helper");
        assert_eq!(result.resolved_symbol_id, result.target_path);
        assert!(updated.contains("return value + 5;"));
    }

    #[test]
    fn reports_ambiguous_c_identifier_bindings() {
        let dir = temporary_dir();
        let alpha_header = dir.join("alpha.h");
        let alpha_source = dir.join("alpha.c");
        let zeta_header = dir.join("zeta.h");
        let zeta_source = dir.join("zeta.c");
        let caller = dir.join("caller.c");

        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let result = patch_ast_node_from_path(
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert!(result.validation.unresolved_identifiers.is_empty());
        assert!(result.validation.resolved_identifiers.is_empty());
        assert!(!result.validation.commit_gate.allowed);
        assert_eq!(result.validation.commit_gate.status, "rejected");
        assert_eq!(
            result.validation.commit_gate.reason,
            "symbol binding is ambiguous"
        );
        assert_eq!(result.validation.commit_gate.syntax_error_count, 0);
        assert_eq!(result.validation.commit_gate.blocking_decisions.len(), 1);
        assert_eq!(
            result.validation.commit_gate.blocking_decisions[0].status,
            "ambiguous"
        );
        assert_eq!(result.validation.commit_gate.evidence_invariants.len(), 1);
        assert_eq!(
            result.validation.commit_gate.evidence_invariants[0].status,
            "blocked"
        );
        assert_eq!(
            result.validation.commit_gate.evidence_invariants[0]
                .candidate_evidence_keys
                .len(),
            2
        );
        assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
        assert_eq!(result.validation.ambiguous_identifiers[0].name, "helper");
        assert_eq!(result.validation.binding_decisions.len(), 1);
        assert_eq!(result.validation.binding_decisions[0].name, "helper");
        assert_eq!(result.validation.binding_decisions[0].status, "ambiguous");
        assert_eq!(
            result.validation.binding_decisions[0].selected_symbol_id,
            None
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates.len(),
            2
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].reason,
            "multiple equally-ranked definitions across include families"
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0]
                .disambiguation_context
                .active_include_family,
            None
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0]
                .disambiguation_context
                .preferred_family,
            None
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0]
                .disambiguation_context
                .visible_include_families,
            vec![
                alpha_header.to_string_lossy().replace('\\', "/"),
                zeta_header.to_string_lossy().replace('\\', "/")
            ]
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0]
                .disambiguation_context
                .candidate_include_families,
            vec![
                alpha_header.to_string_lossy().replace('\\', "/"),
                zeta_header.to_string_lossy().replace('\\', "/")
            ]
        );
        assert_eq!(
            result.validation.binding_decisions[0].reason,
            result.validation.ambiguous_identifiers[0].reason
        );
        assert_eq!(result.validation.binding_decisions[0].candidates.len(), 2);
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[0].symbol_id,
            format!(
                "{}::helper",
                alpha_header.to_string_lossy().replace('\\', "/")
            )
        );
        let alpha_source_text = fs::read_to_string(&alpha_source).unwrap();
        let alpha_start = alpha_source_text.find("int helper(int value) {").unwrap();
        let alpha_end = alpha_source_text.find('}').map(|index| index + 1).unwrap();
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[0].node_kind,
            "function_definition"
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[0].origin_type,
            "companion_source"
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[0].evidence_key,
            result.validation.binding_decisions[0].candidates[0].evidence_key
        );
        assert!(
            result.validation.ambiguous_identifiers[0].candidates[0]
                .evidence_key
                .contains("function_definition|companion_source")
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[0].byte_range,
            (alpha_start, alpha_end)
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[0]
                .signature
                .as_deref(),
            Some("int helper(int value);")
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[1].symbol_id,
            format!(
                "{}::helper",
                zeta_header.to_string_lossy().replace('\\', "/")
            )
        );
        let zeta_source_text = fs::read_to_string(&zeta_source).unwrap();
        let zeta_start = zeta_source_text.find("int helper(int value) {").unwrap();
        let zeta_end = zeta_source_text.find('}').map(|index| index + 1).unwrap();
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[1].node_kind,
            "function_definition"
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[1].origin_type,
            "companion_source"
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[1].byte_range,
            (zeta_start, zeta_end)
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0].candidates[1]
                .signature
                .as_deref(),
            Some("int helper(int value);")
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0]
                .disambiguation_context
                .candidate_symbol_ids,
            vec![
                format!(
                    "{}::helper",
                    alpha_header.to_string_lossy().replace('\\', "/")
                ),
                format!(
                    "{}::helper",
                    zeta_header.to_string_lossy().replace('\\', "/")
                )
            ]
        );
    }

    #[test]
    fn allows_ambiguous_c_identifier_bindings_with_bypass() {
        let dir = temporary_dir();
        let alpha_header = dir.join("alpha.h");
        let alpha_source = dir.join("alpha.c");
        let zeta_header = dir.join("zeta.h");
        let zeta_source = dir.join("zeta.c");
        let caller = dir.join("caller.c");

        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let result = patch_ast_node_from_path(
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            Some("runtime wiring guarantees the intended helper target"),
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.bypass_applied);
        assert!(result.validation.commit_gate.allowed);
        assert_eq!(result.validation.commit_gate.status, "allowed_with_bypass");
        assert_eq!(
            result.validation.commit_gate.bypass_reason.as_deref(),
            Some("runtime wiring guarantees the intended helper target")
        );
        assert_eq!(result.validation.commit_gate.blocking_decisions.len(), 1);
        assert_eq!(
            result.validation.commit_gate.evidence_invariants[0].status,
            "blocked"
        );
        assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
        let updated = fs::read_to_string(&caller).unwrap();
        assert!(updated.contains("return helper(value);"));
    }

    #[test]
    fn reports_transitive_visible_include_families_for_c_ambiguity() {
        let dir = temporary_dir();
        let alpha_header = dir.join("alpha.h");
        let alpha_source = dir.join("alpha.c");
        let zeta_header = dir.join("zeta.h");
        let zeta_source = dir.join("zeta.c");
        let wrapper_header = dir.join("wrapper.h");
        let caller = dir.join("caller.c");

        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(
            &wrapper_header,
            "#include \"alpha.h\"\n#include \"zeta.h\"\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let result = patch_ast_node_from_path(
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            None,
        )
        .unwrap();

        assert_eq!(result.validation.ambiguous_identifiers.len(), 1);
        assert_eq!(
            result.validation.ambiguous_identifiers[0]
                .disambiguation_context
                .visible_include_families,
            vec![
                wrapper_header.to_string_lossy().replace('\\', "/"),
                alpha_header.to_string_lossy().replace('\\', "/"),
                zeta_header.to_string_lossy().replace('\\', "/")
            ]
        );
        assert_eq!(
            result.validation.ambiguous_identifiers[0]
                .disambiguation_context
                .candidate_include_families,
            vec![
                alpha_header.to_string_lossy().replace('\\', "/"),
                zeta_header.to_string_lossy().replace('\\', "/")
            ]
        );
    }

    #[test]
    fn replays_patch_evidence_against_matching_trace() {
        let dir = temporary_dir();
        let header = dir.join("helper.h");
        let source = dir.join("helper.c");
        let caller = dir.join("caller.c");

        fs::write(&header, "int helper(int value);\n").unwrap();
        fs::write(
            &source,
            "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let patch = patch_ast_node_from_path(
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            None,
        )
        .unwrap();
        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        let replay = replay_patch_evidence_against_trace(&patch, &trace);

        assert!(replay.consistent);
        assert_eq!(replay.matched_items, 1);
        assert_eq!(replay.blocked_items, 0);
        assert_eq!(replay.items.len(), 1);
        assert_eq!(replay.items[0].status, "matched");
        assert!(replay.items[0].matched_in_trace);
        assert_eq!(replay.items[0].trace_match_scope, "callees");
    }

    #[test]
    fn keeps_blocked_replay_items_for_ambiguous_patch_evidence() {
        let dir = temporary_dir();
        let alpha_header = dir.join("alpha.h");
        let alpha_source = dir.join("alpha.c");
        let zeta_header = dir.join("zeta.h");
        let zeta_source = dir.join("zeta.c");
        let caller = dir.join("caller.c");

        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"alpha.h\"\n#include \"zeta.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let patch = patch_ast_node_from_path(
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            None,
        )
        .unwrap();
        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        let replay = replay_patch_evidence_against_trace(&patch, &trace);

        assert!(replay.consistent);
        assert_eq!(replay.matched_items, 0);
        assert_eq!(replay.blocked_items, 1);
        assert_eq!(replay.items.len(), 1);
        assert_eq!(replay.items[0].status, "blocked");
        assert!(!replay.items[0].matched_in_trace);
        assert_eq!(replay.items[0].trace_match_scope, "none");
        assert_eq!(replay.items[0].candidate_evidence_keys.len(), 2);
    }

    #[test]
    fn allows_trace_validated_patch_commit_when_replay_matches() {
        let dir = temporary_dir();
        let header = dir.join("helper.h");
        let source = dir.join("helper.c");
        let caller = dir.join("caller.c");

        fs::write(&header, "int helper(int value);\n").unwrap();
        fs::write(
            &source,
            "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let patch = patch_ast_node_from_path(
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            None,
        )
        .unwrap();
        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        let decision = validate_patch_commit_with_trace(&patch, &trace);

        assert!(decision.allowed);
        assert_eq!(decision.status, "allowed");
        assert_eq!(decision.patch_gate_status, "allowed");
        assert_eq!(decision.replay_status, "matched");
        assert!(decision.replay.consistent);
    }

    #[test]
    fn rejects_trace_validated_patch_commit_when_replay_is_missing() {
        let dir = temporary_dir();
        let header = dir.join("helper.h");
        let caller = dir.join("caller.c");

        fs::write(&header, "int helper(int value);\n").unwrap();
        fs::write(
            &caller,
            "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let patch = patch_ast_node_from_path(
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            None,
        )
        .unwrap();
        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Callers).unwrap();
        let decision = validate_patch_commit_with_trace(&patch, &trace);

        assert!(!decision.allowed);
        assert_eq!(decision.status, "rejected_by_trace_replay");
        assert_eq!(decision.patch_gate_status, "allowed");
        assert_eq!(decision.replay_status, "missing");
        assert!(!decision.replay.consistent);
    }

    #[test]
    fn validates_patch_with_trace_context_in_one_call() {
        let dir = temporary_dir();
        let header = dir.join("helper.h");
        let source = dir.join("helper.c");
        let caller = dir.join("caller.c");

        fs::write(&header, "int helper(int value);\n").unwrap();
        fs::write(
            &source,
            "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include \"helper.h\"\n\nint orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let result = validate_patch_with_trace_context_from_path(
            &dir,
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value);\n}\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

        assert!(result.patch.applied);
        assert_eq!(result.trace_target, result.patch.resolved_symbol_id);
        assert!(result.trace.is_some());
        assert!(result.trace_validation.is_some());
        assert!(result.trace_error.is_none());
        assert!(
            result
                .trace_validation
                .as_ref()
                .is_some_and(|decision| decision.allowed)
        );
    }

    #[test]
    fn keeps_trace_error_when_context_patch_has_syntax_errors() {
        let dir = temporary_dir();
        let caller = dir.join("caller.c");

        fs::write(
            &caller,
            "int orchestrate(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();

        let result = validate_patch_with_trace_context_from_path(
            &dir,
            &caller,
            "orchestrate",
            "int orchestrate(int value) {\n    return helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

        assert!(!result.patch.applied);
        assert!(result.trace.is_none());
        assert!(result.trace_validation.is_none());
        assert_eq!(
            result.trace_error.as_deref(),
            Some("trace skipped because patch validation reported syntax errors")
        );
    }

    #[test]
    fn executes_tree_query() {
        let source = "def add(left, right):\n    return left + right\n";
        let query = "(function_definition name: (identifier) @name)";

        let captures = execute_tree_query(Path::new("sample.py"), source, query).unwrap();

        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].capture_name, "name");
        assert_eq!(captures[0].text, "add");
        assert_eq!(captures[0].owner_symbol_id.as_deref(), Some("add"));
        assert_eq!(captures[0].owner_semantic_path.as_deref(), Some("add"));
        assert_eq!(captures[0].owner_scope_path, None);
    }

    #[test]
    fn execute_tree_query_reports_owner_for_decorator_captures() {
        let source = "@logged\ndef top_level(value):\n    return value\n";
        let query = "(decorator (identifier) @decorator)";

        let captures = execute_tree_query(Path::new("sample.py"), source, query).unwrap();

        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].capture_name, "decorator");
        assert_eq!(captures[0].text, "logged");
        assert_eq!(captures[0].owner_symbol_id.as_deref(), Some("top_level"));
        assert_eq!(
            captures[0].owner_semantic_path.as_deref(),
            Some("top_level")
        );
        assert_eq!(captures[0].owner_scope_path, None);
    }

    #[test]
    fn execute_tree_query_reports_owner_for_c_body_captures() {
        let source = "int helper(int value) { return value + 1; }\nint orchestrate(int value) { return helper(value); }\n";
        let query = "(call_expression function: (identifier) @callee)";

        let captures = execute_tree_query(Path::new("sample.c"), source, query).unwrap();

        assert_eq!(captures.len(), 1);
        assert_eq!(captures[0].capture_name, "callee");
        assert_eq!(captures[0].text, "helper");
        assert_eq!(
            captures[0].owner_symbol_id.as_deref(),
            Some("sample.c::orchestrate")
        );
        assert_eq!(
            captures[0].owner_semantic_path.as_deref(),
            Some("orchestrate")
        );
        assert_eq!(captures[0].owner_scope_path, None);
    }

    #[test]
    fn rejects_patch_with_unresolved_identifier_without_bypass() {
        let source = r#"
def helper(value: int) -> int:
    return value + 1

def top_level(value: int) -> int:
    return helper(value)
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: int) -> int:\n    return missing_helper(value)\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert!(!result.validation.commit_gate.allowed);
        assert_eq!(result.validation.commit_gate.status, "rejected");
        assert_eq!(
            result.validation.commit_gate.reason,
            "symbol binding is unresolved"
        );
        assert_eq!(
            result.validation.unresolved_identifiers,
            vec!["missing_helper"]
        );
        assert_eq!(result.validation.binding_decisions.len(), 2);
        let missing_helper_decision = result
            .validation
            .binding_decisions
            .iter()
            .find(|decision| decision.name == "missing_helper")
            .unwrap();
        assert_eq!(missing_helper_decision.status, "unresolved");
        assert_eq!(missing_helper_decision.selected_symbol_id, None);
        assert!(missing_helper_decision.candidates.is_empty());
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "value" && decision.status == "resolved")
        );
    }

    #[test]
    fn ignores_python_type_annotations_during_patch_binding_validation() {
        let source = r#"
def top_level(value: int) -> int:
    return value
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: MissingType) -> MissingReturn:\n    return value\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.unresolved_identifiers.is_empty());
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "value" && decision.status == "resolved")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .all(|decision| decision.name != "MissingType" && decision.name != "MissingReturn")
        );
    }

    #[test]
    fn validates_python_default_parameter_references() {
        let source = r#"
def top_level(value: int) -> int:
    return value
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: int = missing_default) -> int:\n    return value\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert_eq!(
            result.validation.unresolved_identifiers,
            vec!["missing_default"]
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "missing_default"
                    && decision.status == "unresolved")
        );
    }

    #[test]
    fn validates_python_default_parameter_scope() {
        let source = r#"
MODULE_DEFAULT = 1

def top_level(value: int) -> int:
    return value
"#;

        let allowed = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: int = MODULE_DEFAULT) -> int:\n    return value\n",
            None,
        )
        .unwrap();

        assert!(allowed.applied);
        assert!(
            allowed
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "MODULE_DEFAULT" && decision.status == "resolved")
        );

        let rejected = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: int, other=value) -> int:\n    return other\n",
            None,
        )
        .unwrap();

        assert!(!rejected.applied);
        assert_eq!(rejected.validation.unresolved_identifiers, vec!["value"]);
        assert!(
            rejected
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "value" && decision.status == "unresolved")
        );
        assert!(
            rejected
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "other" && decision.status == "resolved")
        );
    }

    #[test]
    fn resolves_python_patch_bindings_with_semantic_metadata() {
        let source = r#"
def helper(value: int) -> int:
    """Shared helper."""
    return value + 1

def top_level(value: int) -> int:
    local_bonus = 2
    return helper(value) + local_bonus
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: int) -> int:\n    local_bonus = 3\n    return helper(value) + local_bonus\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.commit_gate.allowed);
        assert_eq!(result.validation.unresolved_identifiers.len(), 0);
        assert_eq!(result.validation.ambiguous_identifiers.len(), 0);
        assert_eq!(result.validation.resolved_identifiers.len(), 3);

        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.semantic_path, "helper");
        assert_eq!(helper_binding.symbol.scope_path, None);
        assert_eq!(
            helper_binding.symbol.signature.as_deref(),
            Some("def helper(value: int) -> int:")
        );
        assert_eq!(
            helper_binding.symbol.parameters,
            vec!["value: int".to_string()]
        );
        assert_eq!(helper_binding.symbol.return_type.as_deref(), Some("int"));
        assert_eq!(
            helper_binding.symbol.docstring.as_deref(),
            Some("\"\"\"Shared helper.\"\"\"")
        );

        let local_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "local_bonus")
            .unwrap();
        assert_eq!(local_binding.symbol.semantic_path, "local_bonus");
        assert_eq!(
            local_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
        assert_eq!(local_binding.symbol.node_kind, "assignment");
        assert_eq!(local_binding.symbol.origin_type, "local_scope");
        assert!(local_binding.symbol.signature.is_none());
        assert!(local_binding.symbol.parameters.is_empty());
        assert!(local_binding.symbol.return_type.is_none());
        assert!(local_binding.symbol.docstring.is_none());
    }

    #[test]
    fn resolves_python_with_statement_bindings() {
        let source = r#"
def manager():
    return object()

def top_level() -> object:
    return manager()
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> object:\n    with manager() as handle:\n        return handle\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "manager" && decision.status == "resolved")
        );
        let handle_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "handle")
            .unwrap();
        assert_eq!(handle_binding.symbol.node_kind, "with_target");
        assert_eq!(
            handle_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_except_clause_bindings() {
        let source = r#"
def risky():
    raise ValueError()

def top_level() -> object:
    return risky()
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> object:\n    try:\n        risky()\n    except ValueError as exc:\n        return exc\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "risky" && decision.status == "resolved")
        );
        let exc_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "exc")
            .unwrap();
        assert_eq!(exc_binding.symbol.node_kind, "except_target");
        assert_eq!(exc_binding.symbol.scope_path.as_deref(), Some("top_level"));
    }

    #[test]
    fn rejects_python_post_except_target_references() {
        let source = r#"
def exc() -> int:
    return 1

def risky() -> int:
    raise ValueError()

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    try:\n        risky()\n    except ValueError as exc:\n        return 0\n    return exc()\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert_eq!(result.validation.unresolved_identifiers, vec!["exc"]);
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "risky" && decision.status == "resolved")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "exc" && decision.status == "unresolved")
        );
    }

    #[test]
    fn rejects_python_pre_except_target_references() {
        let source = r#"
def exc() -> int:
    return 1

def risky() -> int:
    return 2

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    before = exc\n    try:\n        risky()\n    except ValueError as exc:\n        return before\n    return 0\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert_eq!(result.validation.unresolved_identifiers, vec!["exc"]);
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "risky" && decision.status == "resolved")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "exc" && decision.status == "unresolved")
        );
    }

    #[test]
    fn rejects_python_mixed_except_target_reference_states() {
        let source = r#"
def exc() -> int:
    return 1

def risky() -> int:
    return 2

def handle(value: object) -> object:
    return value

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    try:\n        risky()\n    except ValueError as exc:\n        handle(exc)\n    return exc()\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert_eq!(result.validation.unresolved_identifiers, vec!["exc"]);
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "risky" && decision.status == "resolved")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "handle" && decision.status == "resolved")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "exc" && decision.status == "unresolved")
        );
    }

    #[test]
    fn resolves_python_block_local_bindings() {
        let source = r#"
def top_level(flag: bool) -> int:
    return 1
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(flag: bool) -> int:\n    if flag:\n        branch_value = 2\n    return branch_value\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let branch_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "branch_value")
            .unwrap();
        assert_eq!(branch_binding.symbol.node_kind, "assignment");
        assert_eq!(
            branch_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_global_declared_patch_bindings() {
        let source = r#"
def helper() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    global helper\n    helper = helper\n    return helper()\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.semantic_path, "helper");
        assert_eq!(helper_binding.symbol.node_kind, "function_definition");
    }

    #[test]
    fn resolves_python_match_keyword_patch_bindings() {
        let source = r#"
class Point:
    __match_args__ = ()

def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case Point(value=target):\n            return target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_match_capture_patch_bindings() {
        let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case {\"target\": target}:\n            return target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_match_alias_patch_bindings() {
        let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case int() as target:\n            return target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_match_splat_patch_bindings() {
        let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case [*target]:\n            return target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_match_list_capture_patch_bindings() {
        let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case [target]:\n            return target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_match_mapping_splat_patch_bindings() {
        let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case {\"x\": _, **target}:\n            return target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_match_class_positional_patch_bindings() {
        let source = r#"
class Point:
    __match_args__ = ("value",)

def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case Point(target):\n            return target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_match_union_patch_bindings() {
        let source = r#"
class Point:
    __match_args__ = ("value",)

class Value:
    __match_args__ = ("value",)

def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case Point(target) | Value(target):\n            return target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn rejects_python_match_guard_references_after_prior_capture() {
        let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case [target]:\n            return 0\n        case _ if target():\n            return 1\n        case _:\n            return 2\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert!(
            result
                .validation
                .unresolved_identifiers
                .iter()
                .any(|name| name == "target")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "target" && decision.status == "unresolved")
        );
    }

    #[test]
    fn resolves_python_match_mixed_mapping_patch_bindings() {
        let source = r#"
def target() -> int:
    return 1

def top_level(value) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value) -> int:\n    match value:\n        case {\"x\": x, **target}:\n            return x + target\n        case _:\n            return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "match_capture");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );

        let x_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "x")
            .unwrap();
        assert_eq!(x_binding.symbol.node_kind, "match_capture");
        assert_eq!(x_binding.symbol.scope_path.as_deref(), Some("top_level"));
    }

    #[test]
    fn resolves_python_global_references_inside_nested_functions_despite_outer_shadowing() {
        let source = r#"
def helper() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    helper = 2\n\n    def inner() -> int:\n        global helper\n        return helper()\n\n    return inner()\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.semantic_path, "helper");
        assert_eq!(helper_binding.symbol.node_kind, "function_definition");
    }

    #[test]
    fn resolves_python_comprehension_target_patch_bindings() {
        let source = r#"
def item() -> int:
    return 1

def top_level(values: list[int]) -> object:
    return values
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(values: list[int]) -> object:\n    return [item for item in values]\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let item_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "item")
            .unwrap();
        assert_eq!(item_binding.symbol.node_kind, "comprehension_target");
        assert_eq!(item_binding.symbol.scope_path.as_deref(), Some("top_level"));
    }

    #[test]
    fn resolves_python_comprehension_target_body_bindings_without_global_shadowing() {
        let source = r#"
def top_level(values: list[int]) -> object:
    return values
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(values: list[int]) -> object:\n    return [item for item in values]\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let item_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "item")
            .unwrap();
        assert_eq!(item_binding.symbol.node_kind, "comprehension_target");
        assert_eq!(item_binding.symbol.scope_path.as_deref(), Some("top_level"));
    }

    #[test]
    fn resolves_python_comprehension_target_filter_bindings() {
        let source = r#"
def top_level(values: list[int]) -> object:
    return values
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(values: list[int]) -> object:\n    return [item for item in values if item]\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let item_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "item")
            .unwrap();
        assert_eq!(item_binding.symbol.node_kind, "comprehension_target");
        assert_eq!(item_binding.symbol.scope_path.as_deref(), Some("top_level"));
    }

    #[test]
    fn resolves_python_named_expression_bindings() {
        let source = r#"
def top_level(items: list[int]) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(items: list[int]) -> int:\n    if (count := len(items)):\n        return count\n    return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let count_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "count")
            .unwrap();
        assert_eq!(count_binding.symbol.node_kind, "named_expression");
        assert_eq!(
            count_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "items" && decision.status == "resolved")
        );
    }

    #[test]
    fn rejects_python_pre_named_expression_references() {
        let source = r#"
def target() -> int:
    return 1

def top_level(flag: bool) -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(flag: bool) -> int:\n    before = target\n    if flag and (target := 3):\n        return before\n    return before\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert_eq!(result.validation.unresolved_identifiers, vec!["target"]);
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "flag" && decision.status == "resolved")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "target" && decision.status == "unresolved")
        );
    }

    #[test]
    fn rejects_python_pre_named_expression_references_inside_comprehensions() {
        let source = r#"
def target() -> int:
    return 1

def top_level(values: list[int]) -> object:
    return values
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(values: list[int]) -> object:\n    return [target + (target := item) for item in values]\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert_eq!(result.validation.unresolved_identifiers, vec!["target"]);
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "item" && decision.status == "resolved")
        );
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "target" && decision.status == "unresolved")
        );
    }

    #[test]
    fn resolves_python_named_expression_references_after_binding_inside_comprehensions() {
        let source = r#"
def top_level(values: list[int]) -> object:
    return values
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(values: list[int]) -> object:\n    return [(target := item) + target for item in values]\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "named_expression");
        let item_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "item")
            .unwrap();
        assert_eq!(item_binding.symbol.node_kind, "comprehension_target");
    }

    #[test]
    fn resolves_python_lambda_parameter_bindings() {
        let source = r#"
def target() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    return (lambda target: target)(3)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "parameter");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_lambda_default_parameter_references() {
        let source = r#"
def target() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    return (lambda x=target(): x)()\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "function_definition");
        let x_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "x")
            .unwrap();
        assert_eq!(x_binding.symbol.node_kind, "parameter");
    }

    #[test]
    fn resolves_python_async_function_patch_bindings() {
        let source = r#"
def helper(value: int) -> int:
    return value + 1

async def top_level(value: int) -> int:
    return value
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "async def top_level(value: int) -> int:\n    return helper(value)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert_eq!(result.resolved_path, "top_level");
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.node_kind, "function_definition");
        assert_eq!(helper_binding.symbol.semantic_path, "helper");
    }

    #[test]
    fn replaces_python_decorated_function_without_retaining_old_decorators() {
        let source = r#"
def decorator(func):
    return func

@decorator
def top_level() -> int:
    return 1
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    return 2\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(!result.updated_source.contains("@decorator"));
        assert_eq!(result.resolved_path, "top_level");
    }

    #[test]
    fn replaces_python_decorated_class_without_retaining_old_decorators() {
        let source = r#"
def decorator(cls):
    return cls

@decorator
class Container:
    value = 1
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container",
            "class Container:\n    value = 2\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(!result.updated_source.contains("@decorator"));
        assert_eq!(result.resolved_path, "Container");
    }

    #[test]
    fn replaces_python_decorated_async_function_without_retaining_old_decorators() {
        let source = r#"
def decorator(func):
    return func

@decorator
async def top_level() -> int:
    return 1
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "async def top_level() -> int:\n    return 2\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(!result.updated_source.contains("@decorator"));
        assert_eq!(result.resolved_path, "top_level");
    }

    #[test]
    fn replaces_python_async_function_without_retaining_old_async_keyword() {
        let source = r#"
async def top_level() -> int:
    return 1
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    return 2\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(
            !result
                .updated_source
                .contains("async def top_level() -> int:\n    return 2")
        );
        assert!(
            result
                .updated_source
                .contains("def top_level() -> int:\n    return 2")
        );
        assert_eq!(result.resolved_path, "top_level");
    }

    #[test]
    fn resolves_python_nested_default_parameter_closure_bindings() {
        let source = r#"
def helper() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    helper = 3\n\n    def inner(value=helper) -> int:\n        return value\n\n    return inner()\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.node_kind, "assignment");
        assert_eq!(
            helper_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn rejects_python_class_lambda_references_to_class_locals() {
        let source = r#"
class Container:
    value = 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container",
            "class Container:\n    helper = 2\n    value = (lambda: helper)()\n",
            None,
        )
        .unwrap();

        assert!(!result.applied);
        assert_eq!(result.validation.unresolved_identifiers, vec!["helper"]);
        assert!(
            result
                .validation
                .binding_decisions
                .iter()
                .any(|decision| decision.name == "helper" && decision.status == "unresolved")
        );
    }

    #[test]
    fn resolves_python_class_body_references_to_class_locals() {
        let source = r#"
class Container:
    value = 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container",
            "class Container:\n    helper = 2\n    value = helper\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.node_kind, "assignment");
        assert_eq!(
            helper_binding.symbol.scope_path.as_deref(),
            Some("Container")
        );
    }

    #[test]
    fn resolves_python_class_method_default_parameter_references_to_class_locals() {
        let source = r#"
def helper() -> int:
    return 1

class Container:
    helper = 2

    def method(value=None) -> object:
        return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container.method",
            "def method(value=helper) -> object:\n    return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.node_kind, "assignment");
        assert_eq!(
            helper_binding.symbol.scope_path.as_deref(),
            Some("Container")
        );
    }

    #[test]
    fn resolves_python_nested_decorator_closure_bindings() {
        let source = r#"
def helper(func):
    return func

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    helper = lambda func: func\n\n    @helper\n    def inner() -> int:\n        return 1\n\n    return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.node_kind, "assignment");
        assert_eq!(
            helper_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_class_method_decorator_references_to_class_locals() {
        let source = r#"
def helper(func):
    return func

class Container:
    def method(self) -> int:
        return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container",
            "class Container:\n    helper = helper\n\n    @helper\n    def method(self) -> int:\n        return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.node_kind, "assignment");
        assert_eq!(
            helper_binding.symbol.scope_path.as_deref(),
            Some("Container")
        );
    }

    #[test]
    fn resolves_python_nested_class_base_closure_bindings() {
        let source = r#"
class GlobalBase:
    pass

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    Base = GlobalBase\n\n    class Inner(Base):\n        pass\n\n    return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let base_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "Base")
            .unwrap();
        assert_eq!(base_binding.symbol.node_kind, "assignment");
        assert_eq!(base_binding.symbol.scope_path.as_deref(), Some("top_level"));
    }

    #[test]
    fn resolves_python_class_base_references_to_globals_not_class_locals() {
        let source = r#"
class Base:
    pass

class Container:
    value = 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container",
            "class Container(Base):\n    Base = 1\n    value = 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let base_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "Base")
            .unwrap();
        assert_eq!(base_binding.symbol.node_kind, "class_definition");
        assert_eq!(base_binding.symbol.semantic_path, "Base");
    }

    #[test]
    fn resolves_python_nested_class_metaclass_closure_bindings() {
        let source = r#"
class GlobalMeta(type):
    pass

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    Meta = GlobalMeta\n\n    class Inner(metaclass=Meta):\n        pass\n\n    return 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let meta_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "Meta")
            .unwrap();
        assert_eq!(meta_binding.symbol.node_kind, "assignment");
        assert_eq!(meta_binding.symbol.scope_path.as_deref(), Some("top_level"));
    }

    #[test]
    fn resolves_python_class_metaclass_references_to_globals_not_class_locals() {
        let source = r#"
class Meta(type):
    pass

class Container:
    value = 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container",
            "class Container(metaclass=Meta):\n    Meta = 1\n    value = 0\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let meta_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "Meta")
            .unwrap();
        assert_eq!(meta_binding.symbol.node_kind, "class_definition");
        assert_eq!(meta_binding.symbol.semantic_path, "Meta");
    }

    #[test]
    fn resolves_python_class_comprehension_references_to_globals_not_class_locals() {
        let source = r#"
def helper() -> int:
    return 1

class Container:
    value = 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "Container",
            "class Container:\n    helper = 2\n    value = [helper for item in range(1)]\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.node_kind, "function_definition");
        assert_eq!(helper_binding.symbol.semantic_path, "helper");
    }

    #[test]
    fn resolves_python_nested_lambda_closure_bindings() {
        let source = r#"
def target() -> int:
    return 1

def top_level() -> int:
    return 0
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level() -> int:\n    return (lambda target: (lambda: target)())(3)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        let target_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "target")
            .unwrap();
        assert_eq!(target_binding.symbol.node_kind, "parameter");
        assert_eq!(
            target_binding.symbol.scope_path.as_deref(),
            Some("top_level")
        );
    }

    #[test]
    fn resolves_python_import_alias_patch_bindings_to_local_module_symbols() {
        let dir = temporary_dir();
        let helper = dir.join("graph_b.py");
        let caller = dir.join("caller.py");

        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    \"\"\"Imported helper.\"\"\"\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let source = fs::read_to_string(&caller).unwrap();
        let result = patch_ast_node(
            &caller,
            &source,
            "top_level",
            "def top_level(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.commit_gate.allowed);
        assert!(result.validation.unresolved_identifiers.is_empty());

        let alias_attribute = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "gb.helper")
            .unwrap();
        assert_eq!(alias_attribute.symbol.semantic_path, "helper");
        assert_eq!(
            alias_attribute.symbol.file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(alias_attribute.symbol.origin_type, "imported_module");
        assert_eq!(
            alias_attribute.symbol.docstring.as_deref(),
            Some("\"\"\"Imported helper.\"\"\"")
        );

        let alias_import = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "h")
            .unwrap();
        assert_eq!(alias_import.symbol.semantic_path, "helper");
        assert_eq!(
            alias_import.symbol.file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(alias_import.symbol.origin_type, "imported_module");
    }

    #[test]
    fn resolves_python_relative_import_alias_patch_bindings_to_local_module_symbols() {
        let dir = temporary_dir();
        let package = dir.join("pkg");
        let subpackage = package.join("sub");
        let helper = package.join("graph_b.py");
        let local_helper = subpackage.join("local_mod.py");
        let caller = subpackage.join("caller.py");

        fs::create_dir_all(&subpackage).unwrap();
        fs::write(package.join("__init__.py"), "").unwrap();
        fs::write(subpackage.join("__init__.py"), "").unwrap();
        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    \"\"\"Parent package helper.\"\"\"\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &local_helper,
            "def helper2(value: int) -> int:\n    \"\"\"Sibling package helper.\"\"\"\n    return value + 2\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from ..graph_b import helper as h\nfrom .local_mod import helper2 as h2\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let source = fs::read_to_string(&caller).unwrap();
        let result = patch_ast_node(
            &caller,
            &source,
            "top_level",
            "def top_level(value: int) -> int:\n    return h(value) + h2(value)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.commit_gate.allowed);
        assert!(result.validation.unresolved_identifiers.is_empty());

        let imported_helper = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "h")
            .unwrap();
        assert_eq!(imported_helper.symbol.semantic_path, "helper");
        assert_eq!(
            imported_helper.symbol.file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(imported_helper.symbol.origin_type, "imported_module");
        assert_eq!(
            imported_helper.symbol.docstring.as_deref(),
            Some("\"\"\"Parent package helper.\"\"\"")
        );

        let sibling_helper = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "h2")
            .unwrap();
        assert_eq!(sibling_helper.symbol.semantic_path, "helper2");
        assert_eq!(
            sibling_helper.symbol.file_path,
            local_helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(sibling_helper.symbol.origin_type, "imported_module");
        assert_eq!(
            sibling_helper.symbol.docstring.as_deref(),
            Some("\"\"\"Sibling package helper.\"\"\"")
        );
    }

    #[test]
    fn resolves_python_absolute_package_import_alias_patch_bindings_to_local_module_symbols() {
        let dir = temporary_dir();
        let package = dir.join("pkg");
        let subpackage = package.join("sub");
        let helper = package.join("graph_c.py");
        let caller = subpackage.join("caller.py");

        fs::create_dir_all(&subpackage).unwrap();
        fs::write(package.join("__init__.py"), "").unwrap();
        fs::write(subpackage.join("__init__.py"), "").unwrap();
        fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Absolute package worker.\"\"\"\n    return value + 3\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import pkg.graph_c as gc\nfrom pkg.graph_c import worker as w\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let source = fs::read_to_string(&caller).unwrap();
        let result = patch_ast_node(
            &caller,
            &source,
            "top_level",
            "def top_level(value: int) -> int:\n    return gc.worker(value) + w(value)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.commit_gate.allowed);
        assert!(result.validation.unresolved_identifiers.is_empty());

        let module_alias = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "gc.worker")
            .unwrap();
        assert_eq!(module_alias.symbol.semantic_path, "worker");
        assert_eq!(
            module_alias.symbol.file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(module_alias.symbol.origin_type, "imported_module");

        let symbol_alias = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "w")
            .unwrap();
        assert_eq!(symbol_alias.symbol.semantic_path, "worker");
        assert_eq!(
            symbol_alias.symbol.file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(symbol_alias.symbol.origin_type, "imported_module");
        assert_eq!(
            symbol_alias.symbol.docstring.as_deref(),
            Some("\"\"\"Absolute package worker.\"\"\"")
        );
    }

    #[test]
    fn resolves_python_import_from_module_alias_patch_bindings_to_local_module_symbols() {
        let dir = temporary_dir();
        let package = dir.join("pkg");
        let subpackage = package.join("sub");
        let helper = package.join("graph_c.py");
        let local_helper = subpackage.join("local_mod.py");
        let caller = subpackage.join("caller.py");

        fs::create_dir_all(&subpackage).unwrap();
        fs::write(package.join("__init__.py"), "").unwrap();
        fs::write(subpackage.join("__init__.py"), "").unwrap();
        fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Absolute package worker.\"\"\"\n    return value + 3\n",
        )
        .unwrap();
        fs::write(
            &local_helper,
            "def helper2(value: int) -> int:\n    \"\"\"Sibling module helper.\"\"\"\n    return value + 2\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from pkg import graph_c as gc\nfrom . import local_mod as lm\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let source = fs::read_to_string(&caller).unwrap();
        let result = patch_ast_node(
            &caller,
            &source,
            "top_level",
            "def top_level(value: int) -> int:\n    return gc.worker(value) + lm.helper2(value)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.commit_gate.allowed);
        assert!(result.validation.unresolved_identifiers.is_empty());

        let package_module_alias = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "gc.worker")
            .unwrap();
        assert_eq!(package_module_alias.symbol.semantic_path, "worker");
        assert_eq!(
            package_module_alias.symbol.file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(package_module_alias.symbol.origin_type, "imported_module");

        let sibling_module_alias = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "lm.helper2")
            .unwrap();
        assert_eq!(sibling_module_alias.symbol.semantic_path, "helper2");
        assert_eq!(
            sibling_module_alias.symbol.file_path,
            local_helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(sibling_module_alias.symbol.origin_type, "imported_module");
    }

    #[test]
    fn resolves_python_package_reexport_patch_bindings_to_underlying_local_symbols() {
        let dir = temporary_dir();
        let package = dir.join("pkg");
        let helper = package.join("graph_c.py");
        let caller = dir.join("caller.py");

        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("__init__.py"),
            "from .graph_c import worker as worker\n",
        )
        .unwrap();
        fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Re-exported package worker.\"\"\"\n    return value + 4\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from pkg import worker\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let source = fs::read_to_string(&caller).unwrap();
        let result = patch_ast_node(
            &caller,
            &source,
            "top_level",
            "def top_level(value: int) -> int:\n    return worker(value)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.commit_gate.allowed);
        assert!(result.validation.unresolved_identifiers.is_empty());

        let imported_worker = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "worker")
            .unwrap();
        assert_eq!(imported_worker.symbol.semantic_path, "worker");
        assert_eq!(
            imported_worker.symbol.file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(imported_worker.symbol.origin_type, "imported_module");
        assert_eq!(
            imported_worker.symbol.docstring.as_deref(),
            Some("\"\"\"Re-exported package worker.\"\"\"")
        );
    }

    #[test]
    fn resolves_decorated_python_local_bindings_for_patch_validation() {
        let source = r#"
def decorator(func):
    return func

@decorator
def helper(value: int) -> int:
    return value + 1

def top_level(value: int) -> int:
    return value + 1
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: int) -> int:\n    return helper(value)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.commit_gate.allowed);
        assert!(result.validation.unresolved_identifiers.is_empty());

        let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
        let helper_start = source.find(helper_text).unwrap();
        let helper_end = helper_start + helper_text.len();

        let helper_binding = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "helper")
            .unwrap();
        assert_eq!(helper_binding.symbol.semantic_path, "helper");
        assert_eq!(helper_binding.symbol.origin_type, "module_scope");
        assert_eq!(
            helper_binding.symbol.signature.as_deref(),
            Some("@decorator\ndef helper(value: int) -> int:")
        );
        assert_eq!(helper_binding.symbol.byte_range, (helper_start, helper_end));
    }

    #[test]
    fn resolves_decorated_python_import_metadata_for_patch_validation() {
        let dir = temporary_dir();
        let helper = dir.join("graph_b.py");
        let caller = dir.join("caller.py");

        fs::write(
            &helper,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let source = fs::read_to_string(&caller).unwrap();
        let result = patch_ast_node(
            &caller,
            &source,
            "top_level",
            "def top_level(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
            None,
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.validation.commit_gate.allowed);
        assert!(result.validation.unresolved_identifiers.is_empty());

        let helper_source = fs::read_to_string(&helper).unwrap();
        let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
        let helper_start = helper_source.find(helper_text).unwrap();
        let helper_end = helper_start + helper_text.len();

        let imported_helper = result
            .validation
            .resolved_identifiers
            .iter()
            .find(|binding| binding.name == "h")
            .unwrap();
        assert_eq!(imported_helper.symbol.semantic_path, "helper");
        assert_eq!(imported_helper.symbol.origin_type, "imported_module");
        assert_eq!(
            imported_helper.symbol.signature.as_deref(),
            Some("@decorator\ndef helper(value: int) -> int:")
        );
        assert_eq!(
            imported_helper.symbol.byte_range,
            (helper_start, helper_end)
        );
        assert!(
            imported_helper
                .symbol
                .evidence_key
                .contains("@decorator\ndef helper(value: int) -> int:")
        );
    }

    #[test]
    fn allows_patch_with_bypass_reason() {
        let source = r#"
def top_level(value: int) -> int:
    return value + 1
"#;

        let result = patch_ast_node(
            Path::new("sample.py"),
            source,
            "top_level",
            "def top_level(value: int) -> int:\n    return dynamic_runtime(value)\n",
            Some("resolved at runtime by the embedding host"),
        )
        .unwrap();

        assert!(result.applied);
        assert!(result.bypass_applied);
        assert!(result.validation.commit_gate.allowed);
        assert_eq!(result.validation.commit_gate.status, "allowed_with_bypass");
        assert_eq!(
            result.validation.commit_gate.bypass_reason.as_deref(),
            Some("resolved at runtime by the embedding host")
        );
    }

    #[test]
    fn writes_applied_patch_to_disk() {
        let dir = temporary_dir();
        let file = dir.join("patch_target.py");
        fs::write(
            &file,
            "def top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let result = patch_ast_node_from_path(
            &file,
            "top_level",
            "def top_level(value: int) -> int:\n    return value + 2\n",
            None,
        )
        .unwrap();

        let updated = fs::read_to_string(&file).unwrap();
        assert!(result.applied);
        assert!(updated.contains("return value + 2"));
    }

    #[test]
    fn traces_symbol_graph_across_python_files() {
        let workspace_root = Path::new("../../tests/fixtures");
        let trace =
            trace_symbol_graph(workspace_root, "orchestrate", TraceDirection::Both).unwrap();

        assert_eq!(trace.symbol.semantic_path, "orchestrate");
        assert_eq!(trace.symbol.scope_path, None);
        assert_eq!(trace.symbol.parameters, vec!["value: int".to_string()]);
        assert_eq!(trace.symbol.return_type.as_deref(), Some("int"));
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.parameters == vec!["value: int".to_string()])
        );
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.return_type.as_deref() == Some("int"))
        );

        let leaf_trace =
            trace_symbol_graph(workspace_root, "leaf", TraceDirection::Callers).unwrap();
        assert!(
            leaf_trace
                .callers
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn ignores_python_local_variable_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("shadow.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def orchestrate():\n    helper = 2\n    return helper\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn traces_python_global_declared_references() {
        let dir = temporary_dir();
        let source = dir.join("global_decl.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def orchestrate():\n    global helper\n    helper = helper\n    return helper()\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn traces_python_default_parameter_references_despite_local_shadowing() {
        let dir = temporary_dir();
        let source = dir.join("default_param_shadow.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def orchestrate(value=helper()):\n    helper = 2\n    return value\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn ignores_python_nonlocal_outer_variable_references_in_nested_traces() {
        let dir = temporary_dir();
        let source = dir.join("nonlocal_outer_variable.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 2\n\n    def inner():\n        nonlocal helper\n        return helper\n\n    return inner()\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn traces_python_nonlocal_outer_function_references_in_nested_traces() {
        let dir = temporary_dir();
        let source = dir.join("nonlocal_outer_function.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    def helper():\n        return 2\n\n    def inner():\n        nonlocal helper\n        return helper()\n\n    return inner()\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "top_level.helper")
        );
        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn traces_python_global_references_inside_nested_functions_despite_outer_shadowing() {
        let dir = temporary_dir();
        let source = dir.join("nested_global_shadow.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 2\n\n    def inner():\n        global helper\n        return helper()\n\n    return inner()\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn ignores_python_post_except_target_global_fallback_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("post_except_target.py");

        fs::write(
            &source,
            "def exc():\n    return 1\n\n\
def risky():\n    raise ValueError()\n\n\
def orchestrate():\n    try:\n        risky()\n    except ValueError as exc:\n        return 0\n    return exc()\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "risky")
        );
        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "exc")
        );
    }

    #[test]
    fn ignores_python_pre_except_target_global_fallback_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("pre_except_target.py");

        fs::write(
            &source,
            "def exc():\n    return 1\n\n\
def risky():\n    raise ValueError()\n\n\
def orchestrate():\n    before = exc\n    try:\n        risky()\n    except ValueError as exc:\n        return before\n    return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "risky")
        );
        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "exc")
        );
    }

    #[test]
    fn ignores_python_named_expression_global_fallback_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("named_expression_shadow.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(flag):\n    before = target\n    if flag and (target := 3):\n        return before\n    return before\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_lambda_parameter_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("lambda_param_shadow.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda target: target)(3)\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn traces_python_lambda_default_parameter_references() {
        let dir = temporary_dir();
        let source = dir.join("lambda_default.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda x=target(): x)()\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );
    }

    #[test]
    fn traces_python_async_function_references() {
        let dir = temporary_dir();
        let source = dir.join("async_orchestrate.py");

        fs::write(
            &source,
            "def helper(value: int) -> int:\n    return value + 1\n\n\
async def orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn ignores_python_nested_default_parameter_closure_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_default_param_shadow.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 3\n\n    def inner(value=helper):\n        return value\n\n    return inner()\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_nested_lambda_parameter_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_lambda_param_shadow.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda target: (lambda: target)())(3)\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_class_lambda_local_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_lambda_shadow.py");

        fs::write(
            &source,
            "class Container:\n    helper = 2\n    value = (lambda: helper)()\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_class_method_default_parameter_local_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_method_default_shadow.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n\n    def method(value=helper):\n        return value\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "Container.method", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_nested_decorator_closure_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_decorator_shadow.py");

        fs::write(
            &source,
            "def helper(func):\n    return func\n\n\
def top_level():\n    helper = lambda func: func\n\n    @helper\n    def inner():\n        return 1\n\n    return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_class_method_decorator_local_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_method_decorator_shadow.py");

        fs::write(
            &source,
            "def helper(func):\n    return func\n\n\
class Container:\n    helper = helper\n\n    @helper\n    def method(self):\n        return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "Container.method", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_nested_class_base_local_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_class_base_shadow.py");

        fs::write(
            &source,
            "class GlobalBase:\n    pass\n\n\
def top_level():\n    Base = GlobalBase\n\n    class Inner(Base):\n        pass\n\n    return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "top_level.Inner", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn traces_python_class_base_global_fallbacks() {
        let dir = temporary_dir();
        let source = dir.join("class_base_global.py");

        fs::write(
            &source,
            "class Base:\n    pass\n\n\
class Container(Base):\n    Base = 1\n    value = 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Base")
        );
    }

    #[test]
    fn ignores_python_nested_class_metaclass_local_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_class_metaclass_shadow.py");

        fs::write(
            &source,
            "class GlobalMeta(type):\n    pass\n\n\
def top_level():\n    Meta = GlobalMeta\n\n    class Inner(metaclass=Meta):\n        pass\n\n    return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "top_level.Inner", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn traces_python_class_metaclass_global_fallbacks() {
        let dir = temporary_dir();
        let source = dir.join("class_metaclass_global.py");

        fs::write(
            &source,
            "class Meta(type):\n    pass\n\n\
class Container(metaclass=Meta):\n    Meta = 1\n    value = 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Meta")
        );
    }

    #[test]
    fn ignores_python_class_body_local_references_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_local_reference.py");

        fs::write(
            &source,
            "class Container:\n    helper = 2\n    value = helper\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn traces_python_class_comprehension_global_fallbacks() {
        let dir = temporary_dir();
        let source = dir.join("class_comprehension_global.py");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n    value = [helper for item in range(1)]\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn collects_python_class_comprehension_global_reference_names() {
        let source = "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n    value = [helper for item in range(1)]\n";
        let document = crate::language::parse_document(Path::new("sample.py"), source).unwrap();
        let mut class_range = None;
        let mut callback = |node: tree_sitter::Node<'_>| {
            if node.kind() == "class_definition"
                && crate::semantic::semantic_path(node, source)
                    .ok()
                    .is_some_and(|path| path == "Container")
            {
                class_range = Some((node.start_byte(), node.end_byte()));
            }
        };
        crate::language::visit_tree(document.tree.root_node(), &mut callback);
        let (start, end) = class_range.unwrap();
        let class_node = document
            .tree
            .root_node()
            .descendant_for_byte_range(start, end)
            .unwrap();
        assert_eq!(class_node.kind(), "class_definition");

        let mut references = std::collections::BTreeSet::new();
        crate::patching::collect_python_references(
            Path::new("sample.py"),
            class_node,
            source,
            &mut references,
        )
        .unwrap();

        assert!(
            references.contains("helper"),
            "references: {:?}",
            references
        );
    }

    #[test]
    fn traces_python_comprehension_call_references() {
        let dir = temporary_dir();
        let source = dir.join("comprehension.py");

        fs::write(
            &source,
            "def helper(value: int) -> int:\n    return value + 1\n\n\
def orchestrate(items: list[int]) -> list[int]:\n    return [helper(item) for item in items]\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn ignores_python_comprehension_target_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("comprehension_shadow.py");

        fs::write(
            &source,
            "def item():\n    return 1\n\n\
def orchestrate(values):\n    return [item for item in values]\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_match_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_capture.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case {\"target\": target}:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_pre_match_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_pre_capture.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    before = target\n    match value:\n        case {\"target\": target}:\n            return before\n        case _:\n            return before\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_match_alias_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_alias.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case int() as target:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_match_keyword_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_keyword_capture.py");

        fs::write(
            &source,
            "class Point:\n    __match_args__ = ()\n\ndef target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case Point(value=target):\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );
    }

    #[test]
    fn ignores_python_match_splat_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_splat_capture.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case [*target]:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_match_list_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_list_capture.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case [target]:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_match_mapping_splat_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_mapping_splat_capture.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case {\"x\": _, **target}:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_match_class_positional_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_class_positional_capture.py");

        fs::write(
            &source,
            "class Point:\n    __match_args__ = (\"value\",)\n\n\
def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case Point(target):\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Point")
        );
        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );
    }

    #[test]
    fn ignores_python_match_union_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_union_capture.py");

        fs::write(
            &source,
            "class Point:\n    __match_args__ = (\"value\",)\n\n\
class Value:\n    __match_args__ = (\"value\",)\n\n\
def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case Point(target) | Value(target):\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Point")
        );
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Value")
        );
        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );
    }

    #[test]
    fn ignores_python_match_guard_global_fallback_after_prior_capture_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_guard_reference.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case [target]:\n            return 0\n        case _ if target():\n            return 1\n        case _:\n            return 2\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );
    }

    #[test]
    fn ignores_python_match_capture_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_capture.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case {\"target\": target}:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_pre_match_capture_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_pre_capture.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    before = target\n    match value:\n        case {\"target\": target}:\n            return before\n        case _:\n            return before\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn traces_python_default_parameter_references_despite_local_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("default_param_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def orchestrate(value=helper()):\n    helper = 2\n    return value\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn traces_python_global_references_inside_nested_functions_despite_outer_shadowing_in_persisted_traces()
     {
        let dir = temporary_dir();
        let source = dir.join("nested_global_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 2\n\n    def inner():\n        global helper\n        return helper()\n\n    return inner()\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both)
                .unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn ignores_python_post_except_target_global_fallback_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("post_except_target.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def exc():\n    return 1\n\n\
def risky():\n    raise ValueError()\n\n\
def orchestrate():\n    try:\n        risky()\n    except ValueError as exc:\n        return 0\n    return exc()\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "risky")
        );
        assert!(
            !live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "exc")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "risky")
        );
        assert!(
            !persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "exc")
        );
    }

    #[test]
    fn ignores_python_pre_except_target_global_fallback_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("pre_except_target.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def exc():\n    return 1\n\n\
def risky():\n    raise ValueError()\n\n\
def orchestrate():\n    before = exc\n    try:\n        risky()\n    except ValueError as exc:\n        return before\n    return 0\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "risky")
        );
        assert!(
            !live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "exc")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "risky")
        );
        assert!(
            !persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "exc")
        );
    }

    #[test]
    fn ignores_python_named_expression_global_fallback_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("named_expression_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(flag):\n    before = target\n    if flag and (target := 3):\n        return before\n    return before\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_lambda_parameter_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("lambda_param_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda target: target)(3)\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn traces_python_lambda_default_parameter_references_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("lambda_default.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda x=target(): x)()\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );
    }

    #[test]
    fn traces_python_async_function_references_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("async_orchestrate.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper(value: int) -> int:\n    return value + 1\n\n\
async def orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn ignores_python_nested_default_parameter_closure_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_default_param_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 3\n\n    def inner(value=helper):\n        return value\n\n    return inner()\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both)
                .unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_nested_lambda_parameter_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_lambda_param_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate():\n    return (lambda target: (lambda: target)())(3)\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_class_method_default_parameter_local_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_method_default_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n\n    def method(value=helper):\n        return value\n",
        )
        .unwrap();

        let live_trace =
            trace_symbol_graph(&dir, "Container.method", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "Container.method", TraceDirection::Both)
                .unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_nested_decorator_closure_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_decorator_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper(func):\n    return func\n\n\
def top_level():\n    helper = lambda func: func\n\n    @helper\n    def inner():\n        return 1\n\n    return 0\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both)
                .unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_class_method_decorator_local_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_method_decorator_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper(func):\n    return func\n\n\
class Container:\n    helper = helper\n\n    @helper\n    def method(self):\n        return 0\n",
        )
        .unwrap();

        let live_trace =
            trace_symbol_graph(&dir, "Container.method", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "Container.method", TraceDirection::Both)
                .unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_nested_class_base_local_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_class_base_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "class GlobalBase:\n    pass\n\n\
def top_level():\n    Base = GlobalBase\n\n    class Inner(Base):\n        pass\n\n    return 0\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "top_level.Inner", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "top_level.Inner", TraceDirection::Both)
                .unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn traces_python_class_base_global_fallbacks_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_base_global.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "class Base:\n    pass\n\n\
class Container(Base):\n    Base = 1\n    value = 0\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Base")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Base")
        );
    }

    #[test]
    fn ignores_python_nested_class_metaclass_local_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("nested_class_metaclass_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "class GlobalMeta(type):\n    pass\n\n\
def top_level():\n    Meta = GlobalMeta\n\n    class Inner(metaclass=Meta):\n        pass\n\n    return 0\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "top_level.Inner", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "top_level.Inner", TraceDirection::Both)
                .unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn traces_python_class_metaclass_global_fallbacks_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_metaclass_global.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "class Meta(type):\n    pass\n\n\
class Container(metaclass=Meta):\n    Meta = 1\n    value = 0\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Meta")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Meta")
        );
    }

    #[test]
    fn ignores_python_class_lambda_local_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_lambda_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "class Container:\n    helper = 2\n    value = (lambda: helper)()\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_class_body_local_references_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_local_reference.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "class Container:\n    helper = 2\n    value = helper\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn traces_python_class_comprehension_global_fallbacks_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("class_comprehension_global.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
class Container:\n    helper = 2\n    value = [helper for item in range(1)]\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "Container", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "Container", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn ignores_python_comprehension_target_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("comprehension_shadow.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def item():\n    return 1\n\n\
def orchestrate(values):\n    return [item for item in values]\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn ignores_python_nonlocal_outer_variable_references_in_persisted_nested_traces() {
        let dir = temporary_dir();
        let source = dir.join("nonlocal_outer_variable.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    helper = 2\n\n    def inner():\n        nonlocal helper\n        return helper\n\n    return inner()\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
        assert!(live_trace.callees.is_empty());

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both)
                .unwrap();
        assert!(persisted_trace.callees.is_empty());
    }

    #[test]
    fn traces_python_nonlocal_outer_function_references_in_persisted_nested_traces() {
        let dir = temporary_dir();
        let source = dir.join("nonlocal_outer_function.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def helper():\n    return 1\n\n\
def top_level():\n    def helper():\n        return 2\n\n    def inner():\n        nonlocal helper\n        return helper()\n\n    return inner()\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "top_level.inner", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "top_level.helper")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "top_level.inner", TraceDirection::Both)
                .unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "top_level.helper")
        );
    }

    #[test]
    fn ignores_python_match_class_positional_capture_shadowing_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_class_positional_capture.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "class Point:\n    __match_args__ = (\"value\",)\n\n\
def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case Point(target):\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Point")
        );
        assert!(
            !live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "Point")
        );
        assert!(
            !persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );
    }

    #[test]
    fn ignores_python_match_guard_global_fallback_after_prior_capture_in_persisted_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_guard_reference.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case [target]:\n            return 0\n        case _ if target():\n            return 1\n        case _:\n            return 2\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            !live_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            !persisted_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "target")
        );
    }

    #[test]
    fn ignores_python_match_mixed_mapping_capture_shadowing_in_traces() {
        let dir = temporary_dir();
        let source = dir.join("match_mixed_mapping_capture.py");

        fs::write(
            &source,
            "def target():\n    return 1\n\n\
def orchestrate(value):\n    match value:\n        case {\"x\": x, **target}:\n            return target\n        case _:\n            return 0\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(trace.callees.is_empty());
    }

    #[test]
    fn traces_python_decorator_references() {
        let dir = temporary_dir();
        let source = dir.join("decorated.py");

        fs::write(
            &source,
            "def decorator(func):\n    return func\n\n\
@decorator\n\
def orchestrate(value: int) -> int:\n    return value\n",
        )
        .unwrap();

        let trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();

        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "decorator")
        );
    }

    #[test]
    fn traces_decorated_python_symbol_metadata_through_index() {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let caller = dir.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &helper,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from helper import helper\n\ndef decorator(func):\n    return func\n\n@decorator\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        let helper_source = fs::read_to_string(&helper).unwrap();
        let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
        let helper_start = helper_source.find(helper_text).unwrap();
        let helper_end = helper_start + helper_text.len();

        let caller_source = fs::read_to_string(&caller).unwrap();
        let orchestrate_text =
            "@decorator\ndef orchestrate(value: int) -> int:\n    return helper(value)";
        let orchestrate_start = caller_source.find(orchestrate_text).unwrap();
        let orchestrate_end = orchestrate_start + orchestrate_text.len();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(live_trace.symbol.origin_type, "trace_root");
        assert_eq!(
            live_trace.symbol.evidence_key,
            live_trace.evidence_keys.symbol
        );
        assert_eq!(
            live_trace.symbol.signature.as_deref(),
            Some("@decorator\ndef orchestrate(value: int) -> int:")
        );
        assert_eq!(
            live_trace.symbol.byte_range,
            (orchestrate_start, orchestrate_end)
        );
        let live_helper = live_trace
            .callees
            .iter()
            .find(|symbol| symbol.semantic_path == "helper")
            .unwrap();
        assert_eq!(
            live_helper.signature.as_deref(),
            Some("@decorator\ndef helper(value: int) -> int:")
        );
        assert_eq!(live_helper.byte_range, (helper_start, helper_end));

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(persisted_trace.symbol.origin_type, "trace_root");
        assert_eq!(
            persisted_trace.symbol.evidence_key,
            persisted_trace.evidence_keys.symbol
        );
        assert_eq!(
            persisted_trace.symbol.signature.as_deref(),
            Some("@decorator\ndef orchestrate(value: int) -> int:")
        );
        assert_eq!(
            persisted_trace.symbol.byte_range,
            (orchestrate_start, orchestrate_end)
        );
        let persisted_helper = persisted_trace
            .callees
            .iter()
            .find(|symbol| symbol.semantic_path == "helper")
            .unwrap();
        assert_eq!(
            persisted_helper.signature.as_deref(),
            Some("@decorator\ndef helper(value: int) -> int:")
        );
        assert_eq!(persisted_helper.byte_range, (helper_start, helper_end));
    }

    #[test]
    fn traces_python_alias_import_calls_across_files() {
        let dir = temporary_dir();
        let helper = dir.join("graph_b.py");
        let caller = dir.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\n\ndef orchestrate(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(live_trace.callees.len(), 1);
        assert_eq!(live_trace.callees[0].semantic_path, "helper");
        assert_eq!(
            live_trace.callees[0].file_path,
            helper.to_string_lossy().replace('\\', "/")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(persisted_trace.callees.len(), 1);
        assert_eq!(persisted_trace.callees[0].semantic_path, "helper");
        assert_eq!(
            persisted_trace.callees[0].file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn traces_python_absolute_package_alias_import_calls_across_files() {
        let dir = temporary_dir();
        let package = dir.join("pkg");
        let subpackage = package.join("sub");
        let helper = package.join("graph_c.py");
        let caller = subpackage.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::create_dir_all(&subpackage).unwrap();
        fs::write(package.join("__init__.py"), "").unwrap();
        fs::write(subpackage.join("__init__.py"), "").unwrap();
        fs::write(
            &helper,
            "def worker(value: int) -> int:\n    return value + 3\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "import pkg.graph_c as gc\nfrom pkg.graph_c import worker as w\n\n\ndef orchestrate(value: int) -> int:\n    return gc.worker(value) + w(value)\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(live_trace.callees.len(), 1);
        assert_eq!(live_trace.callees[0].semantic_path, "worker");
        assert_eq!(
            live_trace.callees[0].file_path,
            helper.to_string_lossy().replace('\\', "/")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(persisted_trace.callees.len(), 1);
        assert_eq!(persisted_trace.callees[0].semantic_path, "worker");
        assert_eq!(
            persisted_trace.callees[0].file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn traces_python_import_from_module_alias_calls_across_files() {
        let dir = temporary_dir();
        let package = dir.join("pkg");
        let subpackage = package.join("sub");
        let helper = package.join("graph_c.py");
        let local_helper = subpackage.join("local_mod.py");
        let caller = subpackage.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::create_dir_all(&subpackage).unwrap();
        fs::write(package.join("__init__.py"), "").unwrap();
        fs::write(subpackage.join("__init__.py"), "").unwrap();
        fs::write(
            &helper,
            "def worker(value: int) -> int:\n    return value + 3\n",
        )
        .unwrap();
        fs::write(
            &local_helper,
            "def helper2(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from pkg import graph_c as gc\nfrom . import local_mod as lm\n\n\ndef orchestrate(value: int) -> int:\n    return gc.worker(value) + lm.helper2(value)\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(live_trace.callees.len(), 2);
        assert!(live_trace.callees.iter().any(|symbol| {
            symbol.semantic_path == "worker"
                && symbol.file_path == helper.to_string_lossy().replace('\\', "/")
        }));
        assert!(live_trace.callees.iter().any(|symbol| {
            symbol.semantic_path == "helper2"
                && symbol.file_path == local_helper.to_string_lossy().replace('\\', "/")
        }));

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(persisted_trace.callees.len(), 2);
        assert!(persisted_trace.callees.iter().any(|symbol| {
            symbol.semantic_path == "worker"
                && symbol.file_path == helper.to_string_lossy().replace('\\', "/")
        }));
        assert!(persisted_trace.callees.iter().any(|symbol| {
            symbol.semantic_path == "helper2"
                && symbol.file_path == local_helper.to_string_lossy().replace('\\', "/")
        }));
    }

    #[test]
    fn traces_python_package_reexport_calls_across_files() {
        let dir = temporary_dir();
        let package = dir.join("pkg");
        let helper = package.join("graph_c.py");
        let caller = dir.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("__init__.py"),
            "from .graph_c import worker as worker\n",
        )
        .unwrap();
        fs::write(
            &helper,
            "def worker(value: int) -> int:\n    return value + 4\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from pkg import worker\n\n\ndef orchestrate(value: int) -> int:\n    return worker(value)\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(live_trace.callees.len(), 1);
        assert_eq!(live_trace.callees[0].semantic_path, "worker");
        assert_eq!(
            live_trace.callees[0].file_path,
            helper.to_string_lossy().replace('\\', "/")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(persisted_trace.callees.len(), 1);
        assert_eq!(persisted_trace.callees[0].semantic_path, "worker");
        assert_eq!(
            persisted_trace.callees[0].file_path,
            helper.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn rebuilds_and_reads_persisted_symbol_index() {
        let workspace_root = Path::new("../../tests/fixtures");
        let dir = temporary_dir();
        let db_path = dir.join("symbols.db");

        let stats = rebuild_symbol_index(workspace_root, &db_path).unwrap();
        assert_eq!(stats.indexed_files, 4);
        assert!(stats.indexed_symbols >= 3);
        assert_eq!(stats.reused_files, 0);

        let repeat_stats = rebuild_symbol_index(workspace_root, &db_path).unwrap();
        assert_eq!(repeat_stats.indexed_files, 4);
        assert_eq!(repeat_stats.rebuilt_files, 0);
        assert_eq!(repeat_stats.reused_files, 4);

        let trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(trace.symbol.parameters, vec!["value: int".to_string()]);
        assert_eq!(trace.symbol.return_type.as_deref(), Some("int"));
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.parameters == vec!["value: int".to_string()])
        );
    }

    #[test]
    fn rebuild_symbol_index_normalizes_workspace_and_db_paths() {
        let dir = temporary_dir();
        let workspace = dir.join("workspace");
        let nested = workspace.join("child");
        let helper = workspace.join("helper.py");
        let caller = workspace.join("caller.py");

        fs::create_dir_all(&nested).unwrap();
        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        let workspace_with_segments = nested.join("..");
        let db_path_with_segments = nested.join("..").join("symbols.db");
        let stats = rebuild_symbol_index(&workspace_with_segments, &db_path_with_segments).unwrap();

        assert_eq!(stats.indexed_files, 2);
        assert!(!stats.db_path.contains("/../"));

        let trace = trace_symbol_graph_from_index(
            &db_path_with_segments,
            "orchestrate",
            TraceDirection::Both,
        )
        .unwrap();
        assert_eq!(trace.callees.len(), 1);
        assert_eq!(trace.callees[0].semantic_path, "helper");
        assert!(!trace.symbol.file_path.contains("/../"));
    }

    #[test]
    fn rebuild_symbol_index_skips_cache_and_environment_dirs() {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let cache_dir = dir.join(".pytest_cache");
        let venv_dir = dir.join("venv");
        let db_path = dir.join("symbols.db");

        fs::create_dir_all(&cache_dir).unwrap();
        fs::create_dir_all(&venv_dir).unwrap();
        fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
        fs::write(
            cache_dir.join("cached.py"),
            "def cached() -> int:\n    return 2\n",
        )
        .unwrap();
        fs::write(
            venv_dir.join("installed.py"),
            "def installed() -> int:\n    return 3\n",
        )
        .unwrap();

        let stats = rebuild_symbol_index(&dir, &db_path).unwrap();

        assert_eq!(stats.indexed_files, 1);
        assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
        assert!(trace_symbol_graph_from_index(&db_path, "cached", TraceDirection::Both).is_err());
        assert!(
            trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err()
        );
    }

    #[test]
    fn refresh_symbol_index_ignores_files_in_skipped_dirs() {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let venv_dir = dir.join("venv");
        let installed = venv_dir.join("installed.py");
        let db_path = dir.join("symbols.db");

        fs::create_dir_all(&venv_dir).unwrap();
        fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
        fs::write(&installed, "def installed() -> int:\n    return 3\n").unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let stats = refresh_symbol_index_for_file(&dir, &db_path, &installed).unwrap();

        assert_eq!(stats.indexed_files, 1);
        assert_eq!(stats.rebuilt_files, 0);
        assert_eq!(stats.reused_files, 1);
        assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
        assert!(
            trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err()
        );
    }

    #[test]
    fn from_path_entrypoints_normalize_file_paths() {
        let dir = temporary_dir();
        let nested = dir.join("child");
        let python_file = dir.join("buffer.py");
        let c_file = dir.join("sample.c");

        fs::create_dir_all(&nested).unwrap();
        fs::write(&python_file, "def value() -> int:\n    return 1\n").unwrap();
        fs::write(
            &c_file,
            "int helper(int value) { return value + 1; }\nint orchestrate(int value) { return helper(value); }\n",
        )
        .unwrap();

        let python_alias = nested.join("..").join("buffer.py");
        let c_alias = nested.join("..").join("sample.c");

        let skeleton = get_semantic_skeleton_from_path(&python_alias, 1, &[]).unwrap();
        assert!(!skeleton.file.contains("/../"));

        let patch = patch_ast_node_from_path(
            &python_alias,
            "value",
            "def value() -> int:\n    return 2\n",
            None,
        )
        .unwrap();
        assert!(patch.applied);
        assert!(!patch.file.contains("/../"));
        assert!(
            fs::read_to_string(&python_file)
                .unwrap()
                .contains("return 2")
        );

        let captures = execute_tree_query_from_path(
            &c_alias,
            "(call_expression function: (identifier) @callee)",
        )
        .unwrap();
        let owner_symbol_id = captures[0].owner_symbol_id.as_deref().unwrap();
        assert!(!owner_symbol_id.contains("/../"));
    }

    #[test]
    fn trace_from_missing_symbol_index_does_not_create_database() {
        let dir = temporary_dir();
        let missing_db_path = dir.join("missing-symbols.db");

        let error =
            trace_symbol_graph_from_index(&missing_db_path, "orchestrate", TraceDirection::Both)
                .unwrap_err();

        assert!(error.to_string().contains("does not exist"));
        assert!(!missing_db_path.exists());
    }

    #[test]
    fn trace_from_index_rejects_negative_persisted_byte_ranges() {
        let dir = temporary_dir();
        let db_path = dir.join("symbols.db");
        let connection = Connection::open(&db_path).unwrap();

        create_minimal_symbol_index_schema(&connection);
        connection
            .execute_batch(
                "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', -1, 5,
                    '[]', '[]', '[]', '[]'
                );
                ",
            )
            .unwrap();

        let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("negative persisted byte ranges should be rejected");

        assert!(error.to_string().contains("non-negative integer"));
    }

    #[test]
    fn trace_from_index_rejects_invalid_persisted_json_columns() {
        let dir = temporary_dir();
        let db_path = dir.join("symbols.db");
        let connection = Connection::open(&db_path).unwrap();

        create_minimal_symbol_index_schema(&connection);
        connection
            .execute_batch(
                "
                INSERT INTO symbols (
                    symbol_id, semantic_path, file_path, node_kind, start_byte, end_byte,
                    parameters_json, dependencies_json, references_json, reference_names_json
                ) VALUES (
                    'helper', 'helper', 'helper.py', 'function_definition', 0, 5,
                    '[]', '{not-json', '[]', '[]'
                );
                ",
            )
            .unwrap();

        let error = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both)
            .expect_err("invalid persisted JSON columns should be rejected");

        assert!(error.to_string().contains("Conversion error"));
    }

    #[test]
    fn traces_python_symbol_metadata_through_persisted_index() {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let caller = dir.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    \"\"\"Shared helper.\"\"\"\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    \"\"\"Coordinate the helper call.\"\"\"\n    return helper(value)\n",
        )
        .unwrap();

        let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(
            live_trace.symbol.docstring.as_deref(),
            Some("\"\"\"Coordinate the helper call.\"\"\"")
        );
        assert_eq!(live_trace.symbol.parameters, vec!["value: int".to_string()]);
        assert_eq!(live_trace.symbol.return_type.as_deref(), Some("int"));
        assert_eq!(live_trace.callees.len(), 1);
        assert_eq!(
            live_trace.callees[0].docstring.as_deref(),
            Some("\"\"\"Shared helper.\"\"\"")
        );
        assert_eq!(
            live_trace.callees[0].parameters,
            vec!["value: int".to_string()]
        );
        assert_eq!(live_trace.callees[0].return_type.as_deref(), Some("int"));

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace.symbol.docstring.as_deref(),
            Some("\"\"\"Coordinate the helper call.\"\"\"")
        );
        assert_eq!(
            persisted_trace.symbol.parameters,
            vec!["value: int".to_string()]
        );
        assert_eq!(persisted_trace.symbol.return_type.as_deref(), Some("int"));
        assert_eq!(persisted_trace.callees.len(), 1);
        assert_eq!(
            persisted_trace.callees[0].docstring.as_deref(),
            Some("\"\"\"Shared helper.\"\"\"")
        );
        assert_eq!(
            persisted_trace.callees[0].parameters,
            vec!["value: int".to_string()]
        );
        assert_eq!(
            persisted_trace.callees[0].return_type.as_deref(),
            Some("int")
        );
    }

    #[test]
    fn refreshes_single_file_symbol_index() {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let caller = dir.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();
        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return branch(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();

        let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
        assert_eq!(stats.rebuilt_files, 1);
        assert_eq!(stats.reused_files, 1);

        let trace =
            trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).unwrap();
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "branch")
        );
        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "leaf")
        );
    }

    #[test]
    fn refreshes_c_include_dependents_for_header_change() {
        let dir = temporary_dir();
        let alpha_header = dir.join("alpha.h");
        let alpha_source = dir.join("alpha.c");
        let zeta_header = dir.join("zeta.h");
        let zeta_source = dir.join("zeta.c");
        let wrapper_header = dir.join("wrapper.h");
        let caller = dir.join("caller.c");
        let db_path = dir.join("symbols.db");

        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
        fs::write(
            &caller,
            "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let initial_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(initial_trace.callees.len(), 1);
        assert_eq!(
            initial_trace.callees[0].file_path,
            alpha_source.to_string_lossy().replace('\\', "/")
        );

        fs::write(&wrapper_header, "#include \"zeta.h\"\n").unwrap();

        let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
        assert_eq!(stats.indexed_files, 6);
        assert_eq!(stats.rebuilt_files, 2);
        assert_eq!(stats.reused_files, 4);

        let updated_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(updated_trace.callees.len(), 1);
        assert_eq!(
            updated_trace.callees[0].file_path,
            zeta_source.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn refreshes_c_include_dependents_for_parent_relative_header() {
        let dir = temporary_dir();
        let include_dir = dir.join("include");
        let source_dir = dir.join("src");
        let alpha_header = include_dir.join("alpha.h");
        let alpha_source = include_dir.join("alpha.c");
        let zeta_header = include_dir.join("zeta.h");
        let zeta_source = include_dir.join("zeta.c");
        let wrapper_header = include_dir.join("wrapper.h");
        let caller = source_dir.join("caller.c");
        let db_path = dir.join("symbols.db");

        fs::create_dir_all(&include_dir).unwrap();
        fs::create_dir_all(&source_dir).unwrap();
        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
        fs::write(
            &caller,
            "#include \"../include/wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let initial_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(initial_trace.callees.len(), 1);
        assert_eq!(
            initial_trace.callees[0].file_path,
            alpha_source.to_string_lossy().replace('\\', "/")
        );

        fs::write(&wrapper_header, "#include \"zeta.h\"\n").unwrap();

        let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
        assert_eq!(stats.indexed_files, 6);
        assert_eq!(stats.rebuilt_files, 2);

        let updated_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(updated_trace.callees.len(), 1);
        assert_eq!(
            updated_trace.callees[0].file_path,
            zeta_source.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn refreshes_c_include_dependents_for_deleted_header() {
        let dir = temporary_dir();
        let alpha_header = dir.join("alpha.h");
        let alpha_source = dir.join("alpha.c");
        let zeta_header = dir.join("zeta.h");
        let zeta_source = dir.join("zeta.c");
        let wrapper_header = dir.join("wrapper.h");
        let caller = dir.join("caller.c");
        let db_path = dir.join("symbols.db");

        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(&wrapper_header, "#include \"alpha.h\"\n").unwrap();
        fs::write(
            &caller,
            "#include \"wrapper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let initial_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(initial_trace.callees.len(), 1);
        assert_eq!(
            initial_trace.callees[0].file_path,
            alpha_source.to_string_lossy().replace('\\', "/")
        );

        fs::remove_file(&wrapper_header).unwrap();

        let stats = refresh_symbol_index_for_file(&dir, &db_path, &wrapper_header).unwrap();
        assert_eq!(stats.indexed_files, 5);
        assert_eq!(stats.rebuilt_files, 2);

        let updated_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert_eq!(updated_trace.callees.len(), 1);
        assert_eq!(
            updated_trace.callees[0].file_path,
            zeta_source.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn does_not_refresh_dependents_for_missing_system_include() {
        let dir = temporary_dir();
        let helper_header = dir.join("helper.h");
        let helper_source = dir.join("helper.c");
        let caller = dir.join("caller.c");
        let db_path = dir.join("symbols.db");

        fs::write(&helper_header, "int helper(int value);\n").unwrap();
        fs::write(
            &helper_source,
            "#include \"helper.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "#include <stdio.h>\n#include \"helper.h\"\n\nint orchestrate(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();

        let missing_system_header = dir.join("stdio.h");
        let stats = refresh_symbol_index_for_file(&dir, &db_path, &missing_system_header).unwrap();
        assert_eq!(stats.indexed_files, 3);
        assert_eq!(stats.rebuilt_files, 1);
        assert_eq!(stats.reused_files, 2);
    }

    #[test]
    fn traces_duplicate_c_globals_by_precise_symbol_id() {
        let dir = temporary_dir();
        let alpha_header = dir.join("alpha.h");
        let alpha_source = dir.join("alpha.c");
        let alpha_caller = dir.join("alpha_caller.c");
        let zeta_header = dir.join("zeta.h");
        let zeta_source = dir.join("zeta.c");
        let zeta_caller = dir.join("zeta_caller.c");
        let db_path = dir.join("symbols.db");

        fs::write(&alpha_header, "int helper(int value);\n").unwrap();
        fs::write(
            &alpha_source,
            "#include \"alpha.h\"\n\nint helper(int value) {\n    return value + 1;\n}\n",
        )
        .unwrap();
        fs::write(
            &alpha_caller,
            "#include \"alpha.h\"\n\nint call_alpha(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();
        fs::write(&zeta_header, "int helper(int value);\n").unwrap();
        fs::write(
            &zeta_source,
            "#include \"zeta.h\"\n\nint helper(int value) {\n    return value + 2;\n}\n",
        )
        .unwrap();
        fs::write(
            &zeta_caller,
            "#include \"zeta.h\"\n\nint call_zeta(int value) {\n    return helper(value);\n}\n",
        )
        .unwrap();

        let alpha_symbol_id = format!(
            "{}::helper",
            alpha_header.to_string_lossy().replace('\\', "/")
        );
        let zeta_symbol_id = format!(
            "{}::helper",
            zeta_header.to_string_lossy().replace('\\', "/")
        );

        let alpha_trace = trace_symbol_graph(&dir, &alpha_symbol_id, TraceDirection::Both).unwrap();
        assert_eq!(alpha_trace.symbol.symbol_id, alpha_symbol_id);
        assert_eq!(
            alpha_trace.symbol.file_path,
            alpha_source.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(alpha_trace.callers.len(), 1);
        assert_eq!(alpha_trace.callers[0].semantic_path, "call_alpha");
        assert_eq!(
            alpha_trace.callers[0].file_path,
            alpha_caller.to_string_lossy().replace('\\', "/")
        );

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted_zeta_trace =
            trace_symbol_graph_from_index(&db_path, &zeta_symbol_id, TraceDirection::Both).unwrap();
        assert_eq!(persisted_zeta_trace.symbol.symbol_id, zeta_symbol_id);
        assert_eq!(
            persisted_zeta_trace.symbol.file_path,
            zeta_source.to_string_lossy().replace('\\', "/")
        );
        assert_eq!(persisted_zeta_trace.callers.len(), 1);
        assert_eq!(persisted_zeta_trace.callers[0].semantic_path, "call_zeta");
        assert_eq!(
            persisted_zeta_trace.callers[0].file_path,
            zeta_caller.to_string_lossy().replace('\\', "/")
        );
    }

    #[test]
    fn refreshes_index_when_symbol_becomes_resolvable() {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let caller = dir.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &helper,
            "def assist(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();

        let initial_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(initial_trace.callees.is_empty());

        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
        assert_eq!(stats.rebuilt_files, 1);
        assert_eq!(stats.reused_files, 1);

        let updated_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            updated_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn refreshes_index_when_symbol_becomes_unresolvable() {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let caller = dir.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let initial_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            initial_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );

        fs::write(
            &helper,
            "def assist(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

        let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
        assert_eq!(stats.rebuilt_files, 1);
        assert_eq!(stats.reused_files, 1);

        let updated_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(updated_trace.callees.is_empty());
    }

    #[test]
    fn refreshes_index_when_symbol_file_is_deleted() {
        let dir = temporary_dir();
        let helper = dir.join("helper.py");
        let caller = dir.join("caller.py");
        let db_path = dir.join("symbols.db");

        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller,
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let initial_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            initial_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );

        fs::remove_file(&helper).unwrap();

        let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
        assert_eq!(stats.indexed_files, 1);
        assert_eq!(stats.rebuilt_files, 1);

        let updated_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(updated_trace.callees.is_empty());
        assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_err());
    }

    #[test]
    fn rejects_refresh_path_that_escapes_workspace_after_normalization() {
        let dir = temporary_dir();
        let workspace = dir.join("workspace");
        let nested = workspace.join("child");
        let helper = workspace.join("helper.py");
        let db_path = workspace.join("symbols.db");
        let outside = dir.join("outside.py");

        fs::create_dir_all(&nested).unwrap();
        fs::write(
            &helper,
            "def helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &outside,
            "def outside(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();

        rebuild_symbol_index(&workspace, &db_path).unwrap();

        let escaping_path = nested.join("..").join("..").join("outside.py");
        let error = refresh_symbol_index_for_file(&workspace, &db_path, &escaping_path)
            .expect_err("refresh should reject paths outside the workspace");
        assert!(error.to_string().contains("outside workspace"));
    }

    #[test]
    fn rejects_refresh_path_outside_workspace_before_missing_index_rebuild() {
        let dir = temporary_dir();
        let workspace = dir.join("workspace");
        let outside = dir.join("outside.py");
        let missing_db_path = workspace.join("missing-symbols.db");

        fs::create_dir_all(&workspace).unwrap();
        fs::write(
            workspace.join("helper.py"),
            "def helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &outside,
            "def outside(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();

        let error = refresh_symbol_index_for_file(&workspace, &missing_db_path, &outside)
            .expect_err("refresh should reject outside files before rebuilding a missing index");
        assert!(error.to_string().contains("outside workspace"));
        assert!(!missing_db_path.exists());
    }

    fn temporary_dir() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("arborist-mcp-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn create_minimal_symbol_index_schema(connection: &Connection) {
        connection
            .execute_batch(
                "
                CREATE TABLE metadata (
                    key TEXT PRIMARY KEY,
                    value TEXT NOT NULL
                );
                CREATE TABLE symbols (
                    symbol_id TEXT NOT NULL,
                    semantic_path TEXT NOT NULL,
                    scope_path TEXT,
                    file_path TEXT NOT NULL,
                    node_kind TEXT NOT NULL,
                    start_byte INTEGER NOT NULL,
                    end_byte INTEGER NOT NULL,
                    signature TEXT,
                    parameters_json TEXT NOT NULL DEFAULT '[]',
                    return_type TEXT,
                    docstring TEXT,
                    dependencies_json TEXT NOT NULL,
                    references_json TEXT NOT NULL,
                    reference_names_json TEXT NOT NULL DEFAULT '[]',
                    PRIMARY KEY (semantic_path, file_path)
                );
                CREATE TABLE file_state (
                    file_path TEXT PRIMARY KEY,
                    fingerprint INTEGER NOT NULL
                );
                INSERT INTO metadata(key, value) VALUES('indexed_files', '1');
                ",
            )
            .unwrap();
    }
}
