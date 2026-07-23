use super::super::super::cpp_types::{CppThisMemberReceiver, cpp_temporary_type_path};
use super::super::super::cpp_wrappers::{
    cpp_standard_contiguous_sequence_element_type, cpp_standard_indexable_sequence_element_type,
    cpp_standard_sequence_element_type,
};
use super::super::std_get::{
    cpp_standard_get_container_binding, cpp_standard_get_element_receiver,
};
use super::super::types::{CppLocalBinding, CppMemberAccess};

pub(in super::super) fn cpp_standard_sequence_element_receiver(
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

pub(in super::super::super) fn cpp_standard_sequence_at_receiver(expression: &str) -> Option<&str> {
    let receiver = expression.strip_suffix(')')?.trim_end();
    let opening = receiver.rfind(".at(")?;
    let arguments = &receiver[opening + ".at(".len()..];
    (!arguments.is_empty()).then_some(receiver[..opening].trim())
}

pub(in super::super) fn cpp_standard_indexable_sequence_element_receiver(
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

pub(in super::super) fn cpp_standard_sequence_data_receiver(
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

pub(in super::super::super) fn cpp_subscript_receiver(expression: &str) -> Option<&str> {
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
