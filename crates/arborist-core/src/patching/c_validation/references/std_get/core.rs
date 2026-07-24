use super::super::super::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::super::cpp_wrappers::cpp_standard_typed_get_element_type;
use super::super::cpp_visible_local_binding;
use super::super::types::{CppLocalBinding, CppMemberAccess};
use super::super::{cpp_standard_sequence_at_receiver, cpp_subscript_receiver};

pub(in super::super) fn cpp_typed_standard_get_type(expression: &str) -> Option<&str> {
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

pub(in super::super) fn cpp_typed_standard_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (type_name, binding_receiver) =
        cpp_typed_standard_get_element_binding(expression, byte_offset, local_bindings)?;
    let receiver = cpp_standard_get_element_receiver(&type_name, binding_receiver, true)?;
    Some((cpp_temporary_type_path(&type_name)?, receiver))
}

pub(in super::super) fn cpp_standard_get_element_receiver(
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

pub(in super::super) fn cpp_typed_standard_get_element_binding(
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

pub(in super::super) fn cpp_standard_get_container_binding<'a>(
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

pub(in super::super) fn cpp_standard_sequence_element_access_receiver(
    expression: &str,
) -> Option<&str> {
    cpp_subscript_receiver(expression).or_else(|| {
        [".front()", ".back()"]
            .into_iter()
            .find_map(|suffix| expression.strip_suffix(suffix).map(str::trim))
            .or_else(|| cpp_standard_sequence_at_receiver(expression))
    })
}
