use anyhow::Result;
use tree_sitter::Node;

use super::super::cpp_syntax::{
    compact_cpp_expression, cpp_constructor_type_text, cpp_receiver_call_argument,
    cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::cpp_wrappers::{
    cpp_standard_contiguous_sequence_element_type, cpp_standard_expected_error_type,
    cpp_standard_expected_target_type, cpp_standard_indexable_sequence_element_type,
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_sequence_element_type, cpp_standard_smart_pointer_target_type,
    cpp_standard_weak_pointer_target_type,
};
use super::bindings::{cpp_address_binding, cpp_named_reference_alias_receiver};
use super::std_get::*;
use super::type_qualifiers::*;
use super::types::{CppLocalBinding, CppMemberAccess, CppStandardUnwrap};
use crate::language::node_text;

pub(in super::super) fn cpp_local_member_receiver_type(
    argument: Node<'_>,
    source: &str,
    local_bindings: &[CppLocalBinding],
    member_operator: &str,
) -> Result<Option<(String, CppThisMemberReceiver)>> {
    let receiver = strip_cpp_outer_parentheses(node_text(argument, source)?.trim());
    Ok(cpp_local_member_receiver_from_expression(
        receiver,
        argument.start_byte(),
        local_bindings,
        member_operator,
    ))
}

pub(super) fn cpp_local_member_receiver_from_expression(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
    member_operator: &str,
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_standard_reference_factory_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_optional_smart_pointer_get_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_typed_standard_get_smart_pointer_get_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_optional_smart_pointer_get_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_value_smart_pointer_get_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_error_smart_pointer_get_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_optional_smart_pointer_arrow_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_smart_pointer_arrow_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_optional_smart_pointer_arrow_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_optional_arrow_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_arrow_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_smart_pointer_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_raw_pointer_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_get_if_direct_pointer_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_any_cast_direct_pointer_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_typed_standard_get_weak_pointer_lock_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_value_weak_pointer_lock_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_error_weak_pointer_lock_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_optional_weak_pointer_lock_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_optional_smart_pointer_arrow_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_optional_raw_pointer_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_standard_weak_pointer_lock_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_indexed_tuple_get_weak_pointer_lock_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_value_weak_pointer_lock_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_error_weak_pointer_lock_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_optional_weak_pointer_lock_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_expected_weak_pointer_lock_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_typed_standard_get_expected_sequence_data_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_indexed_tuple_get_expected_sequence_data_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_optional_reference_wrapper_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_value_reference_wrapper_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_error_reference_wrapper_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_optional_reference_wrapper_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_value_reference_wrapper_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_error_reference_wrapper_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if let Some((binding, _)) =
        cpp_standard_wrapper_get_binding(expression, byte_offset, local_bindings)
        && matches!(
            (member_operator, binding.standard_unwrap),
            ("->", Some(CppStandardUnwrap::SmartPointer))
                | (".", Some(CppStandardUnwrap::ReferenceWrapper))
        )
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_value_smart_pointer_get_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_smart_pointer_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_standard_sequence_data_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_standard_optional_value_member_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_optional_value_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_optional_value_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_value_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_value_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_error_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_error_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) = cpp_indexed_tuple_get_reference_wrapper_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) = cpp_typed_standard_get_reference_wrapper_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_typed_standard_get_expected_sequence_element_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_sequence_element_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_standard_sequence_element_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) = cpp_standard_indexable_sequence_element_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_standard_expected_error_member_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_smart_pointer_dereference_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_expected_reference_wrapper_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) = cpp_expected_error_optional_value_member_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some(receiver) = expression.strip_prefix('*').map(str::trim)
    {
        // (*pointer)->member peels one pointer layer first. If the pointee is a
        // smart pointer (for example unique_ptr* from std::get_if), continue
        // through the smart-pointer arrow.
        let receiver = strip_cpp_outer_parentheses(receiver);
        if let Some(binding_name) = cpp_local_binding_name_from_expression(receiver)
            && let Some(binding) =
                cpp_visible_local_binding(binding_name, byte_offset, local_bindings)
            && binding.access == CppMemberAccess::Pointer
        {
            if let Some(target) = cpp_standard_smart_pointer_target_type(&binding.type_name) {
                return Some((
                    cpp_temporary_type_path(target)?,
                    cpp_this_receiver_for_type(target, Some(false))?,
                ));
            }
            return Some((binding.type_name.clone(), binding.receiver));
        }
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_optional_smart_pointer_arrow_member_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_optional_smart_pointer_arrow_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_optional_arrow_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_smart_pointer_arrow_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_value_smart_pointer_arrow_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_value_raw_pointer_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_arrow_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_error_smart_pointer_arrow_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_error_raw_pointer_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_expected_error_smart_pointer_get_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_standard_optional_arrow_member_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_expected_error_nested_arrow_member_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_expected_error_optional_arrow_member_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_expected_error_smart_pointer_arrow_member_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_raw_pointer_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_indexed_tuple_get_smart_pointer_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) = cpp_indexed_tuple_get_smart_pointer_get_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "->"
        && let Some(binding_name) = cpp_local_binding_name_from_expression(expression)
        && let Some(binding) = cpp_visible_local_binding(binding_name, byte_offset, local_bindings)
        && binding.access == CppMemberAccess::Pointer
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if member_operator == "->"
        && let Some((type_name, receiver)) =
            cpp_address_binding(expression, byte_offset, local_bindings)
    {
        return Some((type_name, cpp_named_reference_alias_receiver(receiver)));
    }
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings) {
        if binding.standard_unwrap.is_some() {
            return None;
        }
        let expected_operator = match binding.access {
            CppMemberAccess::Object => ".",
            CppMemberAccess::Pointer => "->",
        };
        if member_operator != expected_operator {
            return None;
        }
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) =
            cpp_standard_optional_dereference_receiver(expression, byte_offset, local_bindings)
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some((type_name, receiver)) = cpp_expected_error_optional_dereference_receiver(
            expression,
            byte_offset,
            local_bindings,
        )
    {
        return Some((type_name, receiver));
    }
    if member_operator == "."
        && let Some(pointer_name) = expression
            .strip_prefix('*')
            .map(str::trim)
            .and_then(cpp_local_binding_name_from_expression)
        && let Some(binding) = cpp_visible_local_binding(pointer_name, byte_offset, local_bindings)
        && binding.access == CppMemberAccess::Pointer
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if member_operator != "." {
        return None;
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_local_member_receiver_from_expression(
            argument,
            byte_offset,
            local_bindings,
            member_operator,
        )
        .map(|(type_name, receiver)| {
            let receiver = match receiver {
                CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                    CppThisMemberReceiver::Rvalue
                }
                CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                    CppThisMemberReceiver::ConstRvalue
                }
            };
            (type_name, receiver)
        });
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_local_member_receiver_from_expression(
            argument,
            byte_offset,
            local_bindings,
            member_operator,
        )
        .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    for function_name in ["static_cast", "std::forward"] {
        if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, function_name) {
            cpp_local_member_receiver_from_expression(
                argument,
                byte_offset,
                local_bindings,
                member_operator,
            )?;
            return Some((
                cpp_temporary_type_path(type_name)?,
                cpp_this_receiver_for_type(type_name, Some(true))?,
            ));
        }
    }
    None
}

