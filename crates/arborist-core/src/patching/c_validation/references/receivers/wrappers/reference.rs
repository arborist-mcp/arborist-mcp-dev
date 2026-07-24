use super::super::super::super::cpp_syntax::{
    cpp_receiver_call_argument, strip_cpp_outer_parentheses,
};
use super::super::super::super::cpp_types::CppThisMemberReceiver;
use super::super::super::std_get::*;
use super::super::super::types::{CppLocalBinding, CppMemberAccess, CppStandardUnwrap};
use super::super::binding_lookup::cpp_visible_local_binding;

pub(in super::super::super) fn cpp_standard_weak_pointer_lock_receiver(
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

pub(in super::super::super) fn cpp_standard_reference_factory_get_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = expression.strip_suffix(".get()")?.trim();
    cpp_standard_reference_factory_binding(receiver, byte_offset, local_bindings)
}

pub(in super::super::super) fn cpp_standard_reference_factory_binding(
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

pub(in super::super::super) fn cpp_reference_factory_argument_receiver(
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
