use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub(crate) struct IndexedSymbol {
    pub(crate) symbol_id: String,
    pub(crate) semantic_path: String,
    pub(crate) base_name: String,
    pub(crate) scope_path: Option<String>,
    pub(crate) file_path: String,
    pub(crate) node_kind: String,
    pub(crate) byte_range: (usize, usize),
    pub(crate) signature: Option<String>,
    pub(crate) parameters: Vec<String>,
    pub(crate) return_type: Option<String>,
    pub(crate) docstring: Option<String>,
    pub(crate) references_by_name: BTreeSet<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct PersistedFileState {
    pub(crate) file_path: String,
    pub(crate) fingerprint: u64,
}

pub(crate) fn symbol_base_name(semantic_path: &str) -> String {
    semantic_path
        .rsplit("::")
        .next()
        .unwrap_or(semantic_path)
        .rsplit('.')
        .next()
        .unwrap_or(semantic_path)
        .to_string()
}

pub(crate) fn symbol_kind_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" => 3,
        "class_definition" => 3,
        "alias_declaration" | "class_specifier" | "concept_definition" | "type_definition" => 2,
        "declaration" | "field_declaration" => 1,
        _ => 0,
    }
}
