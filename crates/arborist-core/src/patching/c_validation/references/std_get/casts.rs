use super::super::super::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::super::cpp_wrappers::cpp_standard_typed_get_element_type;
use super::super::types::{CppLocalBinding, CppMemberAccess};
use super::core::cpp_standard_get_container_binding;

pub(in super::super) fn cpp_smart_pointer_factory_type(expression: &str) -> Option<&str> {
    ["std::make_unique", "std::make_shared"]
        .into_iter()
        .find_map(|factory| {
            cpp_typed_receiver_call(expression, factory).map(|(type_name, _)| type_name)
        })
}

pub(in super::super) fn cpp_get_if_pointer_type(expression: &str) -> Option<&str> {
    // std::get_if<T>(...) yields T*. Treat the explicit template argument as the
    // pointee type for auto/auto* bindings and later nested->member() calls.
    cpp_typed_receiver_call(expression, "std::get_if").map(|(type_name, _)| type_name)
}

pub(in super::super) fn cpp_get_if_direct_pointer_receiver(
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

pub(in super::super) fn cpp_any_cast_direct_pointer_receiver(
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

pub(in super::super) fn cpp_direct_pointer_argument_binding<'a>(
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

pub(in super::super) fn cpp_direct_pointer_pointee_receiver(
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

pub(in super::super) fn cpp_pointer_cast_shared_pointer_type(expression: &str) -> Option<&str> {
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

pub(in super::super) fn cpp_any_cast_pointer_type(expression: &str) -> Option<&str> {
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

pub(in super::super) fn cpp_any_cast_value_type(expression: &str) -> Option<&str> {
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
