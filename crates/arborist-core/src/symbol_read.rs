use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Result, anyhow};

use crate::language::{point_for_offset, position_from, read_source, validate_source_length};
use crate::model::{SymbolMeta, SymbolReadResult};
use crate::symbol_summary::symbol_summary_from_meta;

pub(crate) fn read_symbol_result_from_meta(
    symbol: &SymbolMeta,
    indexed_files: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    let source = symbol_source_text(symbol, file_overrides)?;
    read_symbol_result_from_source(symbol, indexed_files, &source)
}

pub(crate) fn read_symbol_result_from_meta_with_cache(
    symbol: &SymbolMeta,
    indexed_files: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    source_cache: &mut BTreeMap<String, String>,
) -> Result<SymbolReadResult> {
    let source = symbol_source_text_with_cache(symbol, file_overrides, source_cache)?;
    read_symbol_result_from_source(symbol, indexed_files, source)
}

fn read_symbol_result_from_source(
    symbol: &SymbolMeta,
    indexed_files: usize,
    source: &str,
) -> Result<SymbolReadResult> {
    let snippet = symbol_source_slice(symbol, source)?.to_string();
    let start_point = position_from(point_for_offset(source, symbol.byte_range.0)?);
    let end_point = position_from(point_for_offset(source, symbol.byte_range.1)?);

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

    let source = read_source(Path::new(&symbol.file_path))?;
    validate_source_length(Path::new(&symbol.file_path), source.len())?;
    Ok(source)
}

fn symbol_source_text_with_cache<'a>(
    symbol: &SymbolMeta,
    file_overrides: Option<&BTreeMap<String, String>>,
    source_cache: &'a mut BTreeMap<String, String>,
) -> Result<&'a str> {
    if !source_cache.contains_key(&symbol.file_path) {
        let source = if let Some(file_overrides) = file_overrides
            && let Some(source) = file_overrides.get(&symbol.file_path)
        {
            source.clone()
        } else {
            let source = read_source(Path::new(&symbol.file_path))?;
            validate_source_length(Path::new(&symbol.file_path), source.len())?;
            source
        };
        source_cache.insert(symbol.file_path.clone(), source);
    }

    Ok(source_cache
        .get(&symbol.file_path)
        .expect("source cache should contain requested file")
        .as_str())
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::read_symbol_result_from_meta_with_cache;
    use crate::model::{SymbolMeta, SymbolMetaInit};

    fn symbol(symbol_id: &str, byte_range: (usize, usize)) -> SymbolMeta {
        SymbolMeta::new(SymbolMetaInit {
            symbol_id: symbol_id.to_string(),
            semantic_path: symbol_id.to_string(),
            scope_path: None,
            file_path: "virtual.py".to_string(),
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
    fn reuses_cached_source_for_symbols_from_the_same_file() {
        let mut overrides =
            BTreeMap::from([("virtual.py".to_string(), "alpha\nbeta\n".to_string())]);
        let mut source_cache = BTreeMap::new();

        let first = read_symbol_result_from_meta_with_cache(
            &symbol("alpha", (0, 5)),
            1,
            Some(&overrides),
            &mut source_cache,
        )
        .expect("first symbol should read from the override");
        overrides.insert("virtual.py".to_string(), "xxxxx\nyyyy\n".to_string());
        let second = read_symbol_result_from_meta_with_cache(
            &symbol("beta", (6, 10)),
            1,
            Some(&overrides),
            &mut source_cache,
        )
        .expect("second symbol should reuse the cached override");

        assert_eq!(first.source, "alpha");
        assert_eq!(second.source, "beta");
        assert_eq!(source_cache.len(), 1);
    }
}
