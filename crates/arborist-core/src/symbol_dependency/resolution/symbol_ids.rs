use std::path::Path;

use anyhow::Result;

use super::super::c::c_symbol_family_anchor;
use crate::language::{detect_language, is_c_header_path};
use crate::model::LanguageId;
use crate::semantic::cpp_callable_symbol_id;
use crate::symbol_index_model::IndexedSymbol;

pub(crate) fn assign_symbol_ids(raw_symbols: &mut [IndexedSymbol]) -> Result<()> {
    let symbol_ids = (0..raw_symbols.len())
        .map(|index| symbol_id_for_index(index, raw_symbols))
        .collect::<Result<Vec<_>>>()?;

    for (symbol, symbol_id) in raw_symbols.iter_mut().zip(symbol_ids) {
        symbol.symbol_id = symbol_id;
    }

    Ok(())
}

fn symbol_id_for_index(index: usize, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let symbol = &raw_symbols[index];
    let path = Path::new(&symbol.file_path);
    if detect_language(path).ok() == Some(LanguageId::Cpp)
        && matches!(
            symbol.node_kind.as_str(),
            "function_definition" | "declaration" | "field_declaration"
        )
    {
        return Ok(cpp_callable_symbol_id(
            &symbol.semantic_path,
            &symbol.parameters,
            symbol.signature.as_deref(),
        ));
    }
    if !matches!(
        detect_language(path).ok(),
        Some(LanguageId::C | LanguageId::Cpp)
    ) || symbol.semantic_path.contains("::")
    {
        return Ok(symbol.semantic_path.clone());
    }

    let anchor = if is_c_header_path(path) {
        symbol.file_path.clone()
    } else {
        c_symbol_family_anchor(symbol, raw_symbols)?
    };

    Ok(format!("{anchor}::{}", symbol.base_name))
}
