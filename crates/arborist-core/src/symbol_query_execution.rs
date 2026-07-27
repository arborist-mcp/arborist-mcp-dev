use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

use crate::model::{SymbolMeta, SymbolReadResult};
use crate::symbol_index_model::symbol_kind_rank;
use crate::symbol_read::read_symbol_result_from_meta;

mod list;
mod read;
mod search;
mod trace;

pub(crate) use list::{
    list_context_from_symbols, list_context_from_symbols_with_timeout,
    list_discovery_context_from_symbols, list_discovery_context_from_symbols_with_timeout,
    list_from_symbols, list_from_symbols_with_timeout, list_neighborhood_context_from_symbols,
    list_neighborhood_context_from_symbols_with_timeout,
};
pub(crate) use read::{
    read_symbol_at_position_from_symbols, read_symbol_context_at_position_from_symbols,
    read_symbol_context_from_symbols, read_symbol_discovery_context_at_position_from_symbols,
    read_symbol_discovery_context_from_symbols, read_symbol_from_symbols,
    read_symbol_neighborhood_context_at_position_from_symbols,
    read_symbol_neighborhood_context_from_symbols,
};
pub(crate) use search::{
    search_context_from_symbols, search_context_from_symbols_with_timeout,
    search_discovery_context_from_symbols, search_discovery_context_from_symbols_with_timeout,
    search_from_symbols, search_from_symbols_with_timeout,
    search_neighborhood_context_from_symbols,
    search_neighborhood_context_from_symbols_with_timeout,
};
pub(crate) use trace::{
    trace_from_symbols_with_timeout, trace_neighborhood_from_symbols_with_timeout,
    trace_symbol_graph_at_position_from_symbols_with_timeout,
    trace_symbol_neighborhood_at_position_from_symbols_with_timeout,
};

pub(crate) fn read_symbol_from_meta(
    symbol: &SymbolMeta,
    indexed_files: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    read_symbol_result_from_meta(symbol, indexed_files, file_overrides)
}

fn validate_trace_symbol_path(symbol_path: &str) -> Result<()> {
    if symbol_path.trim().is_empty() {
        return Err(anyhow!("invalid symbol_path: selector must not be blank"));
    }

    Ok(())
}

fn choose_trace_symbol<'a>(symbols: &'a [SymbolMeta], symbol_path: &str) -> Option<&'a SymbolMeta> {
    symbols
        .iter()
        .filter(|symbol| symbol.symbol_id == symbol_path || symbol.semantic_path == symbol_path)
        .max_by(|left, right| {
            symbol_kind_rank(&left.node_kind)
                .cmp(&symbol_kind_rank(&right.node_kind))
                .then_with(|| right.file_path.cmp(&left.file_path))
                .then_with(|| right.byte_range.cmp(&left.byte_range))
                .then_with(|| right.symbol_id.cmp(&left.symbol_id))
        })
}

#[cfg(test)]
mod tests {
    use super::choose_trace_symbol;
    use crate::model::{SymbolMeta, SymbolMetaInit};

    fn symbol(symbol_id: &str, file_path: &str, byte_range: (usize, usize)) -> SymbolMeta {
        SymbolMeta::new(SymbolMetaInit {
            symbol_id: symbol_id.to_string(),
            semantic_path: "overloaded".to_string(),
            scope_path: None,
            file_path: file_path.to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            byte_range,
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            dependencies: Vec::new(),
            references: Vec::new(),
        })
    }

    #[test]
    fn choose_trace_symbol_is_stable_for_equal_rank_candidates() {
        let symbols = vec![
            symbol("z", "z.py", (20, 21)),
            symbol("a", "a.py", (40, 41)),
            symbol("b", "a.py", (10, 11)),
        ];

        let selected = choose_trace_symbol(&symbols, "overloaded")
            .expect("semantic path should select a candidate");
        assert_eq!(selected.symbol_id, "b");
    }
}