pub(super) fn cpp_standard_sequence_element_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = [".front()", ".back()"]
        .into_iter()
        .find_map(|suffix| expression.strip_suffix(suffix))
        .or_else(|| cpp_standard_sequence_at_receiver(expression))?
        .trim();
    let (binding, container_receiver) =
        cpp_standard_get_container_binding(receiver, byte_offset, local_bindings)?;
    if binding.access != CppMemberAccess::Object || binding.standard_unwrap.is_some() {
        return None;
    }
    let element_type = cpp_standard_sequence_element_type(&binding.type_name)?;
    let type_name = cpp_temporary_type_path(element_type)?;
    let receiver = cpp_standard_get_element_receiver(element_type, container_receiver, false)?;
    Some((type_name, receiver))
}

pub(in super::super) fn cpp_standard_sequence_at_receiver(expression: &str) -> Option<&str> {
    let receiver = expression.strip_suffix(')')?.trim_end();
    let opening = receiver.rfind(".at(")?;
    let arguments = &receiver[opening + ".at(".len()..];
    (!arguments.is_empty()).then_some(receiver[..opening].trim())
}

pub(super) fn cpp_standard_indexable_sequence_element_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = cpp_subscript_receiver(expression)?;
    let (binding, container_receiver) =
        cpp_standard_get_container_binding(receiver, byte_offset, local_bindings)?;
    if binding.access != CppMemberAccess::Object || binding.standard_unwrap.is_some() {
        return None;
    }
    let element_type = cpp_standard_indexable_sequence_element_type(&binding.type_name)?;
    let receiver = cpp_standard_get_element_receiver(element_type, container_receiver, false)?;
    Some((cpp_temporary_type_path(element_type)?, receiver))
}

pub(super) fn cpp_standard_sequence_data_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".data()")?.trim();
    let (binding, container_receiver) =
        cpp_standard_get_container_binding(receiver, byte_offset, local_bindings)?;
    if binding.access != CppMemberAccess::Object || binding.standard_unwrap.is_some() {
        return None;
    }
    let element_type = cpp_standard_contiguous_sequence_element_type(&binding.type_name)?;
    let receiver = cpp_standard_get_element_receiver(element_type, container_receiver, false)?;
    Some((cpp_temporary_type_path(element_type)?, receiver))
}

pub(in super::super) fn cpp_subscript_receiver(expression: &str) -> Option<&str> {
    let expression = expression.trim();
    let closing = expression.strip_suffix(']')?;
    let mut depth = 1usize;
    for (index, character) in closing.char_indices().rev() {
        match character {
            ']' => depth += 1,
            '[' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return (!closing[index + 1..].trim().is_empty())
                        .then_some(closing[..index].trim());
                }
            }
            _ => {}
        }
    }
    None
}

pub(super) fn cpp_standard_wrapper_get_binding<'a>(
    expression: &str,
    byte_offset: usize,
    local_bindings: &'a [CppLocalBinding],
) -> Option<(&'a CppLocalBinding, CppThisMemberReceiver)> {
    // Accept both nested.get() and nested->get() for local reference_wrapper
    // bindings. Intermediate auto copies of optional<reference_wrapper<T>> peel
    // to ReferenceWrapper, and callers still use the same ->get() form as the
    // original nested chain. move/as_const/forward wrappers preserve the target
    // object's reference semantics because get() always returns T&.
    let receiver = expression
        .strip_suffix(".get()")
        .or_else(|| expression.strip_suffix("->get()"))
        .map(str::trim)?;
    cpp_standard_get_container_binding(receiver, byte_offset, local_bindings)
}

