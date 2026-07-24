use super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
    cpp_top_level_pointer_pointee,
};
use super::super::super::cpp_wrappers::{
    cpp_standard_contiguous_sequence_element_type, cpp_standard_expected_error_type,
    cpp_standard_expected_target_type, cpp_standard_optional_target_type,
    cpp_standard_reference_wrapper_target_type, cpp_standard_sequence_element_type,
    cpp_standard_smart_pointer_target_type, cpp_standard_weak_pointer_target_type,
};
use super::super::types::CppLocalBinding;
use super::core::{
    cpp_standard_get_element_receiver, cpp_standard_sequence_element_access_receiver,
    cpp_typed_standard_get_element_binding, cpp_typed_standard_get_receiver,
};

pub(in super::super) fn cpp_typed_standard_get_smart_pointer_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    cpp_typed_standard_get_smart_pointer_receiver(receiver, byte_offset, local_bindings)
}

pub(in super::super) fn cpp_typed_standard_get_optional_arrow_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_optional_smart_pointer_arrow_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_arrow_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_smart_pointer_arrow_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_value_smart_pointer_get_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_error_smart_pointer_get_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_value_weak_pointer_lock_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_error_weak_pointer_lock_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_value_reference_wrapper_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_error_reference_wrapper_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_optional_target(
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

pub(in super::super) fn cpp_typed_standard_get_expected_optional_smart_pointer_arrow_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_optional_weak_pointer_lock_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_optional_reference_wrapper_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_optional_smart_pointer_get_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_sequence_type(
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

pub(in super::super) fn cpp_typed_standard_get_expected_sequence_element_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_sequence_data_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_optional_value_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_value_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_expected_error_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_raw_pointer_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_weak_pointer_lock_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_reference_wrapper_receiver(
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
