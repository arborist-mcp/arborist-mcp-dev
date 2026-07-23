use super::super::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::cpp_types::{
    cpp_temporary_type_path, cpp_this_receiver_for_type, cpp_top_level_pointer_pointee,
};
use super::super::cpp_wrappers::{
    cpp_standard_contiguous_sequence_element_type, cpp_standard_expected_error_type,
    cpp_standard_expected_target_type, cpp_standard_indexed_element_type,
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_sequence_element_type, cpp_standard_smart_pointer_target_type,
    cpp_standard_typed_get_element_type, cpp_standard_weak_pointer_target_type,
};
use super::{CppLocalBinding, CppMemberAccess, CppThisMemberReceiver, cpp_visible_local_binding};

// Forward helpers defined later in the parent module.
use super::{cpp_standard_sequence_at_receiver, cpp_subscript_receiver};

pub(super) fn cpp_smart_pointer_factory_type(expression: &str) -> Option<&str> {
    ["std::make_unique", "std::make_shared"]
        .into_iter()
        .find_map(|factory| {
            cpp_typed_receiver_call(expression, factory).map(|(type_name, _)| type_name)
        })
}

pub(super) fn cpp_get_if_pointer_type(expression: &str) -> Option<&str> {
    // std::get_if<T>(...) yields T*. Treat the explicit template argument as the
    // pointee type for auto/auto* bindings and later nested->member() calls.
    cpp_typed_receiver_call(expression, "std::get_if").map(|(type_name, _)| type_name)
}

pub(super) fn cpp_get_if_direct_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (requested_type, argument) = cpp_typed_receiver_call(expression, "std::get_if")?;
    let (binding, argument_receiver) =
        cpp_direct_pointer_argument_binding(argument, byte_offset, local_bindings)?;
    if binding.access != CppMemberAccess::Object || binding.standard_unwrap.is_some() {
        return None;
    }
    let element_type = cpp_standard_typed_get_element_type(&binding.type_name, requested_type)?;
    Some((
        cpp_temporary_type_path(element_type)?,
        cpp_direct_pointer_pointee_receiver(requested_type, argument_receiver)?,
    ))
}

pub(super) fn cpp_any_cast_direct_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (requested_type, argument) = cpp_typed_receiver_call(expression, "std::any_cast")?;
    let (_, argument_receiver) =
        cpp_direct_pointer_argument_binding(argument, byte_offset, local_bindings)?;
    Some((
        cpp_temporary_type_path(requested_type)?,
        cpp_direct_pointer_pointee_receiver(requested_type, argument_receiver)?,
    ))
}

pub(super) fn cpp_direct_pointer_argument_binding<'a>(
    argument: &str,
    byte_offset: usize,
    local_bindings: &'a [CppLocalBinding],
) -> Option<(&'a CppLocalBinding, CppThisMemberReceiver)> {
    let argument = strip_cpp_outer_parentheses(argument.trim());
    let receiver = argument
        .strip_prefix('&')
        .map(str::trim)
        .or_else(|| cpp_receiver_call_argument(argument, "std::addressof"))?;
    cpp_standard_get_container_binding(receiver, byte_offset, local_bindings)
}

pub(super) fn cpp_direct_pointer_pointee_receiver(
    requested_type: &str,
    argument_receiver: CppThisMemberReceiver,
) -> Option<CppThisMemberReceiver> {
    let requested_receiver = cpp_this_receiver_for_type(requested_type, Some(false))?;
    matches!(
        (argument_receiver, requested_receiver),
        (
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue,
            _
        ) | (
            _,
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
        )
    )
    .then_some(CppThisMemberReceiver::ConstLvalue)
    .or(Some(CppThisMemberReceiver::Lvalue))
}