pub(super) fn cpp_standard_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression
        .strip_suffix(".lock()")
        .or_else(|| expression.strip_suffix("->lock()"))
        .map(str::trim)?;
    let (binding, _) = cpp_standard_get_container_binding(receiver, byte_offset, local_bindings)?;
    (binding.access == CppMemberAccess::Object
        && binding.standard_unwrap == Some(CppStandardUnwrap::WeakPointer))
    .then(|| (binding.type_name.clone(), binding.receiver))
}

pub(super) fn cpp_expected_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = strip_cpp_outer_parentheses(
        expression
            .strip_suffix(".lock()")
            .or_else(|| expression.strip_suffix("->lock()"))
            .map(str::trim)?,
    );
    // Prefer optional unwrap paths first so "*current.error()" is not
    // misread as a bare expected-error receiver ending in ".error()".
    let type_name = if let Some((type_name, _)) =
        cpp_optional_wrapper_type_from_expression(receiver, byte_offset, local_bindings)
    {
        type_name
    } else if let Some((type_name, _)) =
        cpp_expected_error_nested_arrow_member_receiver(receiver, byte_offset, local_bindings)
    {
        // Nested forms such as optional<expected<..., optional<weak_ptr<T>>>>
        // expose the weak_ptr through operator-> after .error().
        type_name
    } else if let Some((type_name, _)) =
        cpp_expected_error_optional_arrow_member_receiver(receiver, byte_offset, local_bindings)
    {
        type_name
    } else {
        let receiver = cpp_strip_expected_error_access(receiver)?;
        let (type_name, receiver) =
            cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?;
        let mut type_name = type_name;
        let mut wrapper_receiver = receiver;
        while let Some((next_type, next_receiver)) =
            cpp_standard_value_member_receiver(&type_name, wrapper_receiver, true)
        {
            type_name = next_type;
            wrapper_receiver = next_receiver;
        }
        type_name
    };
    let target = cpp_standard_weak_pointer_target_type(&type_name)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_standard_reference_factory_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    cpp_standard_reference_factory_binding(receiver, byte_offset, local_bindings)
}

pub(super) fn cpp_standard_reference_factory_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    for (factory, force_const) in [("std::ref", false), ("std::cref", true)] {
        let Some(argument) = cpp_receiver_call_argument(expression, factory) else {
            continue;
        };
        let (type_name, receiver) =
            cpp_reference_factory_argument_receiver(argument, byte_offset, local_bindings)?;
        let receiver = if force_const {
            CppThisMemberReceiver::ConstLvalue
        } else {
            receiver
        };
        return Some((type_name, receiver));
    }
    None
}

pub(super) fn cpp_reference_factory_argument_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let (expression, force_const) =
        if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
            (strip_cpp_outer_parentheses(argument.trim()), true)
        } else {
            (expression, false)
        };
    let binding = cpp_visible_local_binding(expression, byte_offset, local_bindings)?;
    if binding.access != CppMemberAccess::Object || binding.standard_unwrap.is_some() {
        return None;
    }
    let receiver = if force_const {
        CppThisMemberReceiver::ConstLvalue
    } else {
        binding.receiver
    };
    Some((binding.type_name.clone(), receiver))
}

pub(super) fn cpp_standard_optional_value_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let used_arrow = expression.ends_with("->value()");
    let receiver = cpp_strip_optional_value_access(expression)?;
    let (type_name, receiver) =
        cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings)?;
    // `receiver->value()` first applies operator-> (one optional/expected peel)
    // and then value() (another peel). Keep both peels so nested forms such as
    // optional<optional<expected<T>>> resolve through (*current)->value().
    let (type_name, receiver) = if used_arrow {
        // Preserve the receiver value category so moved wrappers such as
        // std::move(current)->value() still select && overloads.
        match cpp_standard_value_member_receiver(&type_name, receiver, true) {
            Some(peeled) => peeled,
            None => (type_name, receiver),
        }
    } else {
        (type_name, receiver)
    };
    // A trailing .value()/->value() can still be the expected/optional value
    // access on the unwrapped target, for example (*optional).value().
    cpp_standard_value_member_receiver(&type_name, receiver, true).or(Some((type_name, receiver)))
}

pub(super) fn cpp_strip_optional_value_access(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let receiver = expression
        .strip_suffix(".value()")
        .or_else(|| expression.strip_suffix("->value()"))
        .map(str::trim)?;
    // Reject "*expr.value()" where unary * applies to the value call.
    // "(*expr).value()" keeps parentheses and remains valid.
    if receiver.starts_with('*') {
        return None;
    }
    Some(receiver)
}

pub(super) fn cpp_standard_expected_error_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = cpp_strip_expected_error_access(expression)?;
    let (type_name, receiver) =
        cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?;
    // Nested expected/optional layers may still remain after one .error() peel,
    // for example expected<optional<expected<Value, Counter>>, Value>.error().
    let mut type_name = type_name;
    let mut receiver = receiver;
    while let Some((next_type, next_receiver)) =
        cpp_standard_value_member_receiver(&type_name, receiver, true)
    {
        type_name = next_type;
        receiver = next_receiver;
    }
    Some((cpp_temporary_type_path(&type_name)?, receiver))
}

