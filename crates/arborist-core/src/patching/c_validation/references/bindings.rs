use tree_sitter::Node;

use super::super::cpp_syntax::{
    compact_cpp_expression, cpp_constructor_type_text, cpp_default_initialized_type_path,
    cpp_default_initialized_type_text, cpp_receiver_call_argument, cpp_typed_receiver_call,
    strip_cpp_outer_parentheses,
};
use super::super::cpp_types::{
    CppThisMemberReceiver, cpp_pointer_target_path, cpp_temporary_type_path,
    cpp_this_receiver_for_type,
};
use super::super::cpp_wrappers::{
    cpp_standard_expected_error_type, cpp_standard_expected_target_type,
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_smart_pointer_target_type, cpp_standard_weak_pointer_target_type,
};
use super::std_get::*;
use super::type_qualifiers::*;
use super::types::{CppBindingType, CppLocalBinding, CppMemberAccess, CppStandardUnwrap};
use super::{
    cpp_addressable_local_binding_name, cpp_addressable_local_object_receiver,
    cpp_expected_error_nested_arrow_member_receiver,
    cpp_expected_error_optional_arrow_member_receiver,
    cpp_expected_error_optional_dereference_receiver,
    cpp_expected_error_optional_value_member_receiver, cpp_expected_local_binding_error_receiver,
    cpp_expected_reference_wrapper_get_receiver, cpp_expected_weak_pointer_lock_receiver,
    cpp_local_binding_name_from_expression, cpp_optional_local_binding_receiver,
    cpp_smart_pointer_dereference_receiver, cpp_smart_pointer_get_receiver,
    cpp_standard_optional_dereference_receiver, cpp_standard_optional_value_member_receiver,
    cpp_standard_reference_factory_binding, cpp_standard_reference_factory_get_receiver,
    cpp_standard_sequence_data_receiver, cpp_standard_value_member_receiver,
    cpp_standard_weak_pointer_lock_receiver, cpp_standard_wrapper_get_binding,
    cpp_strip_expected_error_access, cpp_strip_optional_value_access,
    cpp_temporary_type_from_expression, cpp_visible_local_binding,
};
use crate::language::{node_text, visit_tree};

