use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Tree};

use crate::language::node_text;
use crate::model::{LanguageId, SemanticSkeleton};

mod c;
mod python;

pub(crate) use c::c_is_callable_declaration;
pub(crate) use c::c_is_scoped_enumerator;
pub(crate) use c::c_named_node_name;
pub(crate) use c::c_symbol_nodes;
pub(crate) use c::c_template_instantiation_name;
pub(crate) use c::c_using_declaration_name;
pub(crate) use c::cpp_callable_symbol_id;
pub(crate) use c::has_c_internal_linkage;
pub use c::{c_function_header, c_semantic_path, c_symbol_id_for_node};
pub(crate) use c::{c_parameters, c_return_type};
pub(crate) use python::{
    python_display_byte_range, python_display_header, python_docstring, python_parameters,
    python_return_type,
};

pub fn get_semantic_skeleton(
    path: &Path,
    language_id: LanguageId,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    match language_id {
        LanguageId::Python => {
            python::build_python_skeleton(path, source, tree, depth_limit, expand_nodes)
        }
        LanguageId::C | LanguageId::Cpp => c::build_c_skeleton(path, source, tree, expand_nodes),
    }
}

pub fn semantic_path(node: Node<'_>, source: &str) -> Result<String> {
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

pub fn semantic_depth(node: Node<'_>) -> usize {
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

pub fn find_semantic_node<'tree>(
    language_id: LanguageId,
    path: &Path,
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
) -> Result<Option<Node<'tree>>> {
    match language_id {
        LanguageId::Python => python::find_python_semantic_node(tree, source, target_path),
        LanguageId::C | LanguageId::Cpp => c::find_c_semantic_node(path, tree, source, target_path),
    }
}

pub fn ascend_to_symbol(language_id: LanguageId, node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);

    while let Some(candidate) = current {
        if matches!(language_id, LanguageId::Python) && candidate.kind() == "decorated_definition" {
            let mut cursor = candidate.walk();
            for child in candidate.named_children(&mut cursor) {
                if matches!(child.kind(), "class_definition" | "function_definition") {
                    return Some(child);
                }
            }
        }

        let is_symbol = match language_id {
            LanguageId::Python => {
                matches!(candidate.kind(), "class_definition" | "function_definition")
            }
            LanguageId::C | LanguageId::Cpp => {
                matches!(
                    candidate.kind(),
                    "alias_declaration"
                        | "class_specifier"
                        | "concept_definition"
                        | "enum_specifier"
                        | "enumerator"
                        | "namespace_alias_definition"
                        | "struct_specifier"
                        | "template_instantiation"
                        | "type_definition"
                        | "union_specifier"
                        | "using_declaration"
                ) || candidate.kind() == "function_definition"
                    || c::c_is_callable_declaration(candidate)
            }
        };

        if is_symbol {
            return Some(candidate);
        }
        current = candidate.parent();
    }

    None
}
