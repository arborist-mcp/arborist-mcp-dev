use std::path::Path;

use anyhow::Result;

use crate::language::{LanguageCapabilities, ParsedDocument, builtin_language_registry};
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
    let registry = builtin_language_registry();
    registry.require_capability(
        document.language_id,
        LanguageCapabilities::SYMBOL_INDEX,
        "symbol extraction",
    )?;
    registry
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
    fn syntax_only_language_symbol_extraction_is_rejected_before_walking_the_tree() {
        let source = "export function sample() { return 1; }";
        let path = Path::new("sample.js");
        let document = parse_document(path, source).expect("source should parse");

        let error = index_symbols_from_document_with_deadline(path, source, &document, None)
            .expect_err("syntax-only languages must not enter symbol extraction");
        assert!(error.to_string().contains("JavaScript"));
        assert!(error.to_string().contains("symbol indexing"));
    }

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