pub(in super::super) fn collect_cpp_local_bindings(
    node: Node<'_>,
    source: &str,
) -> Vec<CppLocalBinding> {
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
    visit_tree(node, &mut callback);
    bindings
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

fn cpp_decltype_auto_binding_type(
    declarator: Node<'_>,
    declaration_start: usize,
    local_bindings: &[CppLocalBinding],
    source: &str,
) -> Option<CppBindingType> {
    let initializer = declarator.child_by_field_name("value")?;
    if initializer.kind() == "initializer_list"
        && source[declarator.start_byte()..initializer.start_byte()].contains('=')
    {
        return None;
    }
    let initializer_text = cpp_auto_constructor_initializer_text(initializer, source)?;
    let expression = strip_cpp_outer_parentheses(initializer_text.trim());
    if is_cpp_identifier(expression) {
        let binding = cpp_visible_local_binding(expression, declaration_start, local_bindings)?;
        return Some((
            binding.type_name.clone(),
            binding.expected_error_type.clone(),
            binding.expected_error_receiver,
            binding.receiver,
            binding.access,
            binding.standard_unwrap,
        ));
    }
    // Prefer pure nested error peels before reference-alias handling so forms
    // such as current->error() and (*current.error()) keep optional/expected/
    // smart-pointer unwrap metadata for later nested->member() resolution.
    if let Some(binding_type) =
        cpp_auto_expected_error_copy_binding(expression, "auto", declaration_start, local_bindings)
    {
        return Some(binding_type);
    }
    // weak_ptr::lock() yields a temporary shared_ptr. Keep smart-pointer unwrap
    // metadata so later nested->member() still resolves through decltype(auto).
    if let Some((type_name, receiver)) =
        cpp_expected_weak_pointer_lock_receiver(expression, declaration_start, local_bindings)
            .or_else(|| {
                cpp_typed_standard_get_expected_value_weak_pointer_lock_receiver(
                    expression,
                    declaration_start,
                    local_bindings,
                )
            })
            .or_else(|| {
                cpp_typed_standard_get_expected_error_weak_pointer_lock_receiver(
                    expression,
                    declaration_start,
                    local_bindings,
                )
            })
            .or_else(|| {
                cpp_typed_standard_get_expected_optional_weak_pointer_lock_receiver(
                    expression,
                    declaration_start,
                    local_bindings,
                )
            })
            .or_else(|| {
                cpp_standard_weak_pointer_lock_receiver(
                    expression,
                    declaration_start,
                    local_bindings,
                )
            })
    {
        return Some((
            type_name,
            None,
            None,
            receiver,
            CppMemberAccess::Pointer,
            Some(CppStandardUnwrap::SmartPointer),
        ));
    }
    // .get() / *smart_pointer peels for decltype(auto) need the same smart-pointer
    // binding metadata that by-value auto already keeps. Keep these ahead of the
    // forced reference-alias path so *nested_optional_sp does not collapse into a
    // plain optional alias.
    if let Some(binding_type) = cpp_auto_expected_error_smart_pointer_binding(
        expression,
        "auto",
        declaration_start,
        local_bindings,
    ) {
        return Some(binding_type);
    }
    // Optional/expected value peels such as *current on nested optional wrappers
    // should keep intermediate unwrap metadata under decltype(auto) as well.
    if let Some(binding_type) =
        cpp_auto_standard_value_copy_binding(expression, "auto", declaration_start, local_bindings)
    {
        return Some(binding_type);
    }
    // Address factories preserve a pointer to the local object's concrete type
    // under decltype(auto), including std::to_address(smart_pointer).
    if let Some((type_name, receiver)) =
        cpp_address_binding(expression, declaration_start, local_bindings)
    {
        return Some((
            type_name,
            None,
            None,
            cpp_named_reference_alias_receiver(receiver),
            CppMemberAccess::Pointer,
            None,
        ));
    }
    // std::get_if<T>(...) and std::any_cast<T>(...) yield T* for decltype(auto).
    if let Some(type_name) =
        cpp_get_if_pointer_type(expression).or_else(|| cpp_any_cast_pointer_type(expression))
    {
        return Some((
            cpp_temporary_type_path(type_name)?,
            None,
            None,
            cpp_this_receiver_for_type(type_name, Some(false))?,
            CppMemberAccess::Pointer,
            None,
        ));
    }
    // std::*_pointer_cast<T>(...) yields shared_ptr<T> for decltype(auto).
    if let Some(type_name) = cpp_pointer_cast_shared_pointer_type(expression) {
        return Some((
            cpp_temporary_type_path(type_name)?,
            None,
            None,
            cpp_this_receiver_for_type(type_name, Some(false))?,
            CppMemberAccess::Pointer,
            Some(CppStandardUnwrap::SmartPointer),
        ));
    }
    // std::get<T>(...) preserves the selected element's reference and value
    // category under decltype(auto). value any_cast returns by value instead.
    if let Some((type_name, receiver)) =
        cpp_typed_standard_get_receiver(expression, declaration_start, local_bindings)
    {
        return cpp_reference_alias_binding_type(
            &type_name,
            cpp_named_reference_alias_receiver(receiver),
        );
    }
    if let Some((type_name, receiver)) = cpp_typed_standard_get_expected_value_receiver(
        expression,
        declaration_start,
        local_bindings,
    ) {
        return cpp_reference_alias_binding_type(
            &type_name,
            cpp_named_reference_alias_receiver(receiver),
        );
    }
    if let Some((type_name, receiver)) = cpp_typed_standard_get_expected_error_receiver(
        expression,
        declaration_start,
        local_bindings,
    ) {
        return cpp_reference_alias_binding_type(
            &type_name,
            cpp_named_reference_alias_receiver(receiver),
        );
    }
    if let Some((type_name, receiver)) = cpp_typed_standard_get_expected_sequence_element_receiver(
        expression,
        declaration_start,
        local_bindings,
    ) {
        return cpp_reference_alias_binding_type(
            &type_name,
            cpp_named_reference_alias_receiver(receiver),
        );
    }
    if let Some(type_name) = cpp_any_cast_value_type(expression) {
        return cpp_copied_standard_binding_type(type_name, "auto");
    }
    if let Some((type_name, receiver)) =
        cpp_indexed_tuple_get_receiver(expression, declaration_start, local_bindings)
    {
        return cpp_reference_alias_binding_type(
            &type_name,
            cpp_named_reference_alias_receiver(receiver),
        );
    }
    if let Some((type_name, receiver)) =
        cpp_standard_sequence_data_receiver(expression, declaration_start, local_bindings)
    {
        return Some((
            type_name,
            None,
            None,
            receiver,
            CppMemberAccess::Pointer,
            None,
        ));
    }
    if let Some((type_name, receiver)) = cpp_typed_standard_get_expected_sequence_data_receiver(
        expression,
        declaration_start,
        local_bindings,
    ) {
        return Some((
            type_name,
            None,
            None,
            receiver,
            CppMemberAccess::Pointer,
            None,
        ));
    }
    if let Some((type_name, receiver)) = cpp_indexed_tuple_get_expected_sequence_data_receiver(
        expression,
        declaration_start,
        local_bindings,
    ) {
        return Some((
            type_name,
            None,
            None,
            receiver,
            CppMemberAccess::Pointer,
            None,
        ));
    }
    if let Some((type_name, receiver)) = cpp_auto_reference_alias_binding(
        initializer_text,
        "auto",
        "&",
        declaration_start,
        local_bindings,
    ) {
        return cpp_reference_alias_binding_type(&type_name, receiver);
    }
    // Nested optional/expected peels such as (*current.error())->value() should still
    // bind through decltype(auto) as aliases to the peeled object.
    if let Some((type_name, receiver)) =
        cpp_auto_optional_alias_binding(expression, declaration_start, local_bindings)
    {
        return cpp_reference_alias_binding_type(&type_name, receiver);
    }
    if let Some((type_name, receiver)) = cpp_expected_error_nested_arrow_member_receiver(
        expression,
        declaration_start,
        local_bindings,
    ) {
        return cpp_reference_alias_binding_type(&type_name, receiver);
    }
    if let Some((type_name, receiver)) =
        cpp_standard_optional_value_member_receiver(expression, declaration_start, local_bindings)
    {
        return cpp_reference_alias_binding_type(&type_name, receiver);
    }
    None
}

fn cpp_auto_constructor_binding_type(
    declarator: Node<'_>,
    type_prefix: &str,
    declaration_start: usize,
    local_bindings: &[CppLocalBinding],
    source: &str,
) -> Option<CppBindingType> {
    if !cpp_binding_type_prefix_is_supported(type_prefix) {
        return None;
    }
    let initializer = declarator.child_by_field_name("value")?;
    if initializer.kind() == "initializer_list"
        && source[declarator.start_byte()..initializer.start_byte()].contains('=')
    {
        return None;
    }
    let initializer_text =
        strip_cpp_outer_parentheses(cpp_auto_constructor_initializer_text(initializer, source)?);
    let type_suffix =
        source[declarator.start_byte()..cpp_declarator_identifier(declarator)?.start_byte()].trim();
    let compact_type_suffix = compact_cpp_expression(type_suffix);
    let declared_access = if cpp_pointer_declarator_suffix(&compact_type_suffix) {
        CppMemberAccess::Pointer
    } else if cpp_object_declarator_suffix(&compact_type_suffix) {
        CppMemberAccess::Object
    } else {
        return None;
    };
    let allocation_initializer = initializer_text.strip_prefix("new").and_then(|remainder| {
        let allocation = remainder.trim_start();
        (allocation.len() < remainder.len()).then_some(allocation)
    });
    let smart_pointer_factory_type = cpp_smart_pointer_factory_type(initializer_text)
        .or_else(|| cpp_pointer_cast_shared_pointer_type(initializer_text));
    let get_if_pointer_type = cpp_get_if_pointer_type(initializer_text)
        .or_else(|| cpp_any_cast_pointer_type(initializer_text));
    let typed_standard_get_type =
        cpp_typed_standard_get_element_binding(initializer_text, declaration_start, local_bindings)
            .map(|(type_name, _)| type_name);
    let direct_initializer_type = typed_standard_get_type
        .as_deref()
        .or_else(|| cpp_any_cast_value_type(initializer_text))
        .or_else(|| cpp_constructor_type_text(initializer_text))
        .or_else(|| cpp_default_initialized_type_text(initializer_text));
    let direct_standard_unwrap = direct_initializer_type.and_then(cpp_standard_wrapper_target_type);
    let expected_error_type = direct_initializer_type
        .and_then(cpp_standard_expected_error_type)
        .map(str::to_string);
    let reference_factory_binding =
        cpp_standard_reference_factory_binding(initializer_text, declaration_start, local_bindings);
    let weak_pointer_lock_binding = cpp_standard_weak_pointer_lock_receiver(
        initializer_text,
        declaration_start,
        local_bindings,
    )
    .or_else(|| {
        cpp_expected_weak_pointer_lock_receiver(initializer_text, declaration_start, local_bindings)
    })
    .or_else(|| {
        cpp_typed_standard_get_expected_value_weak_pointer_lock_receiver(
            initializer_text,
            declaration_start,
            local_bindings,
        )
    })
    .or_else(|| {
        cpp_typed_standard_get_expected_error_weak_pointer_lock_receiver(
            initializer_text,
            declaration_start,
            local_bindings,
        )
    })
    .or_else(|| {
        cpp_typed_standard_get_expected_optional_weak_pointer_lock_receiver(
            initializer_text,
            declaration_start,
            local_bindings,
        )
    });
    let address_binding = cpp_address_binding(initializer_text, declaration_start, local_bindings);
    let reference_alias_binding = cpp_auto_reference_alias_binding(
        initializer_text,
        type_prefix,
        &compact_type_suffix,
        declaration_start,
        local_bindings,
    );
    if let Some((type_name, receiver)) = reference_alias_binding {
        return cpp_reference_alias_binding_type(&type_name, receiver);
    }
    if let Some(binding_type) = cpp_auto_expected_error_copy_binding(
        initializer_text,
        type_prefix,
        declaration_start,
        local_bindings,
    ) {
        return Some(binding_type);
    }
    if let Some(binding_type) = cpp_auto_standard_value_copy_binding(
        initializer_text,
        type_prefix,
        declaration_start,
        local_bindings,
    ) {
        return Some(binding_type);
    }
    if let Some(binding_type) = cpp_auto_expected_error_smart_pointer_binding(
        initializer_text,
        type_prefix,
        declaration_start,
        local_bindings,
    ) {
        return Some(binding_type);
    }
    if let Some((type_name, _)) = cpp_typed_standard_get_expected_value_receiver(
        initializer_text,
        declaration_start,
        local_bindings,
    ) {
        return cpp_copied_standard_binding_type(&type_name, type_prefix);
    }
    if let Some((type_name, _)) = cpp_typed_standard_get_expected_error_receiver(
        initializer_text,
        declaration_start,
        local_bindings,
    ) {
        return cpp_copied_standard_binding_type(&type_name, type_prefix);
    }
    if let Some((type_name, _)) = cpp_typed_standard_get_expected_sequence_element_receiver(
        initializer_text,
        declaration_start,
        local_bindings,
    ) {
        return cpp_copied_standard_binding_type(&type_name, type_prefix);
    }
    if let Some(binding_type) = cpp_auto_reference_wrapper_get_copy_binding(
        initializer_text,
        type_prefix,
        declaration_start,
        local_bindings,
    ) {
        return Some(binding_type);
    }
    if let Some((type_name, _)) =
        cpp_indexed_tuple_get_receiver(initializer_text, declaration_start, local_bindings)
    {
        // auto copies std::get<N>(tuple-like) results and drops top-level const.
        return cpp_copied_standard_binding_type(&type_name, type_prefix);
    }
    if let Some((type_name, receiver)) =
        cpp_standard_sequence_data_receiver(initializer_text, declaration_start, local_bindings)
    {
        return Some((
            type_name,
            None,
            None,
            receiver,
            CppMemberAccess::Pointer,
            None,
        ));
    }
    if let Some((type_name, receiver)) = cpp_typed_standard_get_expected_sequence_data_receiver(
        initializer_text,
        declaration_start,
        local_bindings,
    ) {
        return Some((
            type_name,
            None,
            None,
            receiver,
            CppMemberAccess::Pointer,
            None,
        ));
    }
    if let Some((type_name, receiver)) = cpp_indexed_tuple_get_expected_sequence_data_receiver(
        initializer_text,
        declaration_start,
        local_bindings,
    ) {
        return Some((
            type_name,
            None,
            None,
            receiver,
            CppMemberAccess::Pointer,
            None,
        ));
    }
    let inferred_pointer_type = allocation_initializer
        .and_then(|allocation| {
            cpp_constructor_type_text(allocation)
                .or_else(|| cpp_default_initialized_type_text(allocation))
        })
        .or(smart_pointer_factory_type)
        .or(get_if_pointer_type)
        .or_else(|| {
            direct_standard_unwrap.and_then(|(target, unwrap)| {
                (unwrap == CppStandardUnwrap::SmartPointer).then_some(target)
            })
        });
    let access = match (
        declared_access,
        inferred_pointer_type,
        weak_pointer_lock_binding.as_ref(),
        address_binding.as_ref(),
    ) {
        (CppMemberAccess::Pointer, _, _, _)
        | (CppMemberAccess::Object, Some(_), _, _)
        | (CppMemberAccess::Object, None, Some(_), _) => CppMemberAccess::Pointer,
        (CppMemberAccess::Object, None, None, Some(_)) => CppMemberAccess::Pointer,
        (CppMemberAccess::Object, None, None, None) => CppMemberAccess::Object,
    };
    let initializer_type = if access == CppMemberAccess::Pointer {
        inferred_pointer_type
    } else {
        direct_initializer_type
    };
    let type_name = reference_factory_binding
        .as_ref()
        .map(|(type_name, _)| type_name.clone())
        .or_else(|| {
            weak_pointer_lock_binding
                .as_ref()
                .map(|(type_name, _)| type_name.clone())
        })
        .or_else(|| {
            address_binding
                .as_ref()
                .map(|(type_name, _)| type_name.clone())
        })
        .or_else(|| initializer_type.and_then(cpp_temporary_type_path))
        .or_else(|| {
            cpp_temporary_type_from_expression(initializer_text).map(|(type_name, _)| type_name)
        })
        .or_else(|| cpp_default_initialized_type_path(initializer_text))?;
    let type_qualifiers =
        if access == CppMemberAccess::Pointer && declared_access == CppMemberAccess::Object {
            String::new()
        } else {
            cpp_binding_type_qualifier_prefix(type_prefix)
        };
    let standard_unwrap = direct_standard_unwrap.or_else(|| {
        smart_pointer_factory_type.map(|target| (target, CppStandardUnwrap::SmartPointer))
    });
    let standard_unwrap_kind = standard_unwrap
        .map(|(_, unwrap)| unwrap)
        .or_else(|| {
            reference_factory_binding
                .as_ref()
                .map(|_| CppStandardUnwrap::ReferenceWrapper)
        })
        .or_else(|| {
            weak_pointer_lock_binding
                .as_ref()
                .map(|_| CppStandardUnwrap::SmartPointer)
        });
    let expected_error_receiver = (standard_unwrap_kind == Some(CppStandardUnwrap::Expected))
        .then(|| {
            expected_error_type.as_ref().and_then(|error_type| {
                cpp_this_receiver_for_type(&format!("{type_qualifiers} {error_type}"), Some(false))
            })
        })
        .flatten();
    let receiver = if let Some((_, receiver)) = reference_factory_binding.as_ref() {
        *receiver
    } else if let Some((_, receiver)) = weak_pointer_lock_binding.as_ref() {
        *receiver
    } else if let Some((_, receiver)) = address_binding.as_ref() {
        *receiver
    } else {
        match (access, standard_unwrap) {
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
            (CppMemberAccess::Pointer, Some((target, CppStandardUnwrap::SmartPointer))) => {
                cpp_this_receiver_for_type(target, Some(false))?
            }
            _ => {
                let receiver_type = initializer_type.unwrap_or(&type_name);
                cpp_this_receiver_for_type(
                    &format!("{type_qualifiers} {receiver_type}"),
                    Some(false),
                )?
            }
        }
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
        )
        | (CppMemberAccess::Pointer, Some((target, CppStandardUnwrap::SmartPointer))) => {
            cpp_temporary_type_path(target)?
        }
        _ => type_name,
    };

    Some((
        type_name,
        expected_error_type,
        expected_error_receiver,
        receiver,
        access,
        standard_unwrap_kind,
    ))
}

