use crate::model::{SymbolMeta, SymbolSearchMatchDetail};
use crate::symbol_index_model::symbol_base_name;

mod filters;

pub(crate) use filters::{normalize_optional_search_filter, symbol_matches_search_filters};

pub(crate) fn search_match_detail(
    symbol: &SymbolMeta,
    query: &str,
    normalized_query: &str,
) -> Option<SymbolSearchMatchDetail> {
    let base_name = symbol_base_name(&symbol.semantic_path);
    let normalized_base_name = base_name.to_ascii_lowercase();
    let normalized_symbol_id = symbol.symbol_id.to_ascii_lowercase();
    let normalized_semantic_path = symbol.semantic_path.to_ascii_lowercase();
    let normalized_scope_path = symbol
        .scope_path
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let normalized_file_path = symbol.file_path.to_ascii_lowercase();
    let normalized_node_kind = symbol.node_kind.to_ascii_lowercase();
    let normalized_signature = symbol
        .signature
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let normalized_parameters = symbol.parameters.join(" ").to_ascii_lowercase();
    let normalized_return_type = symbol
        .return_type
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();
    let normalized_docstring = symbol
        .docstring
        .as_deref()
        .unwrap_or("")
        .to_ascii_lowercase();

    let mut matched_fields = Vec::new();
    if normalized_base_name.contains(normalized_query) {
        matched_fields.push("base_name".to_string());
    }
    if normalized_symbol_id.contains(normalized_query) {
        matched_fields.push("symbol_id".to_string());
    }
    if normalized_semantic_path.contains(normalized_query) {
        matched_fields.push("semantic_path".to_string());
    }
    if normalized_scope_path.contains(normalized_query) {
        matched_fields.push("scope_path".to_string());
    }
    if normalized_file_path.contains(normalized_query) {
        matched_fields.push("file_path".to_string());
    }
    if normalized_node_kind.contains(normalized_query) {
        matched_fields.push("node_kind".to_string());
    }
    if normalized_signature.contains(normalized_query) {
        matched_fields.push("signature".to_string());
    }
    if normalized_parameters.contains(normalized_query) {
        matched_fields.push("parameters".to_string());
    }
    if normalized_return_type.contains(normalized_query) {
        matched_fields.push("return_type".to_string());
    }
    if normalized_docstring.contains(normalized_query) {
        matched_fields.push("docstring".to_string());
    }

    if matched_fields.is_empty() {
        return None;
    }

    let exact_query =
        query == symbol.semantic_path || query == symbol.symbol_id || query == base_name;
    let score = if exact_query {
        1000
    } else if normalized_base_name == normalized_query {
        950
    } else if normalized_symbol_id == normalized_query {
        925
    } else if normalized_semantic_path == normalized_query {
        900
    } else if normalized_base_name.starts_with(normalized_query) {
        850
    } else if normalized_semantic_path.starts_with(normalized_query) {
        825
    } else if normalized_symbol_id.starts_with(normalized_query) {
        800
    } else if normalized_file_path.contains(normalized_query) {
        300
    } else if normalized_signature.contains(normalized_query)
        || normalized_parameters.contains(normalized_query)
        || normalized_return_type.contains(normalized_query)
    {
        200
    } else if normalized_docstring.contains(normalized_query) {
        100
    } else {
        400
    };

    Some(SymbolSearchMatchDetail {
        symbol_id: symbol.symbol_id.clone(),
        score,
        matched_fields,
    })
}
