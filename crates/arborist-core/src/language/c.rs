use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};

pub const C_HEADER_EXTENSIONS: &[&str] = &["h"];
pub const C_SOURCE_EXTENSIONS: &[&str] = &["c"];
pub const CPP_HEADER_EXTENSIONS: &[&str] = &["hpp", "hh", "hxx", "h++"];
pub const CPP_SOURCE_EXTENSIONS: &[&str] = &["cc", "cpp", "cxx", "c++", "tpp", "tcc", "ipp", "inl"];

const CPP_COMPANION_SOURCE_EXTENSIONS: &[&str] = &["cc", "cpp", "cxx", "c++"];

pub const C_FAMILY_HEADER_EXTENSIONS: &[&str] = &["h", "hpp", "hh", "hxx", "h++"];

pub fn is_c_header_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            C_FAMILY_HEADER_EXTENSIONS
                .iter()
                .any(|header_extension| extension.eq_ignore_ascii_case(header_extension))
        })
}

fn is_cpp_header_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            CPP_HEADER_EXTENSIONS
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

    let preferred_extensions = if is_cpp_header_path(include_path) {
        CPP_COMPANION_SOURCE_EXTENSIONS
    } else {
        C_SOURCE_EXTENSIONS
    };
    let fallback_extensions = if is_cpp_header_path(include_path) {
        C_SOURCE_EXTENSIONS
    } else {
        CPP_COMPANION_SOURCE_EXTENSIONS
    };
    let candidates = extension_case_candidates(include_path, preferred_extensions)
        .into_iter()
        .chain(extension_case_candidates(include_path, fallback_extensions))
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

pub fn c_include_targets(root: Node<'_>, source: &str) -> Result<Vec<String>> {
    include_targets_for_nodes(
        c_include_target_nodes(root, source, None)?,
        source,
        normalize_include_target,
    )
}

pub(crate) fn c_include_targets_before(
    root: Node<'_>,
    source: &str,
    byte_offset: usize,
) -> Result<Vec<String>> {
    include_targets_for_nodes(
        c_include_target_nodes(root, source, Some(byte_offset))?,
        source,
        normalize_include_target,
    )
}

pub fn c_local_include_targets(root: Node<'_>, source: &str) -> Result<Vec<String>> {
    include_targets_for_nodes(
        c_include_target_nodes(root, source, None)?,
        source,
        normalize_local_include_target,
    )
}

fn include_targets_for_nodes(
    includes: Vec<Node<'_>>,
    source: &str,
    normalize: impl Fn(&str) -> Option<String>,
) -> Result<Vec<String>> {
    let mut targets = Vec::new();
    for include in includes {
        if let Some(target) = include_target_for_node(include, source, &normalize)? {
            targets.push(target);
        }
    }
    Ok(targets)
}

fn c_include_target_nodes<'tree>(
    root: Node<'tree>,
    source: &str,
    byte_offset: Option<usize>,
) -> Result<Vec<Node<'tree>>> {
    let mut includes = Vec::new();
    collect_c_include_target_nodes(root, source, byte_offset, &mut includes)?;
    Ok(includes)
}

fn collect_c_include_target_nodes<'tree>(
    node: Node<'tree>,
    source: &str,
    byte_offset: Option<usize>,
    includes: &mut Vec<Node<'tree>>,
) -> Result<()> {
    match node.kind() {
        "preproc_include" => {
            if byte_offset.is_none_or(|offset| node.start_byte() < offset) {
                includes.push(node);
            }
        }
        "preproc_if" | "preproc_elif" => {
            let Some(condition) = node.child_by_field_name("condition") else {
                return Ok(());
            };
            let Some(active) = known_preprocessor_condition(condition, source)? else {
                return Ok(());
            };
            let alternative = node.child_by_field_name("alternative");
            if active {
                let mut cursor = node.walk();
                for child in node.named_children(&mut cursor) {
                    if child == condition || alternative == Some(child) {
                        continue;
                    }
                    collect_c_include_target_nodes(child, source, byte_offset, includes)?;
                }
            } else if let Some(alternative) = alternative {
                collect_c_include_target_nodes(alternative, source, byte_offset, includes)?;
            }
        }
        "preproc_ifdef" | "preproc_elifdef" => {
            // Macro state is not available during static indexing, so neither branch is trusted.
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_c_include_target_nodes(child, source, byte_offset, includes)?;
            }
        }
    }
    Ok(())
}

fn known_preprocessor_condition(condition: Node<'_>, source: &str) -> Result<Option<bool>> {
    match node_text(condition, source)?.trim() {
        "1" => Ok(Some(true)),
        "0" => Ok(Some(false)),
        _ => Ok(None),
    }
}

fn include_target_for_node(
    include: Node<'_>,
    source: &str,
    normalize: impl FnOnce(&str) -> Option<String>,
) -> Result<Option<String>> {
    let Some(path_node) = include.child_by_field_name("path") else {
        return Ok(None);
    };
    let raw = node_text(path_node, source)?.trim();
    Ok(normalize(raw))
}

pub fn resolve_local_c_include(current_path: &Path, include_target: &str) -> Option<PathBuf> {
    let parent = current_path.parent()?;
    let candidate = normalize_absolute_path(&parent.join(include_target)).ok()?;
    candidate.exists().then_some(candidate)
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