pub(in super::super) fn cpp_address_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::to_address") {
        return cpp_to_address_binding(argument, byte_offset, local_bindings);
    }
    let argument = cpp_receiver_call_argument(expression, "std::addressof")
        .or_else(|| expression.strip_prefix('&').map(str::trim))?;
    cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings)
}

fn cpp_to_address_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let binding_name = cpp_local_binding_name_from_expression(expression)?;
    let binding = cpp_visible_local_binding(binding_name, byte_offset, local_bindings)?;
    if binding.access == CppMemberAccess::Pointer {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if binding.standard_unwrap == Some(CppStandardUnwrap::SmartPointer) {
        let target = cpp_standard_smart_pointer_target_type(&binding.type_name)?;
        return Some((
            cpp_temporary_type_path(target)?,
            cpp_this_receiver_for_type(target, Some(false))?,
        ));
    }
    None
}

fn cpp_auto_reference_alias_binding(
    expression: &str,
    type_prefix: &str,
    type_suffix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    if !matches!(cpp_strip_cv_qualifiers(type_suffix), "&" | "&&") {
        return None;
    }
    if let Some((type_name, receiver)) =
        cpp_smart_pointer_dereference_receiver(expression, byte_offset, local_bindings)
    {
        let receiver = if cpp_auto_reference_alias_is_const(type_prefix, type_suffix) {
            CppThisMemberReceiver::ConstLvalue
        } else {
            cpp_named_reference_alias_receiver(receiver)
        };
        return Some((type_name, receiver));
    }
    if let Some((type_name, receiver)) =
        cpp_auto_reference_wrapper_get_alias_binding(expression, byte_offset, local_bindings)
    {
        let receiver = if cpp_auto_reference_alias_is_const(type_prefix, type_suffix) {
            CppThisMemberReceiver::ConstLvalue
        } else {
            cpp_named_reference_alias_receiver(receiver)
        };
        return Some((type_name, receiver));
    }
    if let Some((type_name, receiver)) =
        cpp_auto_optional_alias_binding(expression, byte_offset, local_bindings)
    {
        let receiver = if cpp_auto_reference_alias_is_const(type_prefix, type_suffix) {
            CppThisMemberReceiver::ConstLvalue
        } else {
            cpp_named_reference_alias_receiver(receiver)
        };
        return Some((type_name, receiver));
    }
    let (type_name, binding, force_const, dereferenced_pointer) =
        cpp_auto_reference_alias_target_binding(expression, byte_offset, local_bindings)?;
    if (binding.standard_unwrap.is_some()
        && !(dereferenced_pointer
            && binding.standard_unwrap == Some(CppStandardUnwrap::SmartPointer)))
        || !(binding.access == CppMemberAccess::Object || dereferenced_pointer)
    {
        return None;
    }
    let receiver = if force_const || cpp_auto_reference_alias_is_const(type_prefix, type_suffix) {
        CppThisMemberReceiver::ConstLvalue
    } else {
        cpp_named_reference_alias_receiver(binding.receiver)
    };
    Some((type_name, receiver))
}

