use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::patching::{
    collect_c_call_arities, collect_c_graph_references, collect_cpp_call_arities,
};
use crate::semantic::{
    c_function_header, c_is_callable_declaration, c_parameters, c_return_type, c_semantic_path,
    c_symbol_nodes, semantic_parent_path,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};

pub(super) fn index_c_symbols(
    path: &Path,
    source: &str,
    root: Node<'_>,
    is_cpp: bool,
) -> Result<Vec<IndexedSymbol>> {
    let normalized_path = normalize_path(path);
    let mut symbols = Vec::new();
    for child in c_symbol_nodes(path, root, source)? {
        match child.kind() {
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
            | "using_declaration" => {
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
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        references_by_name: BTreeSet::new(),
                        call_arities_by_name: BTreeMap::new(),
                    });
                }
            }
            "declaration" | "field_declaration" if c_is_callable_declaration(child) => {
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
                        call_arities_by_name: BTreeMap::new(),
                    });
                }
            }
            "function_definition" => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    let mut references = BTreeSet::new();
                    collect_c_graph_references(child, source, &mut references)?;
                    let mut call_arities = BTreeMap::new();
                    if is_cpp {
                        collect_cpp_call_arities(child, source, &mut call_arities)?;
                    } else {
                        collect_c_call_arities(child, source, &mut call_arities)?;
                    }
                    references.extend(call_arities.keys().cloned());
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
                        call_arities_by_name: call_arities,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(symbols)
}
