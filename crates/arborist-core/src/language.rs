use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use tree_sitter::{Language, Node, Parser, Point, Tree};

use crate::model::{LanguageId, Position};

pub const C_HEADER_EXTENSIONS: &[&str] = &["h", "hpp", "hh"];
pub const C_SOURCE_EXTENSIONS: &[&str] = &["c"];

pub struct ParsedDocument {
    pub language_id: LanguageId,
    pub tree: Tree,
}

pub fn supported_languages() -> Vec<&'static str> {
    vec!["python", "c"]
}

pub fn read_source(path: &Path) -> Result<String> {
    fs::read_to_string(path)
        .with_context(|| format!("failed to read source file {}", path.display()))
}

pub fn normalize_absolute_path(path: &Path) -> Result<PathBuf> {
    let absolute_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };

    let mut normalized = PathBuf::new();
    for component in absolute_path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
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
        other => bail!(
            "unsupported file extension {:?} for {}",
            other,
            path.display()
        ),
    }
}

pub fn is_c_header_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            C_HEADER_EXTENSIONS
                .iter()
                .any(|header_extension| extension.eq_ignore_ascii_case(header_extension))
        })
}

pub(crate) fn extension_case_candidates(path: &Path, extensions: &[&str]) -> Vec<String> {
    let uppercase_first = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension
                .chars()
                .all(|character| character.is_ascii_uppercase())
        });

    extensions
        .iter()
        .flat_map(|extension| {
            let uppercase = extension.to_ascii_uppercase();
            if uppercase_first {
                [uppercase, (*extension).to_string()]
            } else {
                [(*extension).to_string(), uppercase]
            }
        })
        .collect()
}

pub fn c_companion_source_path(include_path: &Path) -> Option<PathBuf> {
    if !is_c_header_path(include_path) {
        return None;
    }

    let candidates = extension_case_candidates(include_path, C_SOURCE_EXTENSIONS)
        .into_iter()
        .map(|source_extension| include_path.with_extension(source_extension))
        .collect::<Vec<_>>();

    candidates
        .iter()
        .find_map(|candidate| existing_path_with_exact_file_name(candidate))
        .or_else(|| {
            candidates
                .iter()
                .find_map(|candidate| existing_path_with_case_insensitive_file_name(candidate))
        })
        .or_else(|| candidates.into_iter().find(|candidate| candidate.exists()))
}

fn existing_path_with_exact_file_name(candidate: &Path) -> Option<PathBuf> {
    existing_path_with_file_name(candidate, |left, right| left == right)
}

fn existing_path_with_case_insensitive_file_name(candidate: &Path) -> Option<PathBuf> {
    existing_path_with_file_name(candidate, |left, right| left.eq_ignore_ascii_case(right))
}

fn existing_path_with_file_name(
    candidate: &Path,
    matches_name: impl Fn(&str, &str) -> bool,
) -> Option<PathBuf> {
    let parent = candidate.parent()?;
    let candidate_name = candidate.file_name()?.to_str()?;
    for entry in fs::read_dir(parent).ok()? {
        let entry = entry.ok()?;
        let entry_name = entry.file_name();
        let entry_name = entry_name.to_str()?;
        if matches_name(entry_name, candidate_name) {
            return Some(entry.path());
        }
    }
    None
}

pub fn language_for_id(language_id: LanguageId) -> Language {
    match language_id {
        LanguageId::Python => tree_sitter_python::LANGUAGE.into(),
        LanguageId::C => tree_sitter_c::LANGUAGE.into(),
    }
}

pub fn node_text<'a>(node: Node<'_>, source: &'a str) -> Result<&'a str> {
    Ok(node.utf8_text(source.as_bytes())?)
}

pub fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn position_from(point: Point) -> Position {
    Position {
        row: point.row,
        column: point.column,
    }
}