fn cpp_reference_alias_binding_type(
    type_name: &str,
    receiver: CppThisMemberReceiver,
) -> Option<CppBindingType> {
    if let Some(target) = cpp_standard_smart_pointer_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Pointer,
            Some(CppStandardUnwrap::SmartPointer),
        ));
    }
    if let Some(target) = cpp_standard_optional_target_type(type_name) {
        let receiver = match receiver {
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                CppThisMemberReceiver::ConstLvalue
            }
            CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                cpp_this_receiver_for_type(target, Some(false))?
            }
        };
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            receiver,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::Optional),
        ));
    }
    if let Some(target) = cpp_standard_reference_wrapper_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::ReferenceWrapper),
        ));
    }
    if let Some(target) = cpp_standard_weak_pointer_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::WeakPointer),
        ));
    }
    Some((
        type_name.to_string(),
        None,
        None,
        receiver,
        CppMemberAccess::Object,
        None,
    ))
}

fn cpp_auto_expected_error_copy_binding(
    expression: &str,
    type_prefix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<CppBindingType> {
    // Support both .error() and nested peels such as current->error() where the
    // optional around expected is first unwrapped by operator->.
    // For binding copies, peel only the layers made explicit by the initializer
    // so decltype(auto) nested = (*current.error()) keeps the expected unwrap
    // needed for later nested->member().
    let (type_name, _) = if let Some(receiver) = cpp_strip_expected_error_access(expression) {
        cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?
    } else if let Some(receiver) = expression.strip_prefix('*').map(str::trim) {
        let receiver = strip_cpp_outer_parentheses(receiver);
        let error_receiver = cpp_strip_expected_error_access(receiver)?;
        let (type_name, error_receiver) =
            cpp_expected_local_binding_error_receiver(error_receiver, byte_offset, local_bindings)?;
        // *current.error() on a smart-pointer error peels the pointee. Do not keep
        // SmartPointer unwrap metadata; by-value auto should bind a plain object.
        if let Some(target) = cpp_standard_smart_pointer_target_type(&type_name) {
            return cpp_copied_standard_binding_type(target, type_prefix);
        }
        // Otherwise peel one optional/expected layer for forms such as
        // decltype(auto) nested = (*current.error()).
        cpp_standard_value_member_receiver(&type_name, error_receiver, true)
            .or(Some((type_name, error_receiver)))?
    } else if let Some((type_name, receiver)) =
        cpp_expected_error_nested_arrow_member_receiver(expression, byte_offset, local_bindings)
    {
        (type_name, receiver)
    } else if let Some((type_name, receiver)) =
        cpp_expected_error_optional_arrow_member_receiver(expression, byte_offset, local_bindings)
    {
        (type_name, receiver)
    } else {
        return None;
    };
    cpp_copied_standard_binding_type(&type_name, type_prefix)
}

fn cpp_auto_standard_value_copy_binding(
    expression: &str,
    type_prefix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<CppBindingType> {
    // Binding copies must preserve intermediate wrappers. Local optional/expected
    // bindings already store one unwrapped layer, so plain `.value()` should not
    // peel again. `->value()` still needs both the operator-> peel and the value
    // peel, for example (*optional<optional<expected<T>>> )->value().
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let (type_name, _) = if let Some(receiver) = cpp_strip_optional_value_access(expression) {
        let used_arrow = expression.ends_with("->value()");
        let (type_name, receiver) =
            cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings).or_else(
                || {
                    cpp_expected_error_optional_value_member_receiver(
                        expression,
                        byte_offset,
                        local_bindings,
                    )
                },
            )?;
        if used_arrow {
            let (type_name, receiver) =
                match cpp_standard_value_member_receiver(&type_name, receiver, true) {
                    Some(peeled) => peeled,
                    None => (type_name, receiver),
                };
            cpp_standard_value_member_receiver(&type_name, receiver, true)
                .or(Some((type_name, receiver)))?
        } else {
            (type_name, receiver)
        }
    } else if let Some((type_name, receiver)) =
        cpp_standard_optional_dereference_receiver(expression, byte_offset, local_bindings)
    {
        (type_name, receiver)
    } else if let Some((type_name, receiver)) =
        cpp_expected_error_optional_dereference_receiver(expression, byte_offset, local_bindings)
    {
        (type_name, receiver)
    } else {
        return None;
    };
    cpp_copied_standard_binding_type(&type_name, type_prefix)
}

fn cpp_auto_expected_error_smart_pointer_binding(
    expression: &str,
    type_prefix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<CppBindingType> {
    if let Some((type_name, _)) =
        cpp_smart_pointer_dereference_receiver(expression, byte_offset, local_bindings)
    {
        // By-value auto copy of the pointee drops top-level const.
        return cpp_copied_standard_binding_type(&type_name, type_prefix);
    }
    let (type_name, receiver) =
        cpp_smart_pointer_get_receiver(expression, byte_offset, local_bindings).or_else(|| {
            cpp_typed_standard_get_expected_optional_smart_pointer_get_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
        })?;
    // .get() yields a pointer; pointee constness is preserved under auto.
    let _ = type_prefix;
    Some((
        type_name,
        None,
        None,
        receiver,
        CppMemberAccess::Pointer,
        Some(CppStandardUnwrap::SmartPointer),
    ))
}

fn cpp_auto_reference_wrapper_get_copy_binding(
    expression: &str,
    type_prefix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<CppBindingType> {
    let (type_name, _) =
        cpp_auto_reference_wrapper_get_alias_binding(expression, byte_offset, local_bindings)?;
    // By-value auto copy of the referenced object drops top-level const.
    cpp_copied_standard_binding_type(&type_name, type_prefix)
}

fn cpp_copied_standard_binding_type(type_name: &str, type_prefix: &str) -> Option<CppBindingType> {
    let type_name = cpp_strip_leading_cv_qualifiers(type_name);
    let type_qualifiers = cpp_binding_type_qualifier_prefix(type_prefix);
    // Prefer concrete wrapper targets first so auto copies of nested wrappers such as
    // optional<unique_ptr<T>> keep usable unwrap metadata.
    if let Some(target) = cpp_standard_smart_pointer_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Pointer,
            Some(CppStandardUnwrap::SmartPointer),
        ));
    }
    if let Some(target) = cpp_standard_reference_wrapper_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::ReferenceWrapper),
        ));
    }
    if let Some(target) = cpp_standard_weak_pointer_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::WeakPointer),
        ));
    }
    if let Some(target) = cpp_standard_optional_target_type(type_name) {
        // Recurse only into nested wrappers such as optional<unique_ptr<T>>.
        // Plain optional<T> must keep Optional unwrap metadata so later
        // error->member() / nested->member() still resolve.
        if let Some(inner) = cpp_copied_standard_binding_type(target, type_prefix)
            && inner.5.is_some()
        {
            return Some(inner);
        }
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(&format!("{type_qualifiers} {target}"), Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::Optional),
        ));
    }
    if let Some(target) = cpp_standard_expected_target_type(type_name) {
        let expected_error_type = cpp_standard_expected_error_type(type_name)?.to_string();
        if let Some(inner) = cpp_copied_standard_binding_type(target, type_prefix) {
            if inner.5.is_none() {
                return Some((
                    inner.0,
                    Some(expected_error_type.clone()),
                    cpp_this_receiver_for_type(
                        &format!("{type_qualifiers} {expected_error_type}"),
                        Some(false),
                    ),
                    inner.3,
                    inner.4,
                    Some(CppStandardUnwrap::Expected),
                ));
            }
            return Some(inner);
        }
        return Some((
            cpp_temporary_type_path(target)?,
            Some(expected_error_type.clone()),
            cpp_this_receiver_for_type(
                &format!("{type_qualifiers} {expected_error_type}"),
                Some(false),
            ),
            cpp_this_receiver_for_type(&format!("{type_qualifiers} {target}"), Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::Expected),
        ));
    }
    Some((
        cpp_temporary_type_path(type_name)?,
        None,
        None,
        cpp_this_receiver_for_type(&format!("{type_qualifiers} {type_name}"), Some(false))?,
        CppMemberAccess::Object,
        None,
    ))
}

