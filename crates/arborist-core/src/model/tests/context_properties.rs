use std::sync::LazyLock;

use super::*;
use proptest::prelude::*;

/// Characters valid in identifier-like non-blank strings.
static IDENTIFIER_CHARACTERS: LazyLock<Vec<char>> =
    LazyLock::new(|| vec!['a', 'z', '0', '9', '_', 'A', 'Z']);

fn nonblank_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*IDENTIFIER_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

/// A trace-root SymbolMeta whose evidence key matches the derived identity so
/// it passes validate_trace_replay_input.
fn trace_root_meta_strategy() -> impl Strategy<Value = SymbolMeta> {
    (nonblank_strategy(), nonblank_strategy(), 1usize..32usize).prop_map(
        |(symbol_id, file_path, end_byte)| {
            let node_kind = "function_definition".to_string();
            let origin_type = "trace_root".to_string();
            let byte_range = (0, end_byte);
            let evidence_key =
                format!("{symbol_id}|{file_path}|{node_kind}|{origin_type}|0..{end_byte}|");
            let semantic_path = format!("path::{symbol_id}");
            SymbolMeta {
                symbol_id,
                semantic_path,
                scope_path: None,
                file_path,
                node_kind,
                origin_type,
                evidence_key,
                byte_range,
                signature: None,
                parameters: Vec::new(),
                return_type: None,
                docstring: None,
                dependencies: Vec::new(),
                references: Vec::new(),
            }
        },
    )
}

/// A workspace summary sharing the aligned fields of the given meta but with a
/// workspace origin and its own derived evidence key.
fn workspace_summary_from(meta: &SymbolMeta) -> SymbolSummary {
    let node_kind = "function_definition".to_string();
    let origin_type = "workspace_symbol";
    SymbolSummary {
        symbol_id: meta.symbol_id.clone(),
        semantic_path: meta.semantic_path.clone(),
        scope_path: None,
        file_path: meta.file_path.clone(),
        node_kind: node_kind.clone(),
        origin_type: origin_type.to_string(),
        evidence_key: format!(
            "{}|{}|{}|{}|{}..{}|",
            meta.symbol_id,
            meta.file_path,
            node_kind,
            origin_type,
            meta.byte_range.0,
            meta.byte_range.1
        ),
        byte_range: meta.byte_range,
        signature: None,
        parameters: Vec::new(),
        return_type: None,
        docstring: None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A read/trace pair sharing every aligned field validates.
    #[test]
    fn aligned_contexts_validate(
        meta in trace_root_meta_strategy(),
        indexed_files in 1usize..8,
        source in nonblank_strategy(),
    ) {
        let context = build_context(&meta, indexed_files, &source);
        prop_assert!(context.validate_public_output().is_ok());
    }

    /// Any single-field misalignment must be rejected.
    #[test]
    fn contexts_reject_field_misalignment(
        meta in trace_root_meta_strategy(),
        indexed_files in 1usize..8,
        source in nonblank_strategy(),
        which in 0usize..7,
    ) {
        let mut broken = build_context(&meta, indexed_files, &source);

        match which {
            0 => broken.read.indexed_files += 1,
            1 => broken.read.symbol.symbol_id = "other".to_string(),
            2 => broken.read.symbol.semantic_path = "other".to_string(),
            3 => broken.read.symbol.file_path = "other".to_string(),
            4 => broken.read.symbol.node_kind = "other".to_string(),
            5 => {
                let range = broken.read.symbol.byte_range;
                broken.read.symbol.byte_range = (range.0 + 100, range.1 + 100);
            }
            // Note: signature misalignment alone is not detectable here
            // because both sides keep their derived evidence keys; the
            // checker compares signatures directly, so it IS detected.
            _ => {
                broken.read.symbol.signature = Some("sig".to_string());
            }
        }
        prop_assert!(broken.validate_public_output().is_err());
    }
}

fn build_context(meta: &SymbolMeta, indexed_files: usize, source: &str) -> SymbolContextResult {
    let trace = TraceSymbolGraphResult {
        symbol: meta.clone(),
        callers: Vec::new(),
        callees: Vec::new(),
        evidence_keys: TraceEvidenceKeys {
            symbol: meta.evidence_key.clone(),
            callers: Vec::new(),
            callees: Vec::new(),
        },
        indexed_files,
    };
    SymbolContextResult {
        read: SymbolReadResult {
            indexed_files,
            symbol: workspace_summary_from(meta),
            source: source.to_string(),
            start_point: Position { row: 0, column: 0 },
            end_point: Position { row: 0, column: 4 },
        },
        trace,
    }
}
