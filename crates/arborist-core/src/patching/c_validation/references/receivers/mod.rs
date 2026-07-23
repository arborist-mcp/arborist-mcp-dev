use anyhow::Result;
use tree_sitter::Node;

use super::super::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::cpp_wrappers::cpp_standard_smart_pointer_target_type;
use super::bindings::{cpp_address_binding, cpp_named_reference_alias_receiver};
use super::std_get::*;
use super::types::{CppLocalBinding, CppMemberAccess, CppStandardUnwrap};
use crate::language::node_text;

mod binding_lookup;
mod sequence;
mod wrappers;

pub(super) use binding_lookup::*;
pub(super) use sequence::*;
pub(super) use wrappers::*;

// Keep c_validation-facing re-exports at the receivers module boundary.
pub(in super::super) use binding_lookup::{
    cpp_temporary_type_from_expression, cpp_this_receiver_from_expression,
    cpp_visible_local_binding,
};
pub(in super::super) use sequence::{cpp_standard_sequence_at_receiver, cpp_subscript_receiver};

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