pub(super) fn cpp_strip_expected_error_access(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    // Reject "*expr.error()" where unary * applies to the error access.
    // "(*expr).error()" keeps parentheses and remains valid.
    if expression.starts_with('*') {
        return None;
    }
    expression
        .strip_suffix(".error()")
        .or_else(|| expression.strip_suffix("->error()"))
        .map(str::trim)
}

pub(super) fn cpp_standard_optional_dereference_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = strip_cpp_outer_parentheses(expression.strip_prefix('*')?.trim());
    cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings)
}

pub(super) fn cpp_standard_optional_arrow_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (type_name, receiver) =
        cpp_optional_local_binding_receiver(expression, byte_offset, local_bindings)?;
    // Optional/expected bindings store one unwrapped layer. Keep peeling while
    // the remaining type is still optional/expected so nested forms such as
    // optional<expected<optional<T>>> resolve through operator->.
    let mut type_name = type_name;
    let mut receiver = receiver;
    while let Some((next_type, next_receiver)) =
        cpp_standard_value_member_receiver(&type_name, receiver, false)
    {
        type_name = next_type;
        receiver = next_receiver;
    }
    let receiver = match receiver {
        CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
            CppThisMemberReceiver::Lvalue
        }
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
    };
    Some((type_name, receiver))
}

pub(super) fn cpp_optional_smart_pointer_arrow_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (type_name, _) =
        cpp_optional_wrapper_type_from_expression(expression, byte_offset, local_bindings)?;
    let target = cpp_standard_smart_pointer_target_type(&type_name)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_expected_error_smart_pointer_arrow_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = cpp_strip_expected_error_access(expression)?;
    let (type_name, _) =
        cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_smart_pointer_target_type(&type_name)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = strip_cpp_outer_parentheses(
        expression
            .strip_suffix(".get()")
            .or_else(|| expression.strip_suffix("->get()"))
            .map(str::trim)?,
    );
    if let Some(binding_name) = cpp_local_binding_name_from_expression(receiver)
        && let Some(binding) = cpp_visible_local_binding(binding_name, byte_offset, local_bindings)
        && binding.standard_unwrap == Some(CppStandardUnwrap::SmartPointer)
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    let type_name = cpp_smart_pointer_wrapper_type(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_smart_pointer_target_type(&type_name)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_smart_pointer_dereference_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = strip_cpp_outer_parentheses(expression.strip_prefix('*')?.trim());
    if let Some(binding_name) = cpp_local_binding_name_from_expression(receiver)
        && let Some(binding) = cpp_visible_local_binding(binding_name, byte_offset, local_bindings)
        && binding.standard_unwrap == Some(CppStandardUnwrap::SmartPointer)
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    let type_name = cpp_smart_pointer_wrapper_type(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_smart_pointer_target_type(&type_name)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_smart_pointer_wrapper_type(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<String> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    // Prefer optional unwrap paths first so "*current.error()" is not
    // misread as a bare expected-error receiver ending in ".error()".
    if let Some((type_name, _)) =
        cpp_optional_wrapper_type_from_expression(expression, byte_offset, local_bindings)
    {
        return Some(type_name);
    }
    // Nested peels such as current->value() / current->error().value() can leave a
    // smart-pointer wrapper that later .get() still needs to bind. Only accept
    // results that are still smart-pointer wrappers; deeper peels can already be
    // the pointee type.
    if let Some((type_name, _)) =
        cpp_standard_optional_value_member_receiver(expression, byte_offset, local_bindings)
        && cpp_standard_smart_pointer_target_type(&type_name).is_some()
    {
        return Some(type_name);
    }
    if let Some((type_name, _)) =
        cpp_expected_error_optional_value_member_receiver(expression, byte_offset, local_bindings)
        && cpp_standard_smart_pointer_target_type(&type_name).is_some()
    {
        return Some(type_name);
    }
    if let Some((type_name, _)) =
        cpp_expected_error_nested_arrow_member_receiver(expression, byte_offset, local_bindings)
        && cpp_standard_smart_pointer_target_type(&type_name).is_some()
    {
        return Some(type_name);
    }
    if let Some(receiver) = cpp_strip_expected_error_access(expression) {
        let (type_name, _) =
            cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?;
        // current->error()->get() peels optional/expected layers after .error()
        // before calling .get() on the remaining smart pointer.
        let mut type_name = type_name;
        loop {
            if cpp_standard_smart_pointer_target_type(&type_name).is_some() {
                return Some(type_name);
            }
            let stripped = cpp_strip_leading_cv_qualifiers(&type_name);
            if let Some(target) = cpp_standard_optional_target_type(stripped)
                .or_else(|| cpp_standard_expected_target_type(stripped))
            {
                type_name = cpp_temporary_type_path(target)?;
                continue;
            }
            return Some(type_name);
        }
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_smart_pointer_wrapper_type(argument, byte_offset, local_bindings);
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_smart_pointer_wrapper_type(argument, byte_offset, local_bindings);
    }
    if let Some((_, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        return cpp_smart_pointer_wrapper_type(argument, byte_offset, local_bindings);
    }
    None
}

pub(super) fn cpp_expected_error_nested_arrow_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    // *(current.error()) and current.error()->... both need nested peels after the
    // error unwrap, including optional/expected layers and smart pointers.
    let (type_name, receiver) = if let Some(receiver) = expression.strip_prefix('*').map(str::trim)
    {
        let receiver = strip_cpp_outer_parentheses(receiver);
        let error_receiver = cpp_strip_expected_error_access(receiver)?;
        let (type_name, error_receiver) =
            cpp_expected_local_binding_error_receiver(error_receiver, byte_offset, local_bindings)?;
        // One dereference peels one optional/expected layer from the error type.
        cpp_standard_value_member_receiver(&type_name, error_receiver, true)
            .or(Some((type_name, error_receiver)))?
    } else {
        let receiver = cpp_strip_expected_error_access(expression)?;
        cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?
    };
    let mut type_name = type_name;
    let mut receiver = receiver;
    while let Some((next_type, next_receiver)) =
        cpp_standard_value_member_receiver(&type_name, receiver, false)
    {
        type_name = next_type;
        receiver = next_receiver;
    }
    if let Some(target) = cpp_standard_smart_pointer_target_type(&type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            cpp_this_receiver_for_type(target, Some(false))?,
        ));
    }
    // Leave weak_ptr / reference_wrapper as-is for callers such as .lock()/.get()
    // that still need the wrapper type itself. Member calls through those wrappers
    // without lock/get remain unresolved by later overload matching.
    if cpp_standard_optional_target_type(&type_name).is_some()
        || cpp_standard_expected_target_type(&type_name).is_some()
    {
        return None;
    }
    let receiver = match receiver {
        CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
            CppThisMemberReceiver::Lvalue
        }
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
    };
    Some((type_name, receiver))
}

