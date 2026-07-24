use tree_sitter::Node;

use super::super::super::cpp_syntax::compact_cpp_expression;
use super::super::super::cpp_types::{
    cpp_pointer_target_path, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::super::cpp_wrappers::{
    cpp_standard_expected_error_type, cpp_standard_expected_target_type,
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_smart_pointer_target_type, cpp_standard_weak_pointer_target_type,
};
use super::super::type_qualifiers::*;
use super::super::types::{CppBindingType, CppMemberAccess, CppStandardUnwrap};
use crate::language::node_text;

pub(in super::super) fn cpp_binding_type(
    type_node: Node<'_>,
    type_prefix: &str,
    type_suffix: &str,
    source: &str,
) -> Option<CppBindingType> {
    if !cpp_binding_type_prefix_is_supported(type_prefix) {
        return None;
    }
    let compact_type_suffix = compact_cpp_expression(type_suffix);
    let declared_access = if cpp_pointer_declarator_suffix(&compact_type_suffix) {
        CppMemberAccess::Pointer
    } else if cpp_object_declarator_suffix(&compact_type_suffix) {
        CppMemberAccess::Object
    } else {
        return None;
    };
    let declared_type = node_text(type_node, source).ok()?.trim();
    let expected_error_type = cpp_standard_expected_error_type(declared_type).map(str::to_string);
    let standard_unwrap = cpp_standard_smart_pointer_target_type(declared_type)
        .map(|target| (target, CppStandardUnwrap::SmartPointer))
        .or_else(|| {
            (declared_access == CppMemberAccess::Object)
                .then(|| cpp_standard_weak_pointer_target_type(declared_type))
                .flatten()
                .map(|target| (target, CppStandardUnwrap::WeakPointer))
        })
        .or_else(|| {
            (declared_access == CppMemberAccess::Object)
                .then(|| cpp_standard_reference_wrapper_target_type(declared_type))
                .flatten()
                .map(|target| (target, CppStandardUnwrap::ReferenceWrapper))
        })
        .or_else(|| {
            (declared_access == CppMemberAccess::Object)
                .then(|| cpp_standard_optional_target_type(declared_type))
                .flatten()
                .map(|target| (target, CppStandardUnwrap::Optional))
        })
        .or_else(|| {
            (declared_access == CppMemberAccess::Object)
                .then(|| cpp_standard_expected_target_type(declared_type))
                .flatten()
                .map(|target| (target, CppStandardUnwrap::Expected))
        });
    let access =
        if standard_unwrap.is_some_and(|(_, unwrap)| unwrap == CppStandardUnwrap::SmartPointer) {
            CppMemberAccess::Pointer
        } else {
            declared_access
        };
    let type_qualifiers = cpp_binding_type_qualifier_prefix(type_prefix);
    let expected_error_receiver = expected_error_type.as_ref().and_then(|error_type| {
        cpp_this_receiver_for_type(&format!("{type_qualifiers} {error_type}"), Some(false))
    });
    let type_name = format!("{type_qualifiers} {} {type_suffix}", declared_type);
    let receiver = match (access, standard_unwrap) {
        (CppMemberAccess::Object, Some((target, CppStandardUnwrap::ReferenceWrapper))) => {
            cpp_this_receiver_for_type(target, Some(false))?
        }
        (
            CppMemberAccess::Object,
            Some((target, CppStandardUnwrap::Optional | CppStandardUnwrap::Expected)),
        ) => cpp_this_receiver_for_type(&format!("{type_qualifiers} {target}"), Some(false))?,
        (CppMemberAccess::Object, Some((target, CppStandardUnwrap::WeakPointer))) => {
            cpp_this_receiver_for_type(target, Some(false))?
        }
        (CppMemberAccess::Object, _) => cpp_named_binding_receiver_for_type(&type_name)?,
        (CppMemberAccess::Pointer, Some((target, _))) => {
            cpp_this_receiver_for_type(target, Some(false))?
        }
        (CppMemberAccess::Pointer, None) => cpp_pointer_binding_receiver_for_type(&type_name)?,
    };
    let type_name = match (access, standard_unwrap) {
        (
            CppMemberAccess::Object,
            Some((
                target,
                CppStandardUnwrap::ReferenceWrapper
                | CppStandardUnwrap::Optional
                | CppStandardUnwrap::Expected
                | CppStandardUnwrap::WeakPointer,
            )),
        ) => cpp_temporary_type_path(target)?,
        (CppMemberAccess::Object, _) => cpp_temporary_type_path(&type_name)?,
        (CppMemberAccess::Pointer, Some((target, _))) => cpp_temporary_type_path(target)?,
        (CppMemberAccess::Pointer, None) => cpp_pointer_target_path(&type_name)?,
    };

    Some((
        type_name,
        expected_error_type,
        expected_error_receiver,
        receiver,
        access,
        standard_unwrap.map(|(_, unwrap)| unwrap),
    ))
}

pub(in super::super) fn cpp_single_declarator(declaration: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = declaration.walk();
    let mut declarators = declaration.children_by_field_name("declarator", &mut cursor);
    let declarator = declarators.next()?;
    (declarators.next().is_none()).then_some(declarator)
}

pub(in super::super) fn cpp_declarator_identifier(declarator: Node<'_>) -> Option<Node<'_>> {
    if declarator.kind() == "identifier" {
        return Some(declarator);
    }
    if let Some(nested_declarator) = declarator.child_by_field_name("declarator") {
        return cpp_declarator_identifier(nested_declarator);
    }
    let mut cursor = declarator.walk();
    let mut identifiers = declarator
        .named_children(&mut cursor)
        .filter_map(cpp_declarator_identifier);
    let identifier = identifiers.next()?;
    (identifiers.next().is_none()).then_some(identifier)
}

pub(in super::super) fn cpp_local_binding_scope(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "compound_statement"
                | "for_statement"
                | "for_range_loop"
                | "if_statement"
                | "switch_statement"
                | "while_statement"
        ) {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

pub(in super::super) fn cpp_parameter_binding_scope(node: Node<'_>) -> Option<Node<'_>> {
    let mut current = node.parent();
    while let Some(candidate) = current {
        if candidate.kind() == "lambda_expression" {
            return Some(candidate);
        }
        if candidate.kind() == "catch_clause" {
            return Some(candidate);
        }
        if candidate.kind() == "function_definition" {
            return candidate.child_by_field_name("body");
        }
        if matches!(candidate.kind(), "declaration" | "field_declaration") {
            return None;
        }
        current = candidate.parent();
    }
    None
}
