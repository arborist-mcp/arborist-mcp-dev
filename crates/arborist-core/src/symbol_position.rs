use std::collections::BTreeMap;
use std::path::Path;

use crate::language::{normalize_path, offset_for_position, parse_document, read_source};
use crate::model::{LanguageId, Position, SymbolMeta};
use crate::semantic::{
    ascend_to_symbol, c_semantic_path, c_symbol_id_for_node, python_display_byte_range,
    semantic_path,
};
use anyhow::{Result, anyhow};

mod selection;

pub(crate) fn resolve_symbol_at_position<'a>(
    resolved_symbols: &'a [SymbolMeta],
    file_path: &Path,
    position: &Position,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<&'a SymbolMeta> {
    let normalized_file_path = normalize_path(file_path);
    let source = match file_overrides.and_then(|overrides| overrides.get(&normalized_file_path)) {
        Some(source) => source.clone(),
        None => read_source(file_path)?,
    };
    let document = parse_document(file_path, &source)?;
    let byte_offset = offset_for_position(&source, position)?;
    let node = selection::node_at_byte_offset(document.tree.root_node(), &source, byte_offset)
        .ok_or_else(|| {
            anyhow!(
                "position {}:{} does not resolve to a syntax node in {}",
                position.row,
                position.column,
                file_path.display()
            )
        })?;
    let symbol_node = ascend_to_symbol(document.language_id, node).ok_or_else(|| {
        anyhow!(
            "position {}:{} does not resolve to a semantic symbol in {}",
            position.row,
            position.column,
            file_path.display()
        )
    })?;

    let (symbol_id, semantic_path, byte_range) = match document.language_id {
        LanguageId::Python => {
            let semantic_path = semantic_path(symbol_node, &source)?;
            let byte_range = python_display_byte_range(symbol_node);
            (semantic_path.clone(), semantic_path, byte_range)
        }
        LanguageId::C | LanguageId::Cpp => {
            let semantic_path = c_semantic_path(file_path, symbol_node, &source)?
                .ok_or_else(|| anyhow!("position does not resolve to a C semantic symbol"))?;
            let symbol_id = c_symbol_id_for_node(file_path, symbol_node, &source)?
                .ok_or_else(|| anyhow!("position does not resolve to a C symbol id"))?;
            (
                symbol_id,
                semantic_path,
                (symbol_node.start_byte(), symbol_node.end_byte()),
            )
        }
    };

    selection::choose_symbol_at_location(
        resolved_symbols,
        &normalized_file_path,
        &symbol_id,
        &semantic_path,
        byte_range,
    )
    .ok_or_else(|| {
        anyhow!(
            "symbol at {}:{} not found in workspace index: {}",
            position.row,
            position.column,
            normalized_file_path
        )
    })
}
