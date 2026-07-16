use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path};

pub const C_HEADER_EXTENSIONS: &[&str] = &["h"];
pub const C_SOURCE_EXTENSIONS: &[&str] = &["c"];
pub const CPP_HEADER_EXTENSIONS: &[&str] = &["hpp", "hh", "hxx", "h++"];
pub const CPP_SOURCE_EXTENSIONS: &[&str] = &["cc", "cpp", "cxx", "c++"];

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
        CPP_SOURCE_EXTENSIONS
    } else {
        C_SOURCE_EXTENSIONS
    };
    let fallback_extensions = if is_cpp_header_path(include_path) {
        C_SOURCE_EXTENSIONS
    } else {
        CPP_SOURCE_EXTENSIONS
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