pub(super) fn cpp_expected_error_optional_arrow_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = cpp_strip_expected_error_access(expression)?;
    let (type_name, error_receiver) =
        cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?;
    // Keep peeling nested optional/expected wrappers so forms such as
    // expected<..., optional<unique_ptr<T>>> and expected<optional<expected<...>>>
    // resolve through a single operator-> after .error().
    let mut type_name = type_name;
    let mut receiver = error_receiver;
    while let Some((next_type, next_receiver)) =
        cpp_standard_value_member_receiver(&type_name, receiver, false)
    {
        type_name = next_type;
        receiver = next_receiver;
    }
    if let Some(target) = cpp_standard_smart_pointer_target_type(&type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            cpp_this_receiver_for_type(target, Some(false))?,
        ));
    }
    if cpp_standard_optional_target_type(&type_name).is_some()
        || cpp_standard_expected_target_type(&type_name).is_some()
    {
        return None;
    }
    let receiver = match receiver {
        CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
            CppThisMemberReceiver::Lvalue
        }
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
    };
    Some((type_name, receiver))
}

pub(super) fn cpp_expected_reference_wrapper_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = strip_cpp_outer_parentheses(
        expression
            .strip_suffix(".get()")
            .or_else(|| expression.strip_suffix("->get()"))
            .map(str::trim)?,
    );
    // Prefer optional unwrap paths first so "*current.error()" is not
    // misread as a bare expected-error receiver ending in ".error()".
    let type_name = if let Some((type_name, _)) =
        cpp_optional_wrapper_type_from_expression(receiver, byte_offset, local_bindings)
    {
        type_name
    } else if let Some((type_name, _)) =
        cpp_expected_error_nested_arrow_member_receiver(receiver, byte_offset, local_bindings)
    {
        // Nested forms such as optional<expected<..., optional<reference_wrapper<T>>>>
        // expose the reference_wrapper through operator-> after .error().
        type_name
    } else if let Some((type_name, _)) =
        cpp_expected_error_optional_arrow_member_receiver(receiver, byte_offset, local_bindings)
    {
        type_name
    } else {
        let receiver = cpp_strip_expected_error_access(receiver)?;
        let (type_name, receiver) =
            cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?;
        let mut type_name = type_name;
        let mut wrapper_receiver = receiver;
        while let Some((next_type, next_receiver)) =
            cpp_standard_value_member_receiver(&type_name, wrapper_receiver, true)
        {
            type_name = next_type;
            wrapper_receiver = next_receiver;
        }
        type_name
    };
    let target = cpp_standard_reference_wrapper_target_type(&type_name)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_optional_wrapper_type_from_expression(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    cpp_standard_optional_value_member_receiver(expression, byte_offset, local_bindings)
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

pub(super) fn cpp_expected_error_optional_value_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let used_arrow = expression.ends_with("->value()");
    let receiver = expression
        .strip_suffix(".value()")
        .or_else(|| expression.strip_suffix("->value()"))
        .map(str::trim)?;
    let receiver = strip_cpp_outer_parentheses(receiver.trim());
    // Support both current.error().value() and (*current.error())->value().
    let (type_name, error_receiver) = if let Some(inner) = receiver.strip_prefix('*').map(str::trim)
    {
        let inner = strip_cpp_outer_parentheses(inner);
        let error_receiver = cpp_strip_expected_error_access(inner)?;
        let (type_name, error_receiver) =
            cpp_expected_local_binding_error_receiver(error_receiver, byte_offset, local_bindings)?;
        // The unary * peels one optional/expected layer from the error type.
        cpp_standard_value_member_receiver(&type_name, error_receiver, true)
            .or(Some((type_name, error_receiver)))?
    } else {
        let receiver = cpp_strip_expected_error_access(receiver)?;
        cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?
    };
    // Keep peeling nested optional/expected layers after the error unwrap so
    // forms such as expected<..., optional<expected<T>>> resolve through
    // .error()->value() / .error().value().
    let mut type_name = type_name;
    let mut receiver = error_receiver;
    if used_arrow
        && let Some((next_type, next_receiver)) =
            cpp_standard_value_member_receiver(&type_name, receiver, true)
    {
        type_name = next_type;
        receiver = next_receiver;
    }
    while let Some((next_type, next_receiver)) =
        cpp_standard_value_member_receiver(&type_name, receiver, true)
    {
        type_name = next_type;
        receiver = next_receiver;
    }
    Some((type_name, receiver))
}

