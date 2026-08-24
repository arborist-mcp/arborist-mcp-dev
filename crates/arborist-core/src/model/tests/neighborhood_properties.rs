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

/// A trace-root SymbolMeta whose evidence key matches the derived identity.
fn trace_root_meta_strategy() -> impl Strategy<Value = SymbolMeta> {
    (nonblank_strategy(), nonblank_strategy(), 1usize..32usize).prop_map(
        |(symbol_id, file_path, end_byte)| {
            let node_kind = "function_definition".to_string();
            let origin_type = "trace_root".to_string();
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
                byte_range: (0, end_byte),
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

/// A workspace summary sharing the aligned fields of the given meta but with
/// a workspace origin and its own derived evidence key.
fn workspace_summary_from(meta: &SymbolMeta) -> SymbolSummary {
    let node_kind = meta.node_kind.clone();
    let origin_type = "workspace_symbol";
    SymbolSummary {
        symbol_id: meta.symbol_id.clone(),
        semantic_path: meta.semantic_path.clone(),
        scope_path: None,
        file_path: meta.file_path.clone(),
        node_kind,
        origin_type: origin_type.to_string(),
        evidence_key: format!(
            "{}|{}|{}|{}|{}..{}|",
            meta.symbol_id,
            meta.file_path,
            meta.node_kind,
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

/// A neighborhood context whose reads mirror every node's symbol fields.
fn aligned_context_strategy() -> impl Strategy<Value = SymbolNeighborhoodContextResult> {
    (
        trace_root_meta_strategy(),
        prop::collection::vec(trace_root_meta_strategy(), 1..=4),
        1usize..8usize,
    )
        .prop_map(|(root, node_metas, indexed_files)| {
            // The neighborhood checker requires nodes[0] to mirror the trace
            // root at depth 0; later nodes are arbitrary neighbors. Position
            // suffixes keep their ids and evidence keys unique.
            let mut nodes: Vec<_> = node_metas
                .iter()
                .enumerate()
                .map(|(position, meta)| {
                    let mut summary = workspace_summary_from(meta);
                    summary.symbol_id = format!("{}::n{position}", summary.symbol_id);
                    summary.semantic_path = format!("{}::n{position}", summary.semantic_path);
                    summary.evidence_key = format!(
                        "{}|{}|{}|{}|{}..{}|",
                        summary.symbol_id,
                        summary.file_path,
                        summary.node_kind,
                        summary.origin_type,
                        summary.byte_range.0,
                        summary.byte_range.1
                    );
                    TraceSymbolNeighborhoodNode {
                        symbol: summary,
                        depth: 1,
                    }
                })
                .collect();
            nodes.insert(
                0,
                TraceSymbolNeighborhoodNode {
                    symbol: workspace_summary_from(&root),
                    depth: 0,
                },
            );
            let reads: Vec<_> = nodes
                .iter()
                .map(|node| SymbolReadResult {
                    indexed_files,
                    symbol: node.symbol.clone(),
                    source: "def value():\n    return 1\n".to_string(),
                    start_point: Position { row: 0, column: 0 },
                    end_point: Position { row: 1, column: 14 },
                })
                .collect();
            SymbolNeighborhoodContextResult {
                neighborhood: TraceSymbolNeighborhoodResult {
                    symbol: root.clone(),
                    direction: TraceDirection::Callers,
                    max_depth: 2,
                    max_nodes: 8,
                    truncated: false,
                    indexed_files,
                    nodes,
                    edges: Vec::new(),
                },
                reads,
            }
        })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// A neighborhood context whose reads mirror the nodes validates.
    #[test]
    fn aligned_neighborhood_contexts_validate(
        mut context in aligned_context_strategy(),
    ) {
        prop_assert!(context.validate_public_output().is_ok());

        // Removing one read breaks length alignment and must be rejected.
        context.reads.pop();
        prop_assert!(context.validate_public_output().is_err());
    }

    /// Any single-field misalignment between a read and its node must be
    /// rejected.
    #[test]
    fn neighborhood_contexts_reject_field_misalignment(
        mut context in aligned_context_strategy(),
        which in 0usize..5,
    ) {
        match which {
            0 => context.reads[0].indexed_files += 1,
            1 => context.reads[0].symbol.symbol_id = "other".to_string(),
            2 => context.reads[0].symbol.semantic_path = "other".to_string(),
            3 => context.reads[0].symbol.file_path = "other".to_string(),
            // Note: node_kind/byte_range/signature follow the same direct
            // comparison path as these four, covered by the shared loop.
            _ => context.reads[0].symbol.node_kind = "other_kind".to_string(),
        }
        prop_assert!(context.validate_public_output().is_err());
    }
}
