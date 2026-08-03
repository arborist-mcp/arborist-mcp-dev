use std::path::Path;

use anyhow::Result;

use crate::language::{ParsedDocument, builtin_language_registry};
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) mod c;
pub(crate) mod python;

pub(crate) fn index_symbols_from_document(
    path: &Path,
    source: &str,
    document: &ParsedDocument,
) -> Result<Vec<IndexedSymbol>> {
    index_symbols_from_document_with_deadline(path, source, document, None)
}

pub(crate) fn index_symbols_from_document_with_deadline(
    path: &Path,
    source: &str,
    document: &ParsedDocument,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    builtin_language_registry()
        .adapter(document.language_id)
        .expect("every LanguageId must have a builtin language adapter")
        .extract_symbols(path, source, document, deadline)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use super::index_symbols_from_document_with_deadline;
    use crate::language::parse_document;
    use crate::workspace_scan::WorkspaceScanDeadline;

    #[test]
    fn symbol_extraction_rejects_expired_deadline() {
        let source = "def sample():\n    return 1\n";
        let path = Path::new("sample.py");
        let document = parse_document(path, source).expect("source should parse");
        let deadline = WorkspaceScanDeadline {
            deadline: Some(Instant::now() - Duration::from_millis(1)),
            timeout_ms: Some(1),
        };

        let error =
            index_symbols_from_document_with_deadline(path, source, &document, Some(&deadline))
                .expect_err("expired symbol extraction should fail before walking the tree");
        assert!(
            error
                .to_string()
                .contains("workspace scan timeout exceeded")
        );
    }
}