pub(super) fn cpp_expected_error_optional_dereference_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = strip_cpp_outer_parentheses(expression.strip_prefix('*')?.trim());
    let receiver = cpp_strip_expected_error_access(receiver)?;
    let (type_name, error_receiver) =
        cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?;
    let mut type_name = type_name;
    let mut receiver = error_receiver;
    while let Some((next_type, next_receiver)) =
        cpp_standard_value_member_receiver(&type_name, receiver, true)
    {
        type_name = next_type;
        receiver = next_receiver;
    }
    Some((type_name, receiver))
}

pub(super) fn cpp_standard_value_member_receiver(
    type_name: &str,
    wrapper_receiver: CppThisMemberReceiver,
    preserves_value_category: bool,
) -> Option<(String, CppThisMemberReceiver)> {
    let target = cpp_standard_optional_target_type(type_name)
        .or_else(|| cpp_standard_expected_target_type(type_name))?;
    let target_receiver = cpp_this_receiver_for_type(target, Some(false))?;
    let const_qualified = matches!(
        wrapper_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    ) || matches!(
        target_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    );
    let rvalue = preserves_value_category
        && matches!(
            wrapper_receiver,
            CppThisMemberReceiver::Rvalue | CppThisMemberReceiver::ConstRvalue
        );
    let receiver = match (const_qualified, rvalue) {
        (false, false) => CppThisMemberReceiver::Lvalue,
        (true, false) => CppThisMemberReceiver::ConstLvalue,
        (false, true) => CppThisMemberReceiver::Rvalue,
        (true, true) => CppThisMemberReceiver::ConstRvalue,
    };
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_optional_local_binding_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && matches!(
            binding.standard_unwrap,
            Some(CppStandardUnwrap::Optional | CppStandardUnwrap::Expected)
        )
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if let Some(receiver) = expression.strip_prefix('*').map(str::trim) {
        // One dereference peels one optional/expected layer. Nested forms such as
        // **optional<optional<expected<T>>> need successive peels for each '*'.
        let (type_name, receiver) = cpp_optional_local_binding_receiver(
            strip_cpp_outer_parentheses(receiver),
            byte_offset,
            local_bindings,
        )?;
        return cpp_standard_value_member_receiver(&type_name, receiver, true)
            .or(Some((type_name, receiver)));
    }
    if let Some(receiver) = cpp_strip_optional_value_access(expression) {
        let (type_name, receiver) =
            cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings)?;
        return cpp_standard_value_member_receiver(&type_name, receiver, true);
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_optional_local_binding_receiver(argument, byte_offset, local_bindings).map(
            |(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Rvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstRvalue
                    }
                };
                (type_name, receiver)
            },
        );
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_optional_local_binding_receiver(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let (target_type, _) =
            cpp_optional_local_binding_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            target_type,
            cpp_this_receiver_for_type(type_name, Some(true))?,
        ));
    }
    None
}

pub(super) fn cpp_expected_error_type_from_wrapper(
    type_name: &str,
    wrapper_receiver: CppThisMemberReceiver,
) -> Option<(String, CppThisMemberReceiver)> {
    // Nested optional/expected layers may remain after one unwrap, for example
    // expected<optional<expected<Value, Counter>>, Value>. Peel until the error
    // type of an expected wrapper is reachable.
    let mut type_name = type_name.to_string();
    let mut wrapper_receiver = wrapper_receiver;
    loop {
        let stripped = cpp_strip_leading_cv_qualifiers(&type_name);
        if let Some(error_type) = cpp_standard_expected_error_type(stripped) {
            return Some((
                error_type.to_string(),
                cpp_expected_error_receiver(error_type, wrapper_receiver)?,
            ));
        }
        let (next_type, next_receiver) =
            cpp_standard_value_member_receiver(&type_name, wrapper_receiver, true)?;
        type_name = next_type;
        wrapper_receiver = next_receiver;
    }
}