pub fn point_for_offset(source: &str, byte_offset: usize) -> Result<Point> {
    if byte_offset > source.len() {
        bail!(
            "byte offset {} is out of bounds for source of length {}",
            byte_offset,
            source.len()
        );
    }
    if !source.is_char_boundary(byte_offset) {
        bail!(
            "byte offset {} does not align to a UTF-8 character boundary",
            byte_offset
        );
    }

    let mut row = 0;
    let mut column = 0;
    for byte in source.as_bytes().iter().take(byte_offset) {
        if *byte == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    Ok(Point { row, column })
}

pub fn offset_for_position(source: &str, position: &Position) -> Result<usize> {
    let mut row = 0;
    let mut column = 0;

    for (index, byte) in source.as_bytes().iter().enumerate() {
        if row == position.row && column == position.column {
            if !source.is_char_boundary(index) {
                bail!(
                    "position {}:{} maps to byte offset {} which does not align to a UTF-8 character boundary",
                    position.row,
                    position.column,
                    index
                );
            }
            return Ok(index);
        }

        if *byte == b'\n' {
            row += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    if row == position.row && column == position.column {
        return Ok(source.len());
    }

    bail!(
        "position {}:{} is out of bounds for source",
        position.row,
        position.column
    )
}

pub fn c_include_targets(root: Node<'_>, source: &str) -> Result<Vec<String>> {
    let mut targets = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "preproc_include" {
            continue;
        }
        let Some(path_node) = child.child_by_field_name("path") else {
            continue;
        };
        let raw = node_text(path_node, source)?.trim();
        if let Some(target) = normalize_include_target(raw) {
            targets.push(target);
        }
    }
    Ok(targets)
}

pub fn c_local_include_targets(root: Node<'_>, source: &str) -> Result<Vec<String>> {
    let mut targets = Vec::new();
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() != "preproc_include" {
            continue;
        }
        let Some(path_node) = child.child_by_field_name("path") else {
            continue;
        };
        let raw = node_text(path_node, source)?.trim();
        if let Some(target) = normalize_local_include_target(raw) {
            targets.push(target);
        }
    }
    Ok(targets)
}

pub fn resolve_local_c_include(
    current_path: &Path,
    include_target: &str,
) -> Option<std::path::PathBuf> {
    let parent = current_path.parent()?;
    let candidate = normalize_absolute_path(&parent.join(include_target)).ok()?;
    candidate.exists().then_some(candidate)
}

fn normalize_include_target(raw: &str) -> Option<String> {
    raw.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            raw.strip_prefix('<')
                .and_then(|value| value.strip_suffix('>'))
        })
        .map(str::to_string)
}

fn normalize_local_include_target(raw: &str) -> Option<String> {
    raw.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .map(str::to_string)
}

pub fn visit_tree(node: Node<'_>, callback: &mut impl FnMut(Node<'_>)) {
    callback(node);
    let child_count = node.child_count();
    for index in 0..child_count {
        if let Some(child) = node.child(index) {
            visit_tree(child, callback);
        }
    }
}

pub fn contains_kind(node: Node<'_>, wanted: &str) -> bool {
    if node.kind() == wanted {
        return true;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if contains_kind(child, wanted) {
            return true;
        }
    }

    false
}

pub fn first_identifier(node: Node<'_>, source: &str) -> Result<Option<String>> {
    find_identifier(node, &["identifier", "field_identifier"], source)
}

pub fn last_type_identifier(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut hits = Vec::new();
    collect_identifiers(node, &["type_identifier"], source, &mut hits)?;
    Ok(hits.pop())
}

pub fn find_identifier(node: Node<'_>, kinds: &[&str], source: &str) -> Result<Option<String>> {
    if kinds.contains(&node.kind()) {
        return Ok(Some(node_text(node, source)?.trim().to_string()));
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_identifier(child, kinds, source)? {
            return Ok(Some(found));
        }
    }

    Ok(None)
}

pub fn collect_identifiers(
    node: Node<'_>,
    kinds: &[&str],
    source: &str,
    hits: &mut Vec<String>,
) -> Result<()> {
    if kinds.contains(&node.kind()) {
        hits.push(node_text(node, source)?.trim().to_string());
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_identifiers(child, kinds, source, hits)?;
    }

    Ok(())
}