fn cpp_auto_optional_alias_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_auto_optional_alias_binding(argument, byte_offset, local_bindings);
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_auto_optional_alias_binding(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let _ = cpp_auto_optional_alias_binding(argument, byte_offset, local_bindings)?;
        return Some((
            cpp_temporary_type_path(type_name)?,
            cpp_this_receiver_for_type(type_name, Some(true))?,
        ));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "static_cast") {
        let _ = cpp_auto_optional_alias_binding(argument, byte_offset, local_bindings)?;
        return Some((
            cpp_temporary_type_path(type_name)?,
            cpp_this_receiver_for_type(type_name, None)?,
        ));
    }
    cpp_standard_optional_value_member_receiver(expression, byte_offset, local_bindings)
        .or_else(|| {
            let receiver = cpp_strip_expected_error_access(expression)?;
            cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)
        })
        .or_else(|| {
            cpp_expected_error_optional_value_member_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
        })
        .or_else(|| {
            cpp_standard_optional_dereference_receiver(expression, byte_offset, local_bindings)
        })
        .or_else(|| {
            cpp_expected_error_optional_dereference_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
        })
}

fn cpp_auto_reference_wrapper_get_alias_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    if let Some(binding) =
        cpp_standard_reference_factory_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some(binding);
    }
    if let Some(binding) =
        cpp_expected_reference_wrapper_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some(binding);
    }
    if let Some(binding) = cpp_typed_standard_get_expected_value_reference_wrapper_receiver(
        expression,
        byte_offset,
        local_bindings,
    ) {
        return Some(binding);
    }
    if let Some(binding) = cpp_typed_standard_get_expected_error_reference_wrapper_receiver(
        expression,
        byte_offset,
        local_bindings,
    ) {
        return Some(binding);
    }
    if let Some(binding) = cpp_typed_standard_get_expected_optional_reference_wrapper_receiver(
        expression,
        byte_offset,
        local_bindings,
    ) {
        return Some(binding);
    }
    let (binding, _) = cpp_standard_wrapper_get_binding(expression, byte_offset, local_bindings)?;
    (binding.access == CppMemberAccess::Object
        && binding.standard_unwrap == Some(CppStandardUnwrap::ReferenceWrapper))
    .then(|| (binding.type_name.clone(), binding.receiver))
}

