use anyhow::Result;
use tree_sitter::Node;

use crate::language::node_text;

pub(crate) fn semantic_path(node: Node<'_>, source: &str) -> Result<String> {
    let mut segments = Vec::new();
    let mut current = Some(node);

    while let Some(candidate) = current {
        if matches!(candidate.kind(), "class_definition" | "function_definition")
            && let Some(name_node) = candidate.child_by_field_name("name")
        {
            segments.push(node_text(name_node, source)?.trim().to_string());
        }
        current = candidate.parent();
    }

    segments.reverse();
    Ok(segments.join("."))
}

pub(crate) fn semantic_depth(node: Node<'_>) -> usize {
    let mut depth = 0;
    let mut current = Some(node);

    while let Some(candidate) = current {
        if matches!(candidate.kind(), "class_definition" | "function_definition") {
            depth += 1;
        }
        current = candidate.parent();
    }

    depth
}

pub(crate) fn semantic_parent_path(path: &str) -> Option<String> {
    if is_file_scoped_c_semantic_path(path) {
        return None;
    }

    path.rsplit_once("::")
        .or_else(|| path.rsplit_once('.'))
        .map(|(parent, _)| parent.to_string())
        .filter(|parent| !parent.is_empty())
}

fn is_file_scoped_c_semantic_path(path: &str) -> bool {
    if path.contains('/') || path.contains('\\') {
        return true;
    }

    path.rsplit_once("::")
        .and_then(|(scope, _)| scope.rsplit_once('.').map(|(_, extension)| extension))
        .is_some_and(|extension| {
            [
                "c", "h", "cc", "cpp", "cxx", "c++", "hpp", "hh", "hxx", "h++",
            ]
            .iter()
            .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}
