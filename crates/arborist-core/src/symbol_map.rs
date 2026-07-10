use std::collections::BTreeMap;

use crate::model::SymbolMeta;
use crate::symbol_index_model::symbol_kind_rank;

pub(crate) fn resolved_symbol_map(symbols: &[SymbolMeta]) -> BTreeMap<String, SymbolMeta> {
    let mut map: BTreeMap<String, SymbolMeta> = BTreeMap::new();
    for symbol in symbols {
        map.entry(symbol.symbol_id.clone())
            .and_modify(|existing| {
                if symbol_kind_rank(&symbol.node_kind) > symbol_kind_rank(&existing.node_kind) {
                    *existing = symbol.clone();
                }
            })
            .or_insert_with(|| symbol.clone());
    }
    map
}