pub(super) fn cpp_expected_local_binding_error_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    // Nested value peels may leave an expected wrapper, for example
    // current->value() on optional<expected<optional<expected<Value, T>>, E>>.
    if let Some((type_name, receiver)) =
        cpp_standard_optional_value_member_receiver(expression, byte_offset, local_bindings)
        && let Some(result) = cpp_expected_error_type_from_wrapper(&type_name, receiver)
    {
        return Some(result);
    }
    // *(expected.error()) peels one layer from the error type after .error().
    // Do not recurse through cpp_expected_local_binding_error_receiver for bare
    // *optional expressions; those are handled by optional_wrapper_only below.
    if let Some(receiver) = expression.strip_prefix('*').map(str::trim) {
        let receiver = strip_cpp_outer_parentheses(receiver);
        if let Some(error_receiver) = cpp_strip_expected_error_access(receiver) {
            let (type_name, wrapper_receiver) = cpp_expected_local_binding_error_receiver(
                error_receiver,
                byte_offset,
                local_bindings,
            )?;
            return cpp_expected_error_type_from_wrapper(&type_name, wrapper_receiver)
                .or(Some((type_name, wrapper_receiver)));
        }
    }
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && binding.standard_unwrap == Some(CppStandardUnwrap::Expected)
    {
        if let Some(result) = binding
            .expected_error_type
            .as_ref()
            .zip(binding.expected_error_receiver)
            .map(|(type_name, receiver)| (type_name.clone(), receiver))
        {
            return Some(result);
        }
        // Nested wrappers can leave an expected error type behind one more
        // optional/expected peel on the stored value type.
        return cpp_expected_error_type_from_wrapper(&binding.type_name, binding.receiver);
    }
    // optional<expected<...>>.value() / *optional peels only the optional layer so
    // the expected error type remains available.
    if let Some(receiver) = cpp_strip_optional_value_access(expression)
        && let Some((type_name, wrapper_receiver)) =
            cpp_optional_wrapper_only_receiver(receiver, byte_offset, local_bindings)
    {
        return cpp_expected_error_type_from_wrapper(&type_name, wrapper_receiver);
    }
    if let Some(receiver) = expression.strip_prefix('*').map(str::trim)
        && let Some((type_name, wrapper_receiver)) = cpp_optional_wrapper_only_receiver(
            strip_cpp_outer_parentheses(receiver),
            byte_offset,
            local_bindings,
        )
    {
        return cpp_expected_error_type_from_wrapper(&type_name, wrapper_receiver);
    }
    if let Some((type_name, wrapper_receiver)) =
        cpp_optional_wrapper_only_receiver(expression, byte_offset, local_bindings)
    {
        // Bare optional<expected<...>> receivers such as current->error().
        return cpp_expected_error_type_from_wrapper(&type_name, wrapper_receiver);
    }
    if let Some(receiver) = cpp_strip_expected_error_access(expression) {
        let (expected_type, expected_receiver) =
            cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?;
        return cpp_expected_error_type_from_wrapper(&expected_type, expected_receiver);
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_expected_local_binding_error_receiver(argument, byte_offset, local_bindings)
            .map(|(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Rvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstRvalue
                    }
                };
                (type_name, receiver)
            });
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_expected_local_binding_error_receiver(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let (target_type, _) =
            cpp_expected_local_binding_error_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            target_type,
            cpp_this_receiver_for_type(type_name, Some(true))?,
        ));
    }
    None
}

pub(super) fn cpp_optional_wrapper_only_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && binding.standard_unwrap == Some(CppStandardUnwrap::Optional)
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    // expected/optional bindings store one unwrapped layer. When the remaining
    // type is still optional, keep peeling so nested wrappers stay available for
    // later .error() / operator-> resolution.
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && matches!(
            binding.standard_unwrap,
            Some(CppStandardUnwrap::Optional | CppStandardUnwrap::Expected)
        )
        && cpp_standard_optional_target_type(cpp_strip_leading_cv_qualifiers(&binding.type_name))
            .is_some()
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if let Some(receiver) = expression.strip_prefix('*').map(str::trim) {
        let (type_name, receiver) = cpp_optional_wrapper_only_receiver(
            strip_cpp_outer_parentheses(receiver),
            byte_offset,
            local_bindings,
        )?;
        return cpp_standard_value_member_receiver(&type_name, receiver, true)
            .or(Some((type_name, receiver)));
    }
    if let Some(receiver) = cpp_strip_optional_value_access(expression) {
        let (type_name, receiver) =
            cpp_optional_wrapper_only_receiver(receiver, byte_offset, local_bindings)?;
        return cpp_standard_value_member_receiver(&type_name, receiver, true)
            .or(Some((type_name, receiver)));
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_optional_wrapper_only_receiver(argument, byte_offset, local_bindings).map(
            |(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Rvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstRvalue
                    }
                };
                (type_name, receiver)
            },
        );
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_optional_wrapper_only_receiver(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let (target_type, _) =
            cpp_optional_wrapper_only_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            target_type,
            cpp_this_receiver_for_type(type_name, Some(true))?,
        ));
    }
    None
}

pub(super) fn cpp_expected_error_receiver(
    error_type: &str,
    expected_receiver: CppThisMemberReceiver,
) -> Option<CppThisMemberReceiver> {
    let error_receiver = cpp_this_receiver_for_type(error_type, Some(false))?;
    let const_qualified = matches!(
        expected_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    ) || matches!(
        error_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    );
    let rvalue = matches!(
        expected_receiver,
        CppThisMemberReceiver::Rvalue | CppThisMemberReceiver::ConstRvalue
    );
    Some(match (const_qualified, rvalue) {
        (false, false) => CppThisMemberReceiver::Lvalue,
        (true, false) => CppThisMemberReceiver::ConstLvalue,
        (false, true) => CppThisMemberReceiver::Rvalue,
        (true, true) => CppThisMemberReceiver::ConstRvalue,
    })
}