pub fn is_field_node(parent: Node<'_>, field_name: &str, node: Node<'_>) -> bool {
    parent
        .child_by_field_name(field_name)
        .is_some_and(|candidate| candidate.id() == node.id())
}

pub fn contains_node(container: Node<'_>, node: Node<'_>) -> bool {
    container.start_byte() <= node.start_byte() && container.end_byte() >= node.end_byte()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tree_sitter::Point;

    use super::{
        c_companion_source_path, detect_language, is_c_header_path, offset_for_position,
        point_for_offset,
    };
    use crate::model::{LanguageId, Position};

    #[test]
    fn detect_language_accepts_uppercase_extensions() {
        assert_eq!(
            detect_language(Path::new("sample.PY")).unwrap(),
            LanguageId::Python
        );
        assert_eq!(
            detect_language(Path::new("sample.PYI")).unwrap(),
            LanguageId::Python
        );
        assert_eq!(
            detect_language(Path::new("sample.C")).unwrap(),
            LanguageId::C
        );
        assert_eq!(
            detect_language(Path::new("sample.H")).unwrap(),
            LanguageId::C
        );
        assert_eq!(
            detect_language(Path::new("sample.HPP")).unwrap(),
            LanguageId::C
        );
        assert_eq!(
            detect_language(Path::new("sample.HH")).unwrap(),
            LanguageId::C
        );
    }

    #[test]
    fn detect_language_reports_original_unsupported_extension() {
        let error = detect_language(Path::new("sample.TXT"))
            .expect_err("unsupported extensions should be reported");

        assert!(error.to_string().contains(r#"Some("TXT")"#));
    }

    #[test]
    fn c_header_detection_accepts_uppercase_extensions() {
        assert!(is_c_header_path(Path::new("sample.h")));
        assert!(is_c_header_path(Path::new("sample.H")));
        assert!(is_c_header_path(Path::new("sample.HPP")));
        assert!(is_c_header_path(Path::new("sample.HH")));
        assert!(!is_c_header_path(Path::new("sample.c")));
    }

    #[test]
    fn companion_c_source_prefers_header_case_style() {
        let dir = std::env::temp_dir().join(format!(
            "arborist-language-companion-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let uppercase_header = dir.join("helper.H");
        let uppercase_source = dir.join("helper.C");
        std::fs::write(&uppercase_header, "int helper(int value);\n").unwrap();
        std::fs::write(
            &uppercase_source,
            "int helper(int value) { return value + 1; }\n",
        )
        .unwrap();

        assert_eq!(
            c_companion_source_path(&uppercase_header).unwrap(),
            uppercase_source
        );

        let mixed_header = dir.join("mixed.HPP");
        let lowercase_source = dir.join("mixed.c");
        std::fs::write(&mixed_header, "int mixed(int value);\n").unwrap();
        std::fs::write(
            &lowercase_source,
            "int mixed(int value) { return value + 1; }\n",
        )
        .unwrap();

        assert_eq!(
            c_companion_source_path(&mixed_header).unwrap(),
            lowercase_source
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn point_for_offset_uses_tree_sitter_byte_columns() {
        let source = "é\nx";

        assert_eq!(
            point_for_offset(source, "é".len()).unwrap(),
            Point { row: 0, column: 2 }
        );
        assert_eq!(
            point_for_offset(source, "é\n".len()).unwrap(),
            Point { row: 1, column: 0 }
        );
    }

    #[test]
    fn offset_for_position_uses_tree_sitter_byte_columns() {
        let source = "é\nx";

        assert_eq!(
            offset_for_position(source, &Position { row: 0, column: 2 }).unwrap(),
            "é".len()
        );
        assert_eq!(
            offset_for_position(source, &Position { row: 1, column: 1 }).unwrap(),
            source.len()
        );
    }

    #[test]
    fn offset_for_position_rejects_non_boundary_byte_columns() {
        let source = "é\nx";

        let error = offset_for_position(source, &Position { row: 0, column: 1 })
            .expect_err("positions inside a UTF-8 character should be rejected");

        assert!(
            error
                .to_string()
                .contains("does not align to a UTF-8 character boundary")
        );
    }
}
