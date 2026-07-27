use std::path::Path;

use anyhow::Result;

use super::super::c::c_symbol_family_anchor;
use crate::language::{detect_language, is_c_header_path};
use crate::model::LanguageId;
use crate::semantic::cpp_callable_symbol_id;
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn assign_symbol_ids(raw_symbols: &mut [IndexedSymbol]) -> Result<()> {
    assign_symbol_ids_with_deadline(raw_symbols, None)
}

pub(crate) fn assign_symbol_ids_with_deadline(
    raw_symbols: &mut [IndexedSymbol],
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    let mut symbol_ids = Vec::with_capacity(raw_symbols.len());
    for index in 0..raw_symbols.len() {
        if let Some(deadline) = deadline {
            deadline.check("assigning symbol identities")?;
        }
        symbol_ids.push(symbol_id_for_index(index, raw_symbols)?);
    }

    for (symbol, symbol_id) in raw_symbols.iter_mut().zip(symbol_ids) {
        if let Some(deadline) = deadline {
            deadline.check("assigning symbol identities")?;
        }
        symbol.symbol_id = symbol_id;
    }

    if let Some(deadline) = deadline {
        deadline.check("assigning symbol identities")?;
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
