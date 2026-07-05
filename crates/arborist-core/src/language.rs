use std::fs;
use std::path::Path;

use anyhow::{Context, Result, anyhow, bail};
use tree_sitter::{Language, Node, Parser, Point, Tree};

use crate::model::{LanguageId, Position};

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
        Some("py") | Some("pyi") => Ok(LanguageId::Python),
        Some("c") | Some("h") => Ok(LanguageId::C),
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
    let candidate = parent.join(include_target);
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
