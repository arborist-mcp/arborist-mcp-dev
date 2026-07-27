use std::collections::BTreeMap;

use crate::model::SymbolMeta;
use crate::symbol_index_model::symbol_kind_rank;

pub(crate) fn resolved_symbol_ref_map<'a>(
    symbols: &'a [SymbolMeta],
) -> BTreeMap<&'a str, &'a SymbolMeta> {
    let mut map: BTreeMap<&'a str, &'a SymbolMeta> = BTreeMap::new();
    for symbol in symbols {
        match map.get_mut(symbol.symbol_id.as_str()) {
            Some(existing) => {
                if symbol_kind_rank(&symbol.node_kind) > symbol_kind_rank(&existing.node_kind) {
                    *existing = symbol;
                }
            }
            None => {
                map.insert(symbol.symbol_id.as_str(), symbol);
            }
        }
    }
    map
}
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

#[cfg(test)]
mod tests {
    use super::{resolved_symbol_map, resolved_symbol_ref_map};
    use crate::model::{SymbolMeta, SymbolMetaInit};

    fn symbol(symbol_id: &str, node_kind: &str, file_path: &str) -> SymbolMeta {
        SymbolMeta::new(SymbolMetaInit {
            symbol_id: symbol_id.to_string(),
            semantic_path: symbol_id.to_string(),
            scope_path: None,
            file_path: file_path.to_string(),
            node_kind: node_kind.to_string(),
            origin_type: "workspace_symbol".to_string(),
            byte_range: (0, 1),
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            dependencies: Vec::new(),
            references: Vec::new(),
        })
    }

    #[test]
    fn borrowed_map_matches_owned_resolution_for_duplicate_ids() {
        let symbols = vec![
            symbol("item", "declaration", "item.h"),
            symbol("item", "function_definition", "item.c"),
            symbol("other", "class_definition", "other.py"),
        ];

        let owned = resolved_symbol_map(&symbols);
        let borrowed = resolved_symbol_ref_map(&symbols);

        assert_eq!(borrowed.len(), owned.len());
        for (symbol_id, symbol) in &owned {
            let borrowed_symbol = borrowed
                .get(symbol_id.as_str())
                .expect("borrowed map should contain every resolved symbol");
            assert_eq!(*borrowed_symbol, symbol);
        }
    }
}
