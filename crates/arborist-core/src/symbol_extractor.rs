use std::path::Path;

use anyhow::Result;

use crate::language::ParsedDocument;
use crate::model::LanguageId;
use crate::symbol_index_model::IndexedSymbol;

mod c;
mod python;

pub(crate) fn index_symbols_from_document(
    path: &Path,
    source: &str,
    document: &ParsedDocument,
) -> Result<Vec<IndexedSymbol>> {
    match document.language_id {
        LanguageId::Python => python::index_python_symbols(path, source, document.tree.root_node()),
        LanguageId::C => c::index_c_symbols(path, source, document.tree.root_node(), false),
        LanguageId::Cpp => c::index_c_symbols(path, source, document.tree.root_node(), true),
    }
}
