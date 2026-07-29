use super::super::super::super::cpp_syntax::strip_cpp_outer_parentheses;
use super::super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::super::super::cpp_wrappers::cpp_standard_reference_wrapper_target_type;
use super::super::super::std_get::*;
use super::super::super::types::CppLocalBinding;

mod expected;
mod helpers;
mod optional;
mod smart_pointers;

pub(in super::super::super) use expected::*;
pub(in super::super::super) use helpers::*;
pub(in super::super::super) use optional::*;
pub(in super::super::super) use smart_pointers::*;

pub(in super::super::super) fn cpp_standard_wrapper_get_binding<'a>(
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

pub(in super::super::super) fn cpp_expected_reference_wrapper_get_receiver(
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
