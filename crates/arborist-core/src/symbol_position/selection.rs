use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::model::SymbolMeta;
use crate::symbol_index_model::symbol_kind_rank;
use anyhow::Result;

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

pub(super) fn choose_symbol_at_location_with_deadline<'a>(
    resolved_symbols: &'a [SymbolMeta],
    file_path: &str,
    symbol_id: &str,
    semantic_path: &str,
    byte_range: (usize, usize),
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<&'a SymbolMeta>> {
    let exact = choose_best_symbol(
        resolved_symbols,
        |symbol| {
            symbol.file_path == file_path
                && symbol.byte_range == byte_range
                && (symbol.symbol_id == symbol_id || symbol.semantic_path == semantic_path)
        },
        deadline,
    )?;
    if exact.is_some() {
        return Ok(exact);
    }
    choose_best_symbol(
        resolved_symbols,
        |symbol| {
            symbol.file_path == file_path
                && (symbol.symbol_id == symbol_id || symbol.semantic_path == semantic_path)
        },
        deadline,
    )
}

fn choose_best_symbol<'a, F>(
    symbols: &'a [SymbolMeta],
    mut predicate: F,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<&'a SymbolMeta>>
where
    F: FnMut(&SymbolMeta) -> bool,
{
    let mut best: Option<&'a SymbolMeta> = None;
    for symbol in symbols {
        if let Some(deadline) = deadline {
            deadline.check("resolving symbol at position")?;
        }
        if !predicate(symbol) {
            continue;
        }
        let replace = best.is_none_or(|current| {
            symbol_kind_rank(&symbol.node_kind)
                .cmp(&symbol_kind_rank(&current.node_kind))
                .then_with(|| current.file_path.cmp(&symbol.file_path))
                .then_with(|| current.byte_range.cmp(&symbol.byte_range))
                .then_with(|| current.symbol_id.cmp(&symbol.symbol_id))
                .is_gt()
        });
        if replace {
            best = Some(symbol);
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::choose_symbol_at_location_with_deadline;
    use crate::model::{SymbolMeta, SymbolMetaInit};
    use crate::symbol_trace::TraceQueryDeadline;

    #[test]
    fn location_selection_checks_deadlines_while_scanning_symbols() {
        let symbol = SymbolMeta::new(SymbolMetaInit {
            symbol_id: "sample.py::helper".to_string(),
            semantic_path: "helper".to_string(),
            scope_path: None,
            file_path: "sample.py".to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            byte_range: (0, 6),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            dependencies: Vec::new(),
            references: Vec::new(),
        });
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = choose_symbol_at_location_with_deadline(
            &[symbol],
            "sample.py",
            "sample.py::helper",
            "helper",
            (0, 6),
            Some(&deadline),
        )
        .expect_err("location selection should honor an expired deadline");

        assert!(error.to_string().contains("resolving symbol at position"));
    }
}
