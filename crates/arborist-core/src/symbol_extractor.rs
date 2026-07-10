use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{ParsedDocument, contains_kind, node_text, normalize_path, visit_tree};
use crate::model::LanguageId;
use crate::patching::{collect_c_references, collect_python_references};
use crate::semantic::{
    c_function_header, c_parameters, c_return_type, c_semantic_path, python_display_byte_range,
    python_display_header, python_docstring, python_parameters, python_return_type,
    semantic_parent_path, semantic_path,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};

pub(crate) fn index_symbols_from_document(
    path: &Path,
    source: &str,
    document: &ParsedDocument,
) -> Result<Vec<IndexedSymbol>> {
    match document.language_id {
        LanguageId::Python => index_python_symbols(path, source, document.tree.root_node()),
        LanguageId::C => index_c_symbols(path, source, document.tree.root_node()),
    }
}

fn index_python_symbols(path: &Path, source: &str, root: Node<'_>) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    let normalized_path = normalize_path(path);

    let mut callback = |node: Node<'_>| {
        if !matches!(node.kind(), "class_definition" | "function_definition") {
            return;
        }

        let mut references = BTreeSet::new();
        let reference_node = python_reference_node(node);
        let _ = collect_python_references(path, reference_node, source, &mut references);
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
            parameters,
            return_type,
            docstring,
            references_by_name: references,
        });
    };

    visit_tree(root, &mut callback);
    Ok(symbols)
}

fn python_reference_node(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == "decorated_definition")
        .unwrap_or(node)
}

fn index_c_symbols(path: &Path, source: &str, root: Node<'_>) -> Result<Vec<IndexedSymbol>> {
    let normalized_path = normalize_path(path);
    let mut symbols = Vec::new();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "type_definition" => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: symbol_base_name(&name),
                        semantic_path: name,
                        scope_path: None,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(node_text(child, source)?.trim().to_string()),
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        references_by_name: BTreeSet::new(),
                    });
                }
            }
            "declaration" if contains_kind(child, "function_declarator") => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    let scope_path = semantic_parent_path(&name);
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: symbol_base_name(&name),
                        semantic_path: name,
                        scope_path,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(node_text(child, source)?.trim().to_string()),
                        parameters: c_parameters(child, source)?,
                        return_type: c_return_type(child, source)?,
                        docstring: None,
                        references_by_name: BTreeSet::new(),
                    });
                }
            }
            "function_definition" => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    let mut references = BTreeSet::new();
                    collect_c_references(child, source, &mut references)?;
                    let scope_path = semantic_parent_path(&name);
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: symbol_base_name(&name),
                        semantic_path: name,
                        scope_path,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(c_function_header(child, source)?),
                        parameters: c_parameters(child, source)?,
                        return_type: c_return_type(child, source)?,
                        docstring: None,
                        references_by_name: references,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(symbols)
}