pub(super) fn cpp_pointer_cast_shared_pointer_type(expression: &str) -> Option<&str> {
    // std::*_pointer_cast<T>(shared_ptr<...>) yields shared_ptr<T>. Keep the
    // smart-pointer unwrap so later nested->member() still resolves.
    [
        "std::static_pointer_cast",
        "std::dynamic_pointer_cast",
        "std::const_pointer_cast",
        "std::reinterpret_pointer_cast",
    ]
    .into_iter()
    .find_map(|factory| {
        cpp_typed_receiver_call(expression, factory).map(|(type_name, _)| type_name)
    })
}

pub(super) fn cpp_any_cast_pointer_type(expression: &str) -> Option<&str> {
    // std::any_cast<T>(&any) yields T*. Value any_cast forms are handled separately.
    let (type_name, argument) = cpp_typed_receiver_call(expression, "std::any_cast")?;
    let argument = strip_cpp_outer_parentheses(argument.trim());
    if argument.starts_with('&') || cpp_receiver_call_argument(argument, "std::addressof").is_some()
    {
        Some(type_name)
    } else {
        None
    }
}

pub(super) fn cpp_any_cast_value_type(expression: &str) -> Option<&str> {
    // std::any_cast<T>(any) yields T by value.
    let (type_name, argument) = cpp_typed_receiver_call(expression, "std::any_cast")?;
    let argument = strip_cpp_outer_parentheses(argument.trim());
    if argument.starts_with('&') || cpp_receiver_call_argument(argument, "std::addressof").is_some()
    {
        None
    } else {
        Some(type_name)
    }
}

pub(super) fn cpp_typed_standard_get_type(expression: &str) -> Option<&str> {
    // std::get<T>(tuple-like) yields T by value/reference for binding copies.
    // Prefer type-based get; index-based get is handled through the local binding.
    let (type_name, _) = cpp_typed_receiver_call(expression, "std::get")?;
    // Reject obvious non-type template arguments such as std::get<0>(...).
    if type_name
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        return None;
    }
    Some(type_name)
}

pub(super) fn cpp_typed_standard_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (type_name, binding_receiver) =
        cpp_typed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let receiver = cpp_standard_get_element_receiver(&type_name, binding_receiver, true)?;
    Some((cpp_temporary_type_path(&type_name)?, receiver))
}

pub(super) fn cpp_standard_get_element_receiver(
    element_type: &str,
    container_receiver: CppThisMemberReceiver,
    preserves_value_category: bool,
) -> Option<CppThisMemberReceiver> {
    let element_receiver = cpp_this_receiver_for_type(
        element_type,
        Some(
            preserves_value_category
                && matches!(
                    container_receiver,
                    CppThisMemberReceiver::Rvalue | CppThisMemberReceiver::ConstRvalue
                ),
        ),
    )?;
    let const_qualified = matches!(
        container_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    ) || matches!(
        element_receiver,
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
    );
    let rvalue = matches!(
        element_receiver,
        CppThisMemberReceiver::Rvalue | CppThisMemberReceiver::ConstRvalue
    );
    Some(match (const_qualified, rvalue) {
        (false, false) => CppThisMemberReceiver::Lvalue,
        (true, false) => CppThisMemberReceiver::ConstLvalue,
        (false, true) => CppThisMemberReceiver::Rvalue,
        (true, true) => CppThisMemberReceiver::ConstRvalue,
    })
}

pub(super) fn cpp_typed_standard_get_element_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let requested_type = cpp_typed_standard_get_type(expression)?;
    let (_, argument) = cpp_typed_receiver_call(expression, "std::get")?;
    let (binding, receiver) =
        cpp_standard_get_container_binding(argument, byte_offset, local_bindings)?;
    if binding.access != CppMemberAccess::Object || binding.standard_unwrap.is_some() {
        return None;
    }
    let element_type = cpp_standard_typed_get_element_type(&binding.type_name, requested_type)?;
    Some((element_type.to_string(), receiver))
}