pub(in super::super) fn cpp_named_reference_alias_receiver(
    receiver: CppThisMemberReceiver,
) -> CppThisMemberReceiver {
    match receiver {
        CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
            CppThisMemberReceiver::Lvalue
        }
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
    }
}

fn cpp_auto_reference_alias_target_binding<'a>(
    expression: &str,
    byte_offset: usize,
    local_bindings: &'a [CppLocalBinding],
) -> Option<(String, &'a CppLocalBinding, bool, bool)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        let (type_name, binding, _, dereferenced_pointer) =
            cpp_auto_reference_alias_target_binding(argument, byte_offset, local_bindings)?;
        return Some((type_name, binding, true, dereferenced_pointer));
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_auto_reference_alias_target_binding(argument, byte_offset, local_bindings);
    }
    if let Some((forwarded_type, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let (_, binding, _, dereferenced_pointer) =
            cpp_auto_reference_alias_target_binding(argument, byte_offset, local_bindings)?;
        let receiver = cpp_this_receiver_for_type(forwarded_type, Some(true))?;
        let force_const = matches!(
            receiver,
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
        );
        return Some((
            cpp_temporary_type_path(forwarded_type)?,
            binding,
            force_const,
            dereferenced_pointer,
        ));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "static_cast") {
        let (_, binding, _, dereferenced_pointer) =
            cpp_auto_reference_alias_target_binding(argument, byte_offset, local_bindings)?;
        let receiver = cpp_this_receiver_for_type(type_name, None)?;
        let force_const = matches!(
            receiver,
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
        );
        return Some((
            cpp_temporary_type_path(type_name)?,
            binding,
            force_const,
            dereferenced_pointer,
        ));
    }
    if let Some(pointer_name) = expression.strip_prefix('*').map(str::trim)
        && let Some(binding) = cpp_visible_local_binding(pointer_name, byte_offset, local_bindings)
        && binding.access == CppMemberAccess::Pointer
        && matches!(
            binding.standard_unwrap,
            None | Some(CppStandardUnwrap::SmartPointer)
        )
    {
        return Some((binding.type_name.clone(), binding, false, true));
    }
    if let Some(pointer_expression) = expression.strip_prefix('*').map(str::trim)
        && let Some((type_name, receiver)) =
            cpp_address_binding(pointer_expression, byte_offset, local_bindings)
        && let Some(binding) = cpp_visible_local_binding(
            cpp_addressable_local_binding_name(pointer_expression)?,
            byte_offset,
            local_bindings,
        )
    {
        let force_const = matches!(
            receiver,
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
        );
        return Some((type_name, binding, force_const, true));
    }
    cpp_visible_local_binding(expression, byte_offset, local_bindings)
        .map(|binding| (binding.type_name.clone(), binding, false, false))
}

