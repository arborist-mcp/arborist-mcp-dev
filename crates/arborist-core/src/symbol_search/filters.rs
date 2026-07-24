use anyhow::{Result, anyhow};

use crate::model::SymbolMeta;

pub(crate) fn normalize_optional_search_filter(
    value: Option<&str>,
    field: &str,
) -> Result<Option<String>> {
    match value {
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return Err(anyhow!("{field} must not be blank"));
            }
            Ok(Some(trimmed.to_ascii_lowercase()))
        }
        None => Ok(None),
    }
}

pub(crate) fn symbol_matches_search_filters(
    symbol: &SymbolMeta,
    file_path_contains: Option<&str>,
    node_kind: Option<&str>,
) -> bool {
    if let Some(file_path_contains) = file_path_contains
        && !symbol
            .file_path
            .to_ascii_lowercase()
            .contains(file_path_contains)
    {
        return false;
    }
    if let Some(node_kind) = node_kind
        && symbol.node_kind.to_ascii_lowercase() != node_kind
    {
        return false;
    }
    true
}
