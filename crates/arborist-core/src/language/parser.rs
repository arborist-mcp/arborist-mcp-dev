use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use tree_sitter::{Language, ParseOptions, Parser, Tree};

use crate::language::{MAX_SOURCE_FILE_BYTES, builtin_language_registry};
use crate::model::LanguageId;

pub struct ParsedDocument {
    pub language_id: LanguageId,
    pub tree: Tree,
}

pub fn supported_languages() -> Vec<&'static str> {
    builtin_language_registry().supported_language_names()
}

pub fn parse_document(path: &Path, source: &str) -> Result<ParsedDocument> {
    parse_document_with_timeout(path, source, 0)
}

pub fn parse_document_with_timeout(
    path: &Path,
    source: &str,
    timeout_micros: u64,
) -> Result<ParsedDocument> {
    validate_source_length(path, source.len())?;
    let language_id = detect_language(path)?;
    let mut parser = parser_for_language(language_id)?;
    let tree = if timeout_micros > 0 {
        let deadline = Instant::now() + Duration::from_micros(timeout_micros);
        let mut progress_callback = |_: &tree_sitter::ParseState| Instant::now() >= deadline;
        let parse_options = ParseOptions::new().progress_callback(&mut progress_callback);
        let mut read_source = |byte_offset: usize, _position: tree_sitter::Point| {
            source.as_bytes().get(byte_offset..).unwrap_or_default()
        };
        parser.parse_with_options(&mut read_source, None, Some(parse_options))
    } else {
        parser.parse(source, None)
    }
    .ok_or_else(|| {
        if timeout_micros > 0 {
            anyhow!(
                "parsing {} timed out after {} microseconds",
                path.display(),
                timeout_micros
            )
        } else {
            anyhow!("failed to parse {}", path.display())
        }
    })?;

    Ok(ParsedDocument { language_id, tree })
}

pub(crate) fn validate_source_size(path: &Path, source: &str) -> Result<()> {
    validate_source_length(path, source.len())
}

pub(crate) fn validate_source_length(path: &Path, size: usize) -> Result<()> {
    if size as u64 > MAX_SOURCE_FILE_BYTES {
        bail!(
            "source text too large for {}: size_bytes={} max_file_bytes={}",
            path.display(),
            size,
            MAX_SOURCE_FILE_BYTES,
        );
    }
    Ok(())
}

pub fn parser_for_language(language_id: LanguageId) -> Result<Parser> {
    let language = language_for_id(language_id);
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .context("failed to configure parser language")?;
    Ok(parser)
}

pub fn detect_language(path: &Path) -> Result<LanguageId> {
    let extension = path.extension().and_then(|extension| extension.to_str());
    builtin_language_registry()
        .language_for_extension(extension.unwrap_or_default())
        .ok_or_else(|| {
            anyhow!(
                "unsupported file extension {:?} for {}",
                extension,
                path.display()
            )
        })
}

pub fn language_for_id(language_id: LanguageId) -> Language {
    builtin_language_registry()
        .descriptor(language_id)
        .expect("every LanguageId must have a builtin language descriptor")
        .tree_sitter_language()
}
