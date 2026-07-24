use super::super::super::cpp_syntax::cpp_typed_receiver_call;
use super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
    cpp_top_level_pointer_pointee,
};
use super::super::super::cpp_wrappers::{
    cpp_standard_contiguous_sequence_element_type, cpp_standard_expected_error_type,
    cpp_standard_expected_target_type, cpp_standard_indexed_element_type,
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_sequence_element_type, cpp_standard_smart_pointer_target_type,
    cpp_standard_weak_pointer_target_type,
};
use super::super::types::{CppLocalBinding, CppMemberAccess};
use super::core::{
    cpp_standard_get_container_binding, cpp_standard_get_element_receiver,
    cpp_standard_sequence_element_access_receiver,
};

pub(in super::super) fn cpp_indexed_tuple_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (element_type, container_receiver) =
        cpp_indexed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let receiver = cpp_standard_get_element_receiver(&element_type, container_receiver, true)?;
    Some((cpp_temporary_type_path(&element_type)?, receiver))
}

pub(in super::super) fn cpp_indexed_standard_get_element_binding(
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

pub(in super::super) fn cpp_indexed_tuple_get_smart_pointer_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_raw_pointer_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_optional_value_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_optional_arrow_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_optional_smart_pointer_arrow_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_value_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_value_smart_pointer_arrow_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_value_raw_pointer_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_optional_smart_pointer_arrow_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_optional_raw_pointer_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_value_smart_pointer_get_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_arrow_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_smart_pointer_arrow_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_error_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_error_smart_pointer_arrow_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_error_raw_pointer_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_error_smart_pointer_get_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_value_weak_pointer_lock_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_value_reference_wrapper_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_error_reference_wrapper_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_error_weak_pointer_lock_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_optional_weak_pointer_lock_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_optional_reference_wrapper_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_optional_smart_pointer_get_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_sequence_element_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_sequence_data_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_expected_sequence_type(
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

pub(in super::super) fn cpp_indexed_tuple_get_weak_pointer_lock_receiver(
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

pub(in super::super) fn cpp_indexed_tuple_get_smart_pointer_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    cpp_indexed_tuple_get_smart_pointer_receiver(receiver, byte_offset, local_bindings)
}

pub(in super::super) fn cpp_indexed_tuple_get_reference_wrapper_receiver(
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
