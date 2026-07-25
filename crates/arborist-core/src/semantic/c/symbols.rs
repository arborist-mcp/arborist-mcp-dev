use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use super::{c_is_callable_declaration, c_semantic_path};

pub(crate) fn c_symbol_nodes<'tree>(
    path: &Path,
    root: Node<'tree>,
    source: &str,
) -> Result<Vec<Node<'tree>>> {
    let mut symbols = Vec::new();
    collect_c_scope_symbols(root, &mut symbols);
    if !symbols
        .iter()
        .any(|node| node.kind() == "using_declaration")
    {
        return Ok(symbols);
    }

    let mut deduplicated = Vec::new();
    for node in symbols {
        if node.kind() != "using_declaration" {
            deduplicated.push(node);
            continue;
        }
        if c_semantic_path(path, node, source)?.is_none() {
            continue;
        }
        deduplicated.push(node);
    }

    Ok(deduplicated)
}
fn collect_c_scope_symbols<'tree>(scope: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    if scope.kind() == "linkage_specification" {
        let Some(body) = scope.child_by_field_name("body") else {
            return;
        };
        if body.kind() == "declaration_list" {
            collect_c_scope_symbols(body, symbols);
        } else {
            collect_c_scope_child(body, symbols);
        }
        return;
    }

    let scope = if scope.kind() == "namespace_definition" {
        match scope.child_by_field_name("body") {
            Some(body) => body,
            None => return,
        }
    } else {
        scope
    };
    let mut cursor = scope.walk();

    for child in scope.named_children(&mut cursor) {
        collect_c_scope_child(child, symbols);
    }
}

fn collect_c_scope_child<'tree>(child: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    if matches!(
        child.kind(),
        "linkage_specification" | "namespace_definition"
    ) {
        collect_c_scope_symbols(child, symbols);
    } else if is_c_preprocessor_conditional(child) {
        collect_c_preprocessor_symbols(child, symbols);
    } else if child.kind() == "template_declaration" {
        collect_cpp_template_symbols(child, symbols);
    } else if is_cpp_type_scope(child) {
        collect_cpp_type_scope_symbols(child, symbols);
    } else if child.kind() == "enum_specifier" {
        collect_c_enum_symbols(child, symbols);
    } else if child.kind() == "declaration" {
        collect_c_named_type_definition_symbols(child, symbols);
        if is_c_symbol_node(child) {
            symbols.push(child);
        }
    } else if is_c_symbol_node(child) {
        symbols.push(child);
    }
}

fn is_c_preprocessor_conditional(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "preproc_else" | "preproc_elif" | "preproc_elifdef" | "preproc_if" | "preproc_ifdef"
    )
}

fn collect_c_preprocessor_symbols<'tree>(conditional: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    let mut cursor = conditional.walk();
    for child in conditional.named_children(&mut cursor) {
        collect_c_scope_child(child, symbols);
    }
}

pub(super) fn is_cpp_type_scope(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class_specifier" | "struct_specifier" | "union_specifier"
    )
}

fn collect_c_named_type_definition_symbols<'tree>(
    declaration: Node<'tree>,
    symbols: &mut Vec<Node<'tree>>,
) {
    let mut cursor = declaration.walk();
    for child in declaration.named_children(&mut cursor) {
        if is_cpp_type_scope(child) && child.child_by_field_name("body").is_some() {
            collect_cpp_type_scope_symbols(child, symbols);
        } else if child.kind() == "enum_specifier" {
            collect_c_enum_symbols(child, symbols);
        }
    }
}

fn collect_c_enum_symbols<'tree>(enum_node: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    symbols.push(enum_node);

    let Some(body) = enum_node.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        if child.kind() == "enumerator" {
            symbols.push(child);
        }
    }
}

fn collect_cpp_type_scope_symbols<'tree>(type_node: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    symbols.push(type_node);

    let Some(body) = type_node.child_by_field_name("body") else {
        return;
    };
    let mut cursor = body.walk();

    for child in body.named_children(&mut cursor) {
        collect_cpp_type_scope_child(child, symbols);
    }
}

fn collect_cpp_type_scope_child<'tree>(child: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    if is_c_preprocessor_conditional(child) {
        let mut cursor = child.walk();
        for nested_child in child.named_children(&mut cursor) {
            collect_cpp_type_scope_child(nested_child, symbols);
        }
    } else if child.kind() == "friend_declaration" {
        collect_cpp_friend_function_symbols(child, symbols);
    } else if is_cpp_type_scope(child) {
        collect_cpp_type_scope_symbols(child, symbols);
    } else if child.kind() == "field_declaration" {
        collect_cpp_nested_type_symbols(child, symbols);
        if c_is_callable_declaration(child) {
            symbols.push(child);
        }
    } else if child.kind() == "template_declaration" {
        collect_cpp_template_symbols(child, symbols);
    } else if child.kind() == "enum_specifier" {
        collect_c_enum_symbols(child, symbols);
    } else if is_c_symbol_node(child) {
        symbols.push(child);
    }
}

fn collect_cpp_friend_function_symbols<'tree>(
    friend_declaration: Node<'tree>,
    symbols: &mut Vec<Node<'tree>>,
) {
    let mut cursor = friend_declaration.walk();
    for child in friend_declaration.named_children(&mut cursor) {
        if child.kind() == "function_definition" || c_is_callable_declaration(child) {
            symbols.push(child);
        }
    }
}

fn collect_cpp_nested_type_symbols<'tree>(
    declaration: Node<'tree>,
    symbols: &mut Vec<Node<'tree>>,
) {
    let mut cursor = declaration.walk();
    for child in declaration.named_children(&mut cursor) {
        if is_cpp_type_scope(child) {
            collect_cpp_type_scope_symbols(child, symbols);
        } else if child.kind() == "enum_specifier" {
            collect_c_enum_symbols(child, symbols);
        }
    }
}

fn collect_cpp_template_symbols<'tree>(template_node: Node<'tree>, symbols: &mut Vec<Node<'tree>>) {
    let mut cursor = template_node.walk();

    for child in template_node.named_children(&mut cursor) {
        if child.kind() == "template_declaration" {
            collect_cpp_template_symbols(child, symbols);
        } else if child.kind() == "friend_declaration" {
            collect_cpp_friend_function_symbols(child, symbols);
        } else if is_cpp_type_scope(child) {
            collect_cpp_type_scope_symbols(child, symbols);
        } else if child.kind() == "enum_specifier" {
            collect_c_enum_symbols(child, symbols);
        } else if child.kind() == "declaration" {
            collect_c_named_type_definition_symbols(child, symbols);
            if is_c_symbol_node(child) {
                symbols.push(child);
            }
        } else if is_c_symbol_node(child) {
            symbols.push(child);
        }
    }
}

fn is_c_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
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
            | "function_definition"
    ) || c_is_callable_declaration(node)
}
