use tree_sitter::Node;

use crate::model::SymbolMeta;
use crate::symbol_index_model::symbol_kind_rank;

pub(super) fn node_at_byte_offset<'tree>(
    root: Node<'tree>,
    source: &str,
    byte_offset: usize,
) -> Option<Node<'tree>> {
    let (start, end) = if source.is_empty() {
        (0, 0)
    } else if byte_offset < source.len() {
        (byte_offset, byte_offset + 1)
    } else {
        (byte_offset.saturating_sub(1), byte_offset)
    };

    root.named_descendant_for_byte_range(start, end)
        .or_else(|| root.descendant_for_byte_range(start, end))
        .or_else(|| root.named_descendant_for_byte_range(start, start))
        .or_else(|| root.descendant_for_byte_range(start, start))
}

pub(super) fn choose_symbol_at_location<'a>(
    resolved_symbols: &'a [SymbolMeta],
    file_path: &str,
    symbol_id: &str,
    semantic_path: &str,
    byte_range: (usize, usize),
) -> Option<&'a SymbolMeta> {
    resolved_symbols
        .iter()
        .filter(|symbol| {
            symbol.file_path == file_path
                && symbol.byte_range == byte_range
                && (symbol.symbol_id == symbol_id || symbol.semantic_path == semantic_path)
        })
        .max_by_key(|symbol| symbol_kind_rank(&symbol.node_kind))
        .or_else(|| {
            resolved_symbols
                .iter()
                .filter(|symbol| {
                    symbol.file_path == file_path
                        && (symbol.symbol_id == symbol_id || symbol.semantic_path == semantic_path)
                })
                .max_by_key(|symbol| symbol_kind_rank(&symbol.node_kind))
        })
}
