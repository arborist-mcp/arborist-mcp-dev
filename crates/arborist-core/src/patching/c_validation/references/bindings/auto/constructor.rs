use tree_sitter::Node;

use super::super::super::super::cpp_syntax::{
    compact_cpp_expression, cpp_constructor_type_text, cpp_default_initialized_type_path,
    cpp_default_initialized_type_text, strip_cpp_outer_parentheses,
};
use super::super::super::super::cpp_types::{cpp_temporary_type_path, cpp_this_receiver_for_type};
use super::super::super::super::cpp_wrappers::cpp_standard_expected_error_type;
use super::super::super::std_get::*;
use super::super::super::type_qualifiers::*;
use super::super::super::types::{
    CppBindingType, CppLocalBinding, CppMemberAccess, CppStandardUnwrap,
};
use super::super::super::{
    cpp_expected_error_nested_arrow_member_receiver, cpp_expected_weak_pointer_lock_receiver,
    cpp_standard_optional_value_member_receiver, cpp_standard_reference_factory_binding,
    cpp_standard_sequence_data_receiver, cpp_standard_weak_pointer_lock_receiver,
    cpp_temporary_type_from_expression, cpp_visible_local_binding,
};
use super::super::declared::cpp_declarator_identifier;
use crate::language::node_text;

use super::alias::*;
use super::copy::*;

pub(in super::super::super) fn cpp_decltype_auto_binding_type(
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

pub(in super::super::super) fn cpp_auto_constructor_binding_type(
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

pub(in super::super::super) fn cpp_auto_constructor_initializer_text<'a>(
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
