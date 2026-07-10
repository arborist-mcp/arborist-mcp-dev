use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};

use crate::language::{point_for_offset, position_from, read_source};
use crate::model::{SymbolMeta, SymbolReadResult};
use crate::symbol_summary::symbol_summary_from_meta;

pub(crate) fn read_symbol_result_from_meta(
    symbol: &SymbolMeta,
    indexed_files: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    let source = symbol_source_text(symbol, file_overrides)?;
    let snippet = symbol_source_slice(symbol, &source)?.to_string();
    let start_point = position_from(point_for_offset(&source, symbol.byte_range.0)?);
    let end_point = position_from(point_for_offset(&source, symbol.byte_range.1)?);

    let result = SymbolReadResult {
        indexed_files,
        symbol: symbol_summary_from_meta(symbol),
        source: snippet,
        start_point,
        end_point,
    };
    result.validate_public_output()?;
    Ok(result)
}

fn symbol_source_text(
    symbol: &SymbolMeta,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<String> {
    if let Some(file_overrides) = file_overrides
        && let Some(source) = file_overrides.get(&symbol.file_path)
    {
        return Ok(source.clone());
    }

    read_source(Path::new(&symbol.file_path))
}

fn symbol_source_slice<'a>(symbol: &SymbolMeta, source: &'a str) -> Result<&'a str> {
    if symbol.byte_range.0 > symbol.byte_range.1 {
        return Err(anyhow!(
            "invalid symbol byte range for {}: start byte is after end byte",
            symbol.symbol_id
        ));
    }

    source
        .get(symbol.byte_range.0..symbol.byte_range.1)
        .ok_or_else(|| anyhow!("symbol source range is invalid for {}", symbol.symbol_id))
}
