use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::{normalize_path, visit_tree, visit_tree_with_deadline};
use crate::patching::{collect_python_references, collect_python_references_with_deadline};
use crate::semantic::{
    python_display_byte_range, python_display_header, python_docstring, python_is_overload,
    python_overload_names, python_parameters, python_return_type, semantic_parent_path,
    semantic_path,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(super) fn index_python_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    let normalized_path = normalize_path(path);
    let overload_names = python_overload_names(
        root,
        source,
        deadline.map(|deadline| deadline as &dyn DeadlineCheck),
    )?;
    let mut extraction_error = None;

    let mut callback = |node: Node<'_>| {
        if extraction_error.is_some() {
            return;
        }
        if !matches!(node.kind(), "class_definition" | "function_definition") {
            return;
        }

        let mut references = BTreeSet::new();
        let reference_node = python_reference_node(node);
        let reference_result = match deadline {
            Some(deadline) => collect_python_references_with_deadline(
                path,
                reference_node,
                source,
                &mut references,
                Some(deadline),
            ),
            None => collect_python_references(path, reference_node, source, &mut references),
        };
        if let Err(error) = reference_result {
            extraction_error = Some(error);
            return;
        }
        let is_overload = match python_is_overload(
            node,
            source,
            &overload_names,
            deadline.map(|deadline| deadline as &dyn DeadlineCheck),
        ) {
            Ok(is_overload) => is_overload,
            Err(error) => {
                extraction_error = Some(error);
                return;
            }
        };
        let signature = python_display_header(node, source).ok();
        let path = match semantic_path(node, source) {
            Ok(path) => path,
            Err(_) => return,
        };
        let scope_path = semantic_parent_path(&path);
        let parameters = python_parameters(node, source).unwrap_or_default();
        let return_type = python_return_type(node, source).ok().flatten();
        let docstring = python_docstring(node, source).ok().flatten();

        symbols.push(IndexedSymbol {
            symbol_id: String::new(),
            base_name: symbol_base_name(&path),
            semantic_path: path,
            scope_path,
            file_path: normalized_path.clone(),
            node_kind: node.kind().to_string(),
            byte_range: python_display_byte_range(node),
            signature,
            is_overload,
            parameters,
            return_type,
            docstring,
            references_by_name: references,
            call_arities_by_name: std::collections::BTreeMap::new(),
        });
    };

    match deadline {
        Some(deadline) => visit_tree_with_deadline(root, &mut callback, deadline)?,
        None => visit_tree(root, &mut callback),
    }
    if let Some(error) = extraction_error {
        return Err(error);
    }
    if let Some(deadline) = deadline {
        deadline.check("extracting Python symbols")?;
    }
    Ok(symbols)
}

fn python_reference_node(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == "decorated_definition")
        .unwrap_or(node)
}
