use std::collections::BTreeMap;

use crate::symbol_index_model::IndexedSymbol;

pub(crate) fn build_name_index(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut name_index = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        name_index
            .entry(symbol.base_name.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    name_index
}

pub(crate) fn build_semantic_path_index(
    raw_symbols: &[IndexedSymbol],
) -> BTreeMap<String, Vec<usize>> {
    let mut semantic_path_index = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        semantic_path_index
            .entry(symbol.semantic_path.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    semantic_path_index
}

pub(crate) fn raw_symbol_indexes_by_id(
    raw_symbols: &[IndexedSymbol],
) -> BTreeMap<String, Vec<usize>> {
    let mut indexes = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        indexes
            .entry(symbol.symbol_id.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    indexes
}