fn cpp_auto_reference_alias_is_const(type_prefix: &str, type_suffix: &str) -> bool {
    type_prefix.split_whitespace().any(|part| part == "const")
        || cpp_declarator_suffix_has_const_qualifier(type_suffix)
}

fn cpp_declarator_suffix_has_const_qualifier(mut type_suffix: &str) -> bool {
    loop {
        if type_suffix.strip_prefix("const").is_some() {
            return true;
        }
        if let Some(remaining) = type_suffix.strip_prefix("volatile") {
            type_suffix = remaining;
        } else {
            return false;
        }
    }
}

fn cpp_auto_constructor_initializer_text<'a>(
    initializer: Node<'_>,
    source: &'a str,
) -> Option<&'a str> {
    if initializer.kind() != "initializer_list" {
        return node_text(initializer, source).ok().map(str::trim);
    }
    let mut cursor = initializer.walk();
    let mut values = initializer.named_children(&mut cursor);
    let value = values.next()?;
    (values.next().is_none()).then(|| node_text(value, source).ok().map(str::trim))?
}

fn cpp_binding_type(
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

fn cpp_single_declarator(declaration: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = declaration.walk();
    let mut declarators = declaration.children_by_field_name("declarator", &mut cursor);
    let declarator = declarators.next()?;
    (declarators.next().is_none()).then_some(declarator)
}

fn cpp_declarator_identifier(declarator: Node<'_>) -> Option<Node<'_>> {
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

fn cpp_local_binding_scope(node: Node<'_>) -> Option<Node<'_>> {
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

fn cpp_parameter_binding_scope(node: Node<'_>) -> Option<Node<'_>> {
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
