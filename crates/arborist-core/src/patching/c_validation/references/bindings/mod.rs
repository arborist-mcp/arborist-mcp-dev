use anyhow::Result;
use tree_sitter::Node;

use super::type_qualifiers::*;
use super::types::CppLocalBinding;
use crate::deadline::DeadlineCheck;
use crate::language::{node_text, visit_tree, visit_tree_with_deadline};

mod auto;
mod declared;

pub(super) use auto::*;
pub(in super::super) use auto::{cpp_address_binding, cpp_named_reference_alias_receiver};
pub(super) use declared::*;

pub(in super::super) fn collect_cpp_local_bindings(
    node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<CppLocalBinding>> {
    let mut bindings = Vec::new();
    let mut callback = |candidate: Node<'_>| match candidate.kind() {
        "declaration" => {
            if let Some(binding) = cpp_local_binding(candidate, source, &bindings) {
                bindings.push(binding);
            }
        }
        "parameter_declaration" | "optional_parameter_declaration" => {
            if let Some(binding) = cpp_parameter_binding(candidate, source) {
                bindings.push(binding);
            }
        }
        "for_range_loop" => {
            if let Some(binding) = cpp_range_for_binding(candidate, source) {
                bindings.push(binding);
            }
        }
        _ => {}
    };
    match deadline {
        Some(deadline) => visit_tree_with_deadline(node, &mut callback, deadline)?,
        None => visit_tree(node, &mut callback),
    }
    Ok(bindings)
}

fn cpp_local_binding(
    declaration: Node<'_>,
    source: &str,
    local_bindings: &[CppLocalBinding],
) -> Option<CppLocalBinding> {
    let scope = cpp_local_binding_scope(declaration)?;
    cpp_object_binding(declaration, source, scope, local_bindings)
}

fn cpp_parameter_binding(parameter: Node<'_>, source: &str) -> Option<CppLocalBinding> {
    let scope = cpp_parameter_binding_scope(parameter)?;
    cpp_object_binding(parameter, source, scope, &[])
}

fn cpp_range_for_binding(loop_node: Node<'_>, source: &str) -> Option<CppLocalBinding> {
    let type_node = loop_node.child_by_field_name("type")?;
    let declarator = loop_node.child_by_field_name("declarator")?;
    let identifier = cpp_declarator_identifier(declarator)?;
    let type_prefix = cpp_range_for_type_prefix(loop_node, source)?;
    let type_suffix = source[type_node.end_byte()..identifier.start_byte()].trim();
    let (
        type_name,
        expected_error_type,
        expected_error_receiver,
        receiver,
        access,
        standard_unwrap,
    ) = cpp_binding_type(type_node, &type_prefix, type_suffix, source)?;

    Some(CppLocalBinding {
        name: node_text(identifier, source).ok()?.trim().to_string(),
        type_name,
        expected_error_type,
        expected_error_receiver,
        receiver,
        access,
        standard_unwrap,
        declaration_start: loop_node.start_byte(),
        scope_range: (loop_node.start_byte(), loop_node.end_byte()),
    })
}

fn cpp_range_for_type_prefix(loop_node: Node<'_>, source: &str) -> Option<String> {
    let mut cursor = loop_node.walk();
    let qualifiers = loop_node
        .named_children(&mut cursor)
        .filter(|child| child.kind() == "type_qualifier")
        .map(|child| node_text(child, source).ok().map(str::trim))
        .collect::<Option<Vec<_>>>()?;
    qualifiers
        .iter()
        .all(|qualifier| matches!(*qualifier, "const" | "volatile"))
        .then(|| qualifiers.join(" "))
}

fn cpp_object_binding(
    declaration: Node<'_>,
    source: &str,
    scope: Node<'_>,
    local_bindings: &[CppLocalBinding],
) -> Option<CppLocalBinding> {
    let type_node = declaration.child_by_field_name("type")?;
    let declarator = cpp_single_declarator(declaration)?;
    let identifier = cpp_declarator_identifier(declarator)?;

    let type_prefix = source[declaration.start_byte()..type_node.start_byte()].trim();
    let type_suffix = source[type_node.end_byte()..identifier.start_byte()].trim();
    let declared_type = node_text(type_node, source).ok()?.trim();
    let (
        type_name,
        expected_error_type,
        expected_error_receiver,
        receiver,
        access,
        standard_unwrap,
    ) = if cpp_auto_binding_type_is_supported(declared_type) {
        let auto_type_prefix = format!(
            "{type_prefix} {declared_type}{}",
            if cpp_declarator_suffix_has_const_qualifier(type_suffix) {
                " const"
            } else {
                ""
            }
        );
        cpp_auto_constructor_binding_type(
            declarator,
            &auto_type_prefix,
            declaration.start_byte(),
            local_bindings,
            source,
        )?
    } else if declared_type == "decltype(auto)" {
        cpp_decltype_auto_binding_type(
            declarator,
            declaration.start_byte(),
            local_bindings,
            source,
        )?
    } else {
        cpp_binding_type(type_node, type_prefix, type_suffix, source)?
    };

    Some(CppLocalBinding {
        name: node_text(identifier, source).ok()?.trim().to_string(),
        type_name,
        expected_error_type,
        expected_error_receiver,
        receiver,
        access,
        standard_unwrap,
        declaration_start: declaration.start_byte(),
        scope_range: (scope.start_byte(), scope.end_byte()),
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::bail;

    use super::collect_cpp_local_bindings;
    use crate::deadline::DeadlineCheck;
    use crate::language::parse_document;

    struct RejectTreeWalk;

    impl DeadlineCheck for RejectTreeWalk {
        fn check(&self, phase: &str) -> anyhow::Result<()> {
            if phase == "walking syntax tree" {
                bail!("deadline reached during binding walk");
            }
            Ok(())
        }
    }

    #[test]
    fn cpp_local_binding_walk_honors_deadline() {
        let source = "int sample() { int local = 1; return local; }\n";
        let path = Path::new("sample.cpp");
        let document = parse_document(path, source).expect("source should parse");
        let root = document.tree.root_node();

        let error = collect_cpp_local_bindings(root, source, Some(&RejectTreeWalk))
            .err()
            .expect("C++ local binding walks should honor the deadline");

        assert!(
            error
                .to_string()
                .contains("deadline reached during binding walk")
        );
    }
}
