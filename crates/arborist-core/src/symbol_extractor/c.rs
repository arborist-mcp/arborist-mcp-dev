use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path};
use crate::patching::{
    collect_c_call_arities, collect_c_call_arities_with_deadline, collect_c_graph_references,
    collect_c_graph_references_with_deadline, collect_cpp_call_arities,
    collect_cpp_call_arities_with_deadline,
};
use crate::semantic::{
    c_function_header, c_is_callable_declaration, c_parameters, c_return_type, c_semantic_path,
    c_symbol_nodes, c_symbol_nodes_with_deadline, semantic_parent_path,
};
use crate::symbol_index_model::{IndexedSymbol, symbol_base_name};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_c_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    is_cpp: bool,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let normalized_path = normalize_path(path);
    let mut symbols = Vec::new();
    let symbol_nodes = match deadline {
        Some(deadline) => c_symbol_nodes_with_deadline(path, root, source, Some(deadline))?,
        None => c_symbol_nodes(path, root, source)?,
    };
    for child in symbol_nodes {
        if let Some(deadline) = deadline {
            deadline.check("extracting C/C++ symbols")?;
        }
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
                        is_overload: false,
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
                        is_overload: false,
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
                    if let Some(deadline) = deadline {
                        collect_c_graph_references_with_deadline(
                            child,
                            source,
                            &mut references,
                            deadline,
                        )?;
                    } else {
                        collect_c_graph_references(child, source, &mut references)?;
                    }
                    let mut call_arities = BTreeMap::new();
                    if is_cpp {
                        if let Some(deadline) = deadline {
                            collect_cpp_call_arities_with_deadline(
                                child,
                                source,
                                &mut call_arities,
                                Some(deadline),
                            )?;
                        } else {
                            collect_cpp_call_arities(child, source, &mut call_arities)?;
                        }
                    } else {
                        if let Some(deadline) = deadline {
                            collect_c_call_arities_with_deadline(
                                child,
                                source,
                                &mut call_arities,
                                Some(deadline),
                            )?;
                        } else {
                            collect_c_call_arities(child, source, &mut call_arities)?;
                        }
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
                        is_overload: false,
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

    if let Some(deadline) = deadline {
        deadline.check("extracting C/C++ symbols")?;
    }
    Ok(symbols)
}