pub(super) fn cpp_standard_get_container_binding<'a>(
    expression: &str,
    byte_offset: usize,
    local_bindings: &'a [CppLocalBinding],
) -> Option<(&'a CppLocalBinding, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings) {
        return Some((binding, binding.receiver));
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        let (binding, receiver) =
            cpp_standard_get_container_binding(argument, byte_offset, local_bindings)?;
        let receiver = match receiver {
            CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                CppThisMemberReceiver::Rvalue
            }
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                CppThisMemberReceiver::ConstRvalue
            }
        };
        return Some((binding, receiver));
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        let (binding, _) =
            cpp_standard_get_container_binding(argument, byte_offset, local_bindings)?;
        return Some((binding, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let (binding, _) =
            cpp_standard_get_container_binding(argument, byte_offset, local_bindings)?;
        return Some((binding, cpp_this_receiver_for_type(type_name, Some(true))?));
    }
    None
}

pub(super) fn cpp_typed_standard_get_smart_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (pointer_type, _) =
        cpp_typed_standard_get_receiver(expression, byte_offset, local_bindings)?;
    let target = cpp_standard_smart_pointer_target_type(&pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    cpp_typed_standard_get_smart_pointer_receiver(receiver, byte_offset, local_bindings)
}

pub(super) fn cpp_typed_standard_get_optional_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (optional_type, binding_receiver) =
        cpp_typed_standard_get_receiver(expression, byte_offset, local_bindings)?;
    let target = cpp_standard_optional_target_type(&optional_type)?;
    let receiver = cpp_standard_get_element_receiver(target, binding_receiver, false)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_typed_standard_get_optional_smart_pointer_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (optional_type, _) =
        cpp_typed_standard_get_receiver(expression, byte_offset, local_bindings)?;
    let pointer_type = cpp_standard_optional_target_type(&optional_type)?;
    let target = cpp_standard_smart_pointer_target_type(pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (expected_type, binding_receiver) =
        cpp_typed_standard_get_receiver(expression, byte_offset, local_bindings)?;
    let target = cpp_standard_expected_target_type(&expected_type)?;
    let receiver = cpp_standard_get_element_receiver(target, binding_receiver, false)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_typed_standard_get_expected_smart_pointer_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (expected_type, _) =
        cpp_typed_standard_get_receiver(expression, byte_offset, local_bindings)?;
    let pointer_type = cpp_standard_expected_target_type(&expected_type)?;
    let target = cpp_standard_smart_pointer_target_type(pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_value_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let receiver = receiver.strip_suffix(".value()")?.trim();
    let (expected_type, _) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let pointer_type = cpp_standard_expected_target_type(&expected_type)?;
    let target = cpp_standard_smart_pointer_target_type(pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_error_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let receiver = receiver.strip_suffix(".error()")?.trim();
    let (expected_type, _) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let pointer_type = cpp_standard_expected_error_type(&expected_type)?;
    let target = cpp_standard_smart_pointer_target_type(pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_value_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".lock()")?.trim();
    let receiver = receiver.strip_suffix(".value()")?.trim();
    let (expected_type, _) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let weak_pointer_type = cpp_standard_expected_target_type(&expected_type)?;
    let target = cpp_standard_weak_pointer_target_type(weak_pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_error_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".lock()")?.trim();
    let receiver = receiver.strip_suffix(".error()")?.trim();
    let (expected_type, _) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let weak_pointer_type = cpp_standard_expected_error_type(&expected_type)?;
    let target = cpp_standard_weak_pointer_target_type(weak_pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_value_reference_wrapper_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let receiver = receiver.strip_suffix(".value()")?.trim();
    let (expected_type, _) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let wrapper_type = cpp_standard_expected_target_type(&expected_type)?;
    let target = cpp_standard_reference_wrapper_target_type(wrapper_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_error_reference_wrapper_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let receiver = receiver.strip_suffix(".error()")?.trim();
    let (expected_type, _) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let wrapper_type = cpp_standard_expected_error_type(&expected_type)?;
    let target = cpp_standard_reference_wrapper_target_type(wrapper_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_optional_target(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<String> {
    let (receiver, value_target) = if let Some(receiver) = expression.strip_suffix(".value()") {
        (receiver.trim(), true)
    } else {
        let receiver = expression.strip_suffix(".error()")?;
        (receiver.trim(), false)
    };
    let (expected_type, _) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let optional_type = if value_target {
        cpp_standard_expected_target_type(&expected_type)?
    } else {
        cpp_standard_expected_error_type(&expected_type)?
    };
    Some(cpp_standard_optional_target_type(optional_type)?.to_string())
}

pub(super) fn cpp_typed_standard_get_expected_optional_smart_pointer_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let pointer_type =
        cpp_typed_standard_get_expected_optional_target(expression, byte_offset, local_bindings)?;
    let target = cpp_standard_smart_pointer_target_type(&pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_optional_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression
        .strip_suffix(".lock()")
        .or_else(|| expression.strip_suffix("->lock()"))
        .map(str::trim)?;
    let weak_pointer_type =
        cpp_typed_standard_get_expected_optional_target(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_weak_pointer_target_type(&weak_pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_optional_reference_wrapper_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression
        .strip_suffix(".get()")
        .or_else(|| expression.strip_suffix("->get()"))
        .map(str::trim)?;
    let wrapper_type =
        cpp_typed_standard_get_expected_optional_target(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_reference_wrapper_target_type(&wrapper_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_optional_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression
        .strip_suffix(".get()")
        .or_else(|| expression.strip_suffix("->get()"))
        .map(str::trim)?;
    let pointer_type =
        cpp_typed_standard_get_expected_optional_target(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_smart_pointer_target_type(&pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_expected_sequence_type(
    receiver: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (receiver, value_target) = if let Some(receiver) = receiver.strip_suffix(".value()") {
        (receiver.trim(), true)
    } else {
        let receiver = receiver.strip_suffix(".error()")?;
        (receiver.trim(), false)
    };
    let (expected_type, container_receiver) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let sequence_type = if value_target {
        cpp_standard_expected_target_type(&expected_type)?
    } else {
        cpp_standard_expected_error_type(&expected_type)?
    };
    Some((sequence_type.to_string(), container_receiver))
}

pub(super) fn cpp_typed_standard_get_expected_sequence_element_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = cpp_standard_sequence_element_access_receiver(expression)?;
    let (sequence_type, container_receiver) =
        cpp_typed_standard_get_expected_sequence_type(receiver, byte_offset, local_bindings)?;
    let element_type = cpp_standard_sequence_element_type(&sequence_type)?;
    let receiver = match container_receiver {
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
        _ => cpp_this_receiver_for_type(element_type, Some(false))?,
    };
    Some((cpp_temporary_type_path(element_type)?, receiver))
}

pub(super) fn cpp_typed_standard_get_expected_sequence_data_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".data()")?.trim();
    let (sequence_type, container_receiver) =
        cpp_typed_standard_get_expected_sequence_type(receiver, byte_offset, local_bindings)?;
    let element_type = cpp_standard_contiguous_sequence_element_type(&sequence_type)?;
    let receiver = match container_receiver {
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
        _ => cpp_this_receiver_for_type(element_type, Some(false))?,
    };
    Some((cpp_temporary_type_path(element_type)?, receiver))
}

pub(super) fn cpp_typed_standard_get_optional_value_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".value()")?.trim();
    let (optional_type, binding_receiver) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_optional_target_type(&optional_type)?;
    let receiver = cpp_standard_get_element_receiver(target, binding_receiver, true)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_typed_standard_get_expected_value_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".value()")?.trim();
    let (expected_type, binding_receiver) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_expected_target_type(&expected_type)?;
    let receiver = cpp_standard_get_element_receiver(target, binding_receiver, true)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_typed_standard_get_expected_error_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".error()")?.trim();
    let (expected_type, binding_receiver) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_expected_error_type(&expected_type)?;
    let receiver = cpp_standard_get_element_receiver(target, binding_receiver, true)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_typed_standard_get_raw_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (pointer_type, _) =
        cpp_typed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let target = cpp_top_level_pointer_pointee(&pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".lock()")?.trim();
    let (weak_pointer_type, _) =
        cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_weak_pointer_target_type(&weak_pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_typed_standard_get_reference_wrapper_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let (wrapper_type, _) = cpp_typed_standard_get_receiver(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_reference_wrapper_target_type(&wrapper_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (element_type, container_receiver) =
        cpp_indexed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let receiver = cpp_standard_get_element_receiver(&element_type, container_receiver, true)?;
    Some((cpp_temporary_type_path(&element_type)?, receiver))
}

pub(super) fn cpp_indexed_standard_get_element_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (index, argument) = cpp_typed_receiver_call(expression, "std::get")?;
    let index = index.parse::<usize>().ok()?;
    let (binding, receiver) =
        cpp_standard_get_container_binding(argument, byte_offset, local_bindings)?;
    if binding.access != CppMemberAccess::Object || binding.standard_unwrap.is_some() {
        return None;
    }
    let element_type = cpp_standard_indexed_element_type(&binding.type_name, index)?;
    Some((element_type.to_string(), receiver))
}

pub(super) fn cpp_indexed_tuple_get_smart_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let target = cpp_standard_smart_pointer_target_type(&element_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_raw_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let pointee_type = cpp_top_level_pointer_pointee(&element_type)?;
    Some((
        cpp_temporary_type_path(pointee_type)?,
        cpp_this_receiver_for_type(pointee_type, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_optional_value_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".value()")?.trim();
    let (element_type, container_receiver) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_optional_target_type(&element_type)?;
    let receiver = cpp_standard_get_element_receiver(target, container_receiver, true)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_indexed_tuple_get_optional_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (element_type, container_receiver) =
        cpp_indexed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let target = cpp_standard_optional_target_type(&element_type)?;
    let receiver = cpp_standard_get_element_receiver(target, container_receiver, false)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_indexed_tuple_get_optional_smart_pointer_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let wrapper_target = cpp_standard_optional_target_type(&element_type)?;
    let target = cpp_standard_smart_pointer_target_type(wrapper_target)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_value_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".value()")?.trim();
    let (element_type, container_receiver) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_expected_target_type(&element_type)?;
    let receiver = cpp_standard_get_element_receiver(target, container_receiver, true)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_indexed_tuple_get_expected_value_smart_pointer_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".value()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let value_type = cpp_standard_expected_target_type(&element_type)?;
    let target = cpp_standard_smart_pointer_target_type(value_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_value_raw_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".value()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let value_type = cpp_standard_expected_target_type(&element_type)?;
    let pointee_type = cpp_top_level_pointer_pointee(value_type)?;
    Some((
        cpp_temporary_type_path(pointee_type)?,
        cpp_this_receiver_for_type(pointee_type, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_optional_smart_pointer_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (receiver, value_target) = if let Some(receiver) = expression.strip_suffix(".value()") {
        (receiver.trim(), true)
    } else {
        let receiver = expression.strip_suffix(".error()")?;
        (receiver.trim(), false)
    };
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let optional_type = if value_target {
        cpp_standard_expected_target_type(&element_type)?
    } else {
        cpp_standard_expected_error_type(&element_type)?
    };
    let pointer_type = cpp_standard_optional_target_type(optional_type)?;
    let target = cpp_standard_smart_pointer_target_type(pointer_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_optional_raw_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (receiver, value_target) = if let Some(receiver) = expression.strip_suffix(".value()") {
        (receiver.trim(), true)
    } else {
        let receiver = expression.strip_suffix(".error()")?;
        (receiver.trim(), false)
    };
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let optional_type = if value_target {
        cpp_standard_expected_target_type(&element_type)?
    } else {
        cpp_standard_expected_error_type(&element_type)?
    };
    let pointer_type = cpp_standard_optional_target_type(optional_type)?;
    let pointee_type = cpp_top_level_pointer_pointee(pointer_type)?;
    Some((
        cpp_temporary_type_path(pointee_type)?,
        cpp_this_receiver_for_type(pointee_type, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_value_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let receiver = receiver.strip_suffix(".value()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let value_type = cpp_standard_expected_target_type(&element_type)?;
    let target = cpp_standard_smart_pointer_target_type(value_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (element_type, container_receiver) =
        cpp_indexed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let target = cpp_standard_expected_target_type(&element_type)?;
    let receiver = cpp_standard_get_element_receiver(target, container_receiver, false)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_indexed_tuple_get_expected_smart_pointer_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let wrapper_target = cpp_standard_expected_target_type(&element_type)?;
    let target = cpp_standard_smart_pointer_target_type(wrapper_target)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_error_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".error()")?.trim();
    let (element_type, container_receiver) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_expected_error_type(&element_type)?;
    let receiver = cpp_standard_get_element_receiver(target, container_receiver, true)?;
    Some((cpp_temporary_type_path(target)?, receiver))
}

pub(super) fn cpp_indexed_tuple_get_expected_error_smart_pointer_arrow_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".error()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let error_type = cpp_standard_expected_error_type(&element_type)?;
    let target = cpp_standard_smart_pointer_target_type(error_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_error_raw_pointer_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".error()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let error_type = cpp_standard_expected_error_type(&element_type)?;
    let pointee_type = cpp_top_level_pointer_pointee(error_type)?;
    Some((
        cpp_temporary_type_path(pointee_type)?,
        cpp_this_receiver_for_type(pointee_type, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_error_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let receiver = receiver.strip_suffix(".error()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let error_type = cpp_standard_expected_error_type(&element_type)?;
    let target = cpp_standard_smart_pointer_target_type(error_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_value_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".lock()")?.trim();
    let receiver = receiver.strip_suffix(".value()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let value_type = cpp_standard_expected_target_type(&element_type)?;
    let target = cpp_standard_weak_pointer_target_type(value_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_value_reference_wrapper_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let receiver = receiver.strip_suffix(".value()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let value_type = cpp_standard_expected_target_type(&element_type)?;
    let target = cpp_standard_reference_wrapper_target_type(value_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_error_reference_wrapper_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let receiver = receiver.strip_suffix(".error()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let error_type = cpp_standard_expected_error_type(&element_type)?;
    let target = cpp_standard_reference_wrapper_target_type(error_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_error_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".lock()")?.trim();
    let receiver = receiver.strip_suffix(".error()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let error_type = cpp_standard_expected_error_type(&element_type)?;
    let target = cpp_standard_weak_pointer_target_type(error_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_optional_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression
        .strip_suffix(".lock()")
        .or_else(|| expression.strip_suffix("->lock()"))
        .map(str::trim)?;
    let (receiver, value_target) = if let Some(receiver) = receiver.strip_suffix(".value()") {
        (receiver.trim(), true)
    } else {
        let receiver = receiver.strip_suffix(".error()")?;
        (receiver.trim(), false)
    };
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let expected_target = if value_target {
        cpp_standard_expected_target_type(&element_type)?
    } else {
        cpp_standard_expected_error_type(&element_type)?
    };
    let optional_target = cpp_standard_optional_target_type(expected_target)?;
    let target = cpp_standard_weak_pointer_target_type(optional_target)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_optional_reference_wrapper_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression
        .strip_suffix(".get()")
        .or_else(|| expression.strip_suffix("->get()"))
        .map(str::trim)?;
    let (receiver, value_target) = if let Some(receiver) = receiver.strip_suffix(".value()") {
        (receiver.trim(), true)
    } else {
        let receiver = receiver.strip_suffix(".error()")?;
        (receiver.trim(), false)
    };
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let expected_target = if value_target {
        cpp_standard_expected_target_type(&element_type)?
    } else {
        cpp_standard_expected_error_type(&element_type)?
    };
    let optional_target = cpp_standard_optional_target_type(expected_target)?;
    let target = cpp_standard_reference_wrapper_target_type(optional_target)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_optional_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression
        .strip_suffix(".get()")
        .or_else(|| expression.strip_suffix("->get()"))
        .map(str::trim)?;
    let (receiver, value_target) = if let Some(receiver) = receiver.strip_suffix(".value()") {
        (receiver.trim(), true)
    } else {
        let receiver = receiver.strip_suffix(".error()")?;
        (receiver.trim(), false)
    };
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let expected_target = if value_target {
        cpp_standard_expected_target_type(&element_type)?
    } else {
        cpp_standard_expected_error_type(&element_type)?
    };
    let optional_target = cpp_standard_optional_target_type(expected_target)?;
    let target = cpp_standard_smart_pointer_target_type(optional_target)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_expected_sequence_element_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = cpp_standard_sequence_element_access_receiver(expression)?;
    let (sequence_type, binding_receiver) =
        cpp_indexed_tuple_get_expected_sequence_type(receiver, byte_offset, local_bindings)?;
    let element_type = cpp_standard_sequence_element_type(&sequence_type)?;
    let receiver = match binding_receiver {
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
        _ => cpp_this_receiver_for_type(element_type, Some(false))?,
    };
    Some((cpp_temporary_type_path(element_type)?, receiver))
}

pub(super) fn cpp_indexed_tuple_get_expected_sequence_data_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".data()")?.trim();
    let (sequence_type, binding_receiver) =
        cpp_indexed_tuple_get_expected_sequence_type(receiver, byte_offset, local_bindings)?;
    let element_type = cpp_standard_contiguous_sequence_element_type(&sequence_type)?;
    let receiver = match binding_receiver {
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
        _ => cpp_this_receiver_for_type(element_type, Some(false))?,
    };
    Some((cpp_temporary_type_path(element_type)?, receiver))
}

pub(super) fn cpp_indexed_tuple_get_expected_sequence_type(
    receiver: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (receiver, value_target) = if let Some(receiver) = receiver.strip_suffix(".value()") {
        (receiver.trim(), true)
    } else {
        let receiver = receiver.strip_suffix(".error()")?;
        (receiver.trim(), false)
    };
    let (tuple_element, container_receiver) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let sequence_type = if value_target {
        cpp_standard_expected_target_type(&tuple_element)?
    } else {
        cpp_standard_expected_error_type(&tuple_element)?
    };
    Some((sequence_type.to_string(), container_receiver))
}

pub(super) fn cpp_standard_sequence_element_access_receiver(expression: &str) -> Option<&str> {
    cpp_subscript_receiver(expression).or_else(|| {
        [".front()", ".back()"]
            .into_iter()
            .find_map(|suffix| expression.strip_suffix(suffix).map(str::trim))
            .or_else(|| cpp_standard_sequence_at_receiver(expression))
    })
}

pub(super) fn cpp_indexed_tuple_get_weak_pointer_lock_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".lock()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_weak_pointer_target_type(&element_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}

pub(super) fn cpp_indexed_tuple_get_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    cpp_indexed_tuple_get_smart_pointer_receiver(receiver, byte_offset, local_bindings)
}

pub(super) fn cpp_indexed_tuple_get_reference_wrapper_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    let (element_type, _) =
        cpp_indexed_standard_get_element_binding(receiver, byte_offset, local_bindings)?;
    let target = cpp_standard_reference_wrapper_target_type(&element_type)?;
    Some((
        cpp_temporary_type_path(target)?,
        cpp_this_receiver_for_type(target, Some(false))?,
    ))
}