pub(super) fn cpp_local_binding_name_from_expression(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if is_cpp_identifier(expression) {
        return Some(expression);
    }
    ["std::move", "std::as_const"]
        .into_iter()
        .find_map(|wrapper| {
            cpp_receiver_call_argument(expression, wrapper)
                .and_then(cpp_local_binding_name_from_expression)
        })
        .or_else(|| {
            cpp_typed_receiver_call(expression, "std::forward")
                .and_then(|(_, argument)| cpp_local_binding_name_from_expression(argument))
        })
}

pub(super) fn cpp_addressable_local_object_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && binding.access == CppMemberAccess::Object
        && binding.standard_unwrap.is_none()
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings).map(
            |(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Lvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstLvalue
                    }
                };
                (type_name, receiver)
            },
        );
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            cpp_temporary_type_path(type_name)?,
            cpp_addressable_receiver(cpp_this_receiver_for_type(type_name, Some(true))?),
        ));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "static_cast") {
        cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            cpp_temporary_type_path(type_name)?,
            cpp_addressable_receiver(cpp_this_receiver_for_type(type_name, None)?),
        ));
    }
    None
}

pub(super) fn cpp_addressable_receiver(receiver: CppThisMemberReceiver) -> CppThisMemberReceiver {
    match receiver {
        CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
            CppThisMemberReceiver::Lvalue
        }
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
    }
}

pub(super) fn cpp_addressable_local_binding_name(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let argument = cpp_receiver_call_argument(expression, "std::addressof")
        .or_else(|| expression.strip_prefix('&').map(str::trim))?;
    cpp_addressable_local_binding_name_from_expression(argument)
}

pub(super) fn cpp_addressable_local_binding_name_from_expression(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if is_cpp_identifier(expression) {
        return Some(expression);
    }
    for wrapper in ["std::move", "std::as_const"] {
        if let Some(argument) = cpp_receiver_call_argument(expression, wrapper) {
            return cpp_addressable_local_binding_name_from_expression(argument);
        }
    }
    for function_name in ["std::forward", "static_cast"] {
        if let Some((_, argument)) = cpp_typed_receiver_call(expression, function_name) {
            return cpp_addressable_local_binding_name_from_expression(argument);
        }
    }
    None
}

pub(in super::super) fn cpp_visible_local_binding<'a>(
    name: &str,
    byte_offset: usize,
    local_bindings: &'a [CppLocalBinding],
) -> Option<&'a CppLocalBinding> {
    if !is_cpp_identifier(name) {
        return None;
    }
    local_bindings
        .iter()
        .filter(|binding| {
            binding.name == name
                && binding.declaration_start < byte_offset
                && (binding.scope_range.0..binding.scope_range.1).contains(&byte_offset)
        })
        .min_by_key(|binding| {
            (
                binding.scope_range.1.saturating_sub(binding.scope_range.0),
                usize::MAX.saturating_sub(binding.declaration_start),
            )
        })
}

pub(in super::super) fn cpp_temporary_type_from_expression(
    expression: &str,
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = expression.trim();
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_temporary_type_from_expression(strip_cpp_outer_parentheses(argument)).map(
            |(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Rvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstRvalue
                    }
                };
                (type_name, receiver)
            },
        );
    }
    for function_name in ["static_cast", "std::forward"] {
        if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, function_name) {
            cpp_temporary_type_from_expression(strip_cpp_outer_parentheses(argument))?;
            return Some((
                cpp_temporary_type_path(type_name)?,
                cpp_this_receiver_for_type(type_name, Some(true))?,
            ));
        }
    }
    cpp_constructor_type_text(expression).map(|type_name| {
        (
            compact_cpp_expression(type_name),
            CppThisMemberReceiver::Rvalue,
        )
    })
}

pub(in super::super) fn cpp_this_receiver_from_expression(
    receiver: &str,
) -> Option<CppThisMemberReceiver> {
    let receiver = strip_cpp_outer_parentheses(receiver.trim());
    match compact_cpp_expression(receiver).as_str() {
        "this" | "*this" => return Some(CppThisMemberReceiver::Lvalue),
        _ => {}
    }

    if let Some(argument) = cpp_receiver_call_argument(receiver, "std::move") {
        return cpp_this_receiver_from_expression(argument).map(|receiver| match receiver {
            CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                CppThisMemberReceiver::Rvalue
            }
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                CppThisMemberReceiver::ConstRvalue
            }
        });
    }
    if let Some(argument) = cpp_receiver_call_argument(receiver, "std::as_const") {
        return cpp_this_receiver_from_expression(argument)
            .map(|_| CppThisMemberReceiver::ConstLvalue);
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(receiver, "static_cast") {
        cpp_this_receiver_from_expression(argument)?;
        return cpp_this_receiver_for_type(type_name, None);
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(receiver, "std::forward") {
        cpp_this_receiver_from_expression(argument)?;
        return cpp_this_receiver_for_type(type_name, Some(true));
    }
    None
}
