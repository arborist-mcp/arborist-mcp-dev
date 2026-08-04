use std::path::Path;

use anyhow::Result;
use tree_sitter::{Node, Tree};

use crate::deadline::DeadlineCheck;
use crate::language::{LanguageCapabilities, builtin_language_registry};
use crate::model::{LanguageId, SemanticSkeleton};

pub(crate) mod c;
pub(crate) mod go;
pub(crate) mod javascript;
mod paths;
pub(crate) mod python;
mod python_identity;
mod python_overloads;
pub(crate) mod rust;

pub(crate) use paths::{semantic_depth, semantic_parent_path, semantic_path};

pub(crate) use c::c_is_callable_declaration;
pub(crate) use c::c_is_scoped_enumerator;
pub(crate) use c::c_named_node_name;
pub(crate) use c::c_template_instantiation_name;
pub(crate) use c::c_using_declaration_name;
pub(crate) use c::cpp_callable_symbol_id;
pub(crate) use c::has_c_internal_linkage;
pub use c::{c_function_header, c_semantic_path, c_symbol_id_for_node};
pub(crate) use c::{c_parameters, c_return_type};
pub(crate) use c::{c_symbol_nodes, c_symbol_nodes_with_deadline};
pub(crate) use python::{
    python_display_byte_range, python_display_header, python_docstring, python_parameters,
    python_return_type, python_symbol_id_for_node,
};
pub(crate) use python_identity::{PythonSymbolIdentity, python_symbol_ids};
pub(crate) use python_overloads::{python_is_overload, python_overload_names};

pub(crate) fn get_semantic_skeleton_with_deadline(
    path: &Path,
    language_id: LanguageId,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<SemanticSkeleton> {
    let registry = builtin_language_registry();
    registry.require_capability(
        language_id,
        LanguageCapabilities::SEMANTIC_SKELETON,
        "semantic skeleton requests",
    )?;
    registry
        .adapter(language_id)
        .expect("every LanguageId must have a builtin language adapter")
        .build_semantic_skeleton(path, source, tree, depth_limit, expand_nodes, deadline)
}

pub(crate) fn find_semantic_node_with_deadline<'tree>(
    language_id: LanguageId,
    path: &Path,
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<Node<'tree>>> {
    let registry = builtin_language_registry();
    registry.require_capability(
        language_id,
        LanguageCapabilities::SEMANTIC_SKELETON,
        "semantic symbol lookup",
    )?;
    registry
        .adapter(language_id)
        .expect("every LanguageId must have a builtin language adapter")
        .find_semantic_node(path, tree, source, target_path, deadline)
}

pub fn ascend_to_symbol(language_id: LanguageId, node: Node<'_>) -> Option<Node<'_>> {
    builtin_language_registry()
        .adapter(language_id)
        .expect("every LanguageId must have a builtin language adapter")
        .ascend_to_symbol(node)
}

pub(crate) fn ascend_python_to_symbol(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);

    while let Some(candidate) = current {
        if candidate.kind() == "decorated_definition" {
            let mut cursor = candidate.walk();
            for child in candidate.named_children(&mut cursor) {
                if matches!(child.kind(), "class_definition" | "function_definition") {
                    return Some(child);
                }
            }
        }

        if matches!(candidate.kind(), "class_definition" | "function_definition") {
            return Some(candidate);
        }
        current = candidate.parent();
    }

    None
}

pub(crate) fn ascend_c_to_symbol(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);

    while let Some(candidate) = current {
        let is_symbol = matches!(
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
            || c::c_is_callable_declaration(candidate);

        if is_symbol {
            return Some(candidate);
        }
        current = candidate.parent();
    }

    None
}

pub(crate) fn ascend_javascript_to_symbol(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = Some(node);

    while let Some(candidate) = current {
        if javascript::is_javascript_symbol_node(candidate) {
            return Some(candidate);
        }
        current = candidate.parent();
    }

    None
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::get_semantic_skeleton_with_deadline;
    use crate::deadline::CooperativeDeadline;
    use crate::language::parse_document;

    #[test]
    fn semantic_skeleton_traversal_checks_expired_deadlines() {
        let cases = [
            (
                Path::new("sample.py"),
                "def sample():
    return 1
",
            ),
            (
                Path::new("sample.c"),
                "int sample(void) { return 1; }
",
            ),
            (
                Path::new("sample.js"),
                "export function sample() { return 1; }
",
            ),
        ];

        for (path, source) in cases {
            let document = parse_document(path, source).expect("source should parse");
            let deadline = CooperativeDeadline::expired_for_tests(1, "semantic skeleton");
            let error = get_semantic_skeleton_with_deadline(
                path,
                document.language_id,
                source,
                &document.tree,
                2,
                &[],
                Some(&deadline),
            )
            .expect_err("expired skeleton traversal should fail");

            assert!(
                error
                    .to_string()
                    .contains("semantic skeleton timeout exceeded")
            );
        }
    }
}
