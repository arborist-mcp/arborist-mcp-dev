use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use tree_sitter::{Language, Parser, Tree};

use crate::language::{
    C_HEADER_EXTENSIONS, C_SOURCE_EXTENSIONS, CPP_HEADER_EXTENSIONS, CPP_SOURCE_EXTENSIONS,
};
use crate::model::LanguageId;

pub struct ParsedDocument {
    pub language_id: LanguageId,
    pub tree: Tree,
}

pub fn supported_languages() -> Vec<&'static str> {
    vec!["python", "c", "cpp"]
}

pub fn parse_document(path: &Path, source: &str) -> Result<ParsedDocument> {
    let language_id = detect_language(path)?;
    let mut parser = parser_for_language(language_id)?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("failed to parse {}", path.display()))?;

    Ok(ParsedDocument { language_id, tree })
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
    match path.extension().and_then(|ext| ext.to_str()) {
        Some(ext) if ext.eq_ignore_ascii_case("py") || ext.eq_ignore_ascii_case("pyi") => {
            Ok(LanguageId::Python)
        }
        Some(ext)
            if C_SOURCE_EXTENSIONS
                .iter()
                .any(|extension| ext.eq_ignore_ascii_case(extension))
                || C_HEADER_EXTENSIONS
                    .iter()
                    .any(|extension| ext.eq_ignore_ascii_case(extension)) =>
        {
            Ok(LanguageId::C)
        }
        Some(ext)
            if CPP_SOURCE_EXTENSIONS
                .iter()
                .any(|extension| ext.eq_ignore_ascii_case(extension))
                || CPP_HEADER_EXTENSIONS
                    .iter()
                    .any(|extension| ext.eq_ignore_ascii_case(extension)) =>
        {
            Ok(LanguageId::Cpp)
        }
        other => bail!(
            "unsupported file extension {:?} for {}",
            other,
            path.display()
        ),
    }
}

pub fn language_for_id(language_id: LanguageId) -> Language {
    match language_id {
        LanguageId::Python => tree_sitter_python::LANGUAGE.into(),
        LanguageId::C => tree_sitter_c::LANGUAGE.into(),
        LanguageId::Cpp => tree_sitter_cpp::LANGUAGE.into(),
    }
}
