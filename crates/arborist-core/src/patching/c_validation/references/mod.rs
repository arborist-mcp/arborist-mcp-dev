use anyhow::Result;
use tree_sitter::Node;

use super::cpp_syntax::{
    compact_cpp_expression, cpp_constructor_type_text, cpp_receiver_call_argument,
    cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::cpp_wrappers::{
    cpp_standard_contiguous_sequence_element_type, cpp_standard_expected_error_type,
    cpp_standard_expected_target_type, cpp_standard_indexable_sequence_element_type,
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_sequence_element_type, cpp_standard_smart_pointer_target_type,
    cpp_standard_weak_pointer_target_type,
};
use crate::language::node_text;

mod bindings;
mod call_arities;
mod member_call_names;
mod name_collection;
mod std_get;
mod type_qualifiers;
mod types;

pub(super) use bindings::{
    collect_cpp_local_bindings, cpp_address_binding, cpp_named_reference_alias_receiver,
};
pub(crate) use call_arities::{collect_c_call_arities, collect_cpp_call_arities};
pub(super) use name_collection::collect_c_local_definitions;
pub(crate) use name_collection::{collect_c_graph_references, collect_c_references};
use std_get::*;
use type_qualifiers::*;
pub(super) use types::*;

pub(super) fn cpp_local_member_receiver_type(
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

fn cpp_local_member_receiver_from_expression(
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

fn cpp_standard_sequence_element_receiver(
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

pub(super) fn cpp_standard_sequence_at_receiver(expression: &str) -> Option<&str> {
    let receiver = expression.strip_suffix(')')?.trim_end();
    let opening = receiver.rfind(".at(")?;
    let arguments = &receiver[opening + ".at(".len()..];
    (!arguments.is_empty()).then_some(receiver[..opening].trim())
}

fn cpp_standard_indexable_sequence_element_receiver(
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

fn cpp_standard_sequence_data_receiver(
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

pub(super) fn cpp_subscript_receiver(expression: &str) -> Option<&str> {
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

fn cpp_standard_wrapper_get_binding<'a>(
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

fn cpp_standard_weak_pointer_lock_receiver(
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

fn cpp_expected_weak_pointer_lock_receiver(
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

fn cpp_standard_reference_factory_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    cpp_standard_reference_factory_binding(receiver, byte_offset, local_bindings)
}

fn cpp_standard_reference_factory_binding(
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

fn cpp_reference_factory_argument_receiver(
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

fn cpp_standard_optional_value_member_receiver(
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

fn cpp_strip_optional_value_access(expression: &str) -> Option<&str> {
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

fn cpp_standard_expected_error_member_receiver(
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

fn cpp_strip_expected_error_access(expression: &str) -> Option<&str> {
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

fn cpp_standard_optional_dereference_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = strip_cpp_outer_parentheses(expression.strip_prefix('*')?.trim());
    cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings)
}

fn cpp_standard_optional_arrow_member_receiver(
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

fn cpp_optional_smart_pointer_arrow_member_receiver(
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

fn cpp_expected_error_smart_pointer_arrow_member_receiver(
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

fn cpp_smart_pointer_get_receiver(
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

fn cpp_smart_pointer_dereference_receiver(
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

fn cpp_smart_pointer_wrapper_type(
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

fn cpp_expected_error_nested_arrow_member_receiver(
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

fn cpp_expected_error_optional_arrow_member_receiver(
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

fn cpp_expected_reference_wrapper_get_receiver(
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

fn cpp_optional_wrapper_type_from_expression(
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

fn cpp_expected_error_optional_value_member_receiver(
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

fn cpp_expected_error_optional_dereference_receiver(
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

fn cpp_standard_value_member_receiver(
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

fn cpp_optional_local_binding_receiver(
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

fn cpp_expected_error_type_from_wrapper(
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

fn cpp_expected_local_binding_error_receiver(
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

fn cpp_optional_wrapper_only_receiver(
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

fn cpp_expected_error_receiver(
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

fn cpp_local_binding_name_from_expression(expression: &str) -> Option<&str> {
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

fn cpp_addressable_local_object_receiver(
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

fn cpp_addressable_receiver(receiver: CppThisMemberReceiver) -> CppThisMemberReceiver {
    match receiver {
        CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
            CppThisMemberReceiver::Lvalue
        }
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
    }
}

fn cpp_addressable_local_binding_name(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let argument = cpp_receiver_call_argument(expression, "std::addressof")
        .or_else(|| expression.strip_prefix('&').map(str::trim))?;
    cpp_addressable_local_binding_name_from_expression(argument)
}

fn cpp_addressable_local_binding_name_from_expression(expression: &str) -> Option<&str> {
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

pub(super) fn cpp_visible_local_binding<'a>(
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

pub(super) fn cpp_temporary_type_from_expression(
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

pub(super) fn cpp_this_receiver_from_expression(receiver: &str) -> Option<CppThisMemberReceiver> {
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

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use crate::language::parse_document;

    use super::super::cpp_types::cpp_type_is_top_level_const;
    use super::{
        collect_c_graph_references, collect_cpp_call_arities, cpp_this_receiver_from_expression,
    };
    use crate::symbol_index_model::{
        CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CPP_TEMPORARY_MEMBER_CALL_SEPARATOR,
    };

    #[test]
    fn collects_only_object_braced_initializers() {
        let source = "namespace api { class Counter { public: Counter(int value) {} }; }\nint caller(api::Counter* existing, api::Counter& current) { api::Counter counter{1}; api::Counter* pointer{existing}; api::Counter& reference{current}; return 0; }\n";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([("api::Counter".to_string(), BTreeSet::from([1]))])
        );
    }

    #[test]
    fn collects_this_and_typed_pointer_member_call_arities() {
        let source = "class Counter { int adjust(int value) { return value; } int caller(Counter* other) { return this->adjust(1) + (*this).adjust(1, 2) + other->adjust(1, 2, 3) + (*other).adjust(1, 2, 3, 4); } };";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([
                ("adjust".to_string(), BTreeSet::from([1, 2])),
                (
                    format!(
                        "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
                    ),
                    BTreeSet::from([3, 4]),
                ),
            ])
        );
    }

    #[test]
    fn ignores_parameters_of_local_function_prototypes() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller(int value) { int declared(Counter current); return current.adjust(value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert!(!arities.keys().any(|name| {
            name.contains("Counter::adjust")
                && name.starts_with(CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX)
        }));
    }

    #[test]
    fn collects_catch_parameter_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller(int value) { try { throw value; } catch (Counter current) { return current.adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn collects_this_member_template_call_arities() {
        let source = "class Counter { template <typename T> T adjust(T value) { return value; } int caller(int value) { return this->template adjust<int>(value); } };";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([("adjust<int>".to_string(), BTreeSet::from([1]))])
        );
    }

    #[test]
    fn collects_temporary_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) && { return value; } }; int caller(int value) { return Counter{}.adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([
                ("Counter".to_string(), BTreeSet::from([0])),
                (
                    format!(
                        "{CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
                    ),
                    BTreeSet::from([1]),
                ),
            ])
        );
    }

    #[test]
    fn collects_local_variable_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } int adjust(int value) && { return value; } }; int caller(int value) { Counter current{}; const Counter locked{}; return current.adjust(value) + locked.adjust(value) + std::move(current).adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn collects_wrapped_pointer_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(Counter* current, int value) { return std::as_const(current)->adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert!(!arities.contains_key(&format!(
            "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
        )));
    }

    #[test]
    fn collects_auto_reference_factory_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; auto mutable_ref = std::ref(target); auto parenthesized_ref = (std::ref(target)); auto const_ref = std::cref(target); auto as_const_ref = std::ref(std::as_const(target)); return mutable_ref.get().adjust(value) + parenthesized_ref.get().adjust(value) + const_ref.get().adjust(value) + as_const_ref.get().adjust(value) + (std::cref(target)).get().adjust(value) + std::ref(std::move(target)).get().adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn scopes_auto_reference_factories_to_visible_bindings() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; { const Counter target{}; auto current = std::ref(target); current.get().adjust(value); } auto current = std::ref(target); return current.get().adjust(value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn collects_auto_addressof_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; const Counter locked{}; auto mutable_pointer = std::addressof(target); auto const_pointer = std::addressof(locked); auto native_pointer = &target; auto native_const_pointer = &locked; return mutable_pointer->adjust(value) + const_pointer->adjust(value) + native_pointer->adjust(value, value) + native_const_pointer->adjust(value, value, value) + std::addressof(std::move(target))->adjust(value, value, value, value) + std::addressof(std::as_const(target))->adjust(value, value, value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 2, 4]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 3, 5]))
        );
    }

    #[test]
    fn collects_auto_reference_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; const Counter locked{}; Counter* pointer = &target; const Counter* const_pointer = &locked; std::reference_wrapper<Counter> wrapper(target); std::reference_wrapper<const Counter> const_wrapper(locked); auto& mutable_alias = target; const auto& const_alias = target; auto const& postfix_const_alias = target; auto&& forwarding_alias = locked; auto&& moved_alias = std::move(target); auto&& as_const_alias = std::as_const(target); auto&& forwarded_alias = std::forward<Counter&&>(target); auto&& const_forwarded_alias = std::forward<const Counter&&>(target); auto&& cast_alias = static_cast<Counter&&>(target); auto&& const_cast_alias = static_cast<const Counter&&>(target); auto& pointer_alias = *pointer; auto&& const_pointer_alias = *const_pointer; auto& address_alias = *std::addressof(std::as_const(target)); auto& wrapper_alias = wrapper.get(); auto&& const_wrapper_alias = const_wrapper.get(); auto&& ref_alias = std::ref(target).get(); auto&& cref_alias = std::cref(target).get(); return mutable_alias.adjust(value) + const_alias.adjust(value, value) + postfix_const_alias.adjust(value, value, value) + forwarding_alias.adjust(value, value, value, value) + moved_alias.adjust(value, value, value, value, value) + as_const_alias.adjust(value, value, value, value, value, value) + forwarded_alias.adjust(value, value, value, value, value, value, value) + const_forwarded_alias.adjust(value, value, value, value, value, value, value, value) + cast_alias.adjust(value, value, value, value, value, value, value, value, value) + const_cast_alias.adjust(value, value, value, value, value, value, value, value, value, value) + pointer_alias.adjust(value, value, value, value, value, value, value, value, value, value, value) + const_pointer_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value) + address_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value) + wrapper_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value, value) + const_wrapper_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value, value, value) + ref_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value) + cref_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 5, 7, 9, 11, 14, 16]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2, 3, 4, 6, 8, 10, 12, 13, 15, 17]))
        );
    }

    #[test]
    fn collects_decltype_auto_reference_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; const Counter locked{}; Counter* pointer = &target; std::optional<Counter> optional; std::reference_wrapper<Counter> wrapper(target); decltype(auto) copied_value = target; decltype(auto) copied_const_value = locked; decltype(auto) parenthesized_alias = (target); decltype(auto) const_alias = (locked); decltype(auto) moved_alias = std::move(target); decltype(auto) pointer_alias = *pointer; decltype(auto) optional_alias = optional.value(); decltype(auto) wrapper_alias = wrapper.get(); return copied_value.adjust(value) + copied_const_value.adjust(value, value) + parenthesized_alias.adjust(value, value, value) + const_alias.adjust(value, value, value, value) + moved_alias.adjust(value, value, value, value, value) + pointer_alias.adjust(value, value, value, value, value, value) + optional_alias.adjust(value, value, value, value, value, value, value) + wrapper_alias.adjust(value, value, value, value, value, value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 3, 5, 6, 7, 8]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2, 4]))
        );
    }

    #[test]
    fn collects_auto_optional_value_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { std::optional<Counter> current; const std::optional<Counter> locked{}; auto& value_alias = current.value(); auto&& const_value_alias = locked.value(); auto&& moved_value_alias = std::move(current).value(); return value_alias.adjust(value) + const_value_alias.adjust(value, value) + moved_value_alias.adjust(value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 3]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2]))
        );
    }

    #[test]
    fn collects_auto_optional_dereference_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { std::optional<Counter> current; const std::optional<Counter> locked{}; auto& value_alias = *current; auto&& const_value_alias = *locked; auto&& moved_value_alias = *std::move(current); auto&& moved_alias = std::move(*current); auto&& as_const_alias = std::as_const(*current); auto&& forwarded_alias = std::forward<Counter&&>(*current); auto&& const_forwarded_alias = std::forward<const Counter&&>(*current); return value_alias.adjust(value) + const_value_alias.adjust(value, value) + moved_value_alias.adjust(value, value, value) + moved_alias.adjust(value, value, value, value) + as_const_alias.adjust(value, value, value, value, value) + forwarded_alias.adjust(value, value, value, value, value, value) + const_forwarded_alias.adjust(value, value, value, value, value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 3, 4, 6]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2, 5, 7]))
        );
    }

    #[test]
    fn collects_auto_smart_pointer_dereference_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { std::unique_ptr<Counter> current; std::shared_ptr<const Counter> locked; auto& value_alias = *current; auto&& const_value_alias = *locked; return value_alias.adjust(value) + const_value_alias.adjust(value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2]))
        );
    }

    #[test]
    fn distinguishes_auto_direct_and_copy_list_initializers() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller(int value) { auto direct{Counter{}}; auto copied = {Counter{}}; return direct.adjust(value) + copied.adjust(value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn scopes_range_for_bindings_to_the_loop() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller() { for (Counter current : values) { current.adjust(1); } return current.adjust(1, 2); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn identifies_only_top_level_cpp_const_qualifiers() {
        assert!(cpp_type_is_top_level_const("const Counter&&"));
        assert!(cpp_type_is_top_level_const("Counter const &"));
        assert!(!cpp_type_is_top_level_const("constCounter&&"));
        assert!(!cpp_type_is_top_level_const("Wrapper<const Counter>&&"));
    }

    #[test]
    fn rejects_non_this_and_malformed_cpp_member_receivers() {
        assert!(cpp_this_receiver_from_expression("std::move(other)").is_none());
        assert!(cpp_this_receiver_from_expression("std::forward<Counter&>(other)").is_none());
        assert!(cpp_this_receiver_from_expression("static_cast<Counter&&>(*this").is_none());
    }

    #[test]
    fn skips_nested_cpp_type_template_arguments_when_collecting_references() {
        let source = "class Value {}; class Counter { public: int adjust(int) &; }; int caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { auto error = current.error(); return error.error().adjust(value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut references = BTreeSet::new();

        collect_c_graph_references(document.tree.root_node(), source, &mut references).unwrap();

        assert!(!references.contains("Value"));
    }
}
