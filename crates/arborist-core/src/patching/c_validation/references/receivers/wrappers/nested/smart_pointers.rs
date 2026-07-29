use crate::patching::c_validation::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use crate::patching::c_validation::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use crate::patching::c_validation::cpp_wrappers::{
    cpp_standard_expected_target_type, cpp_standard_optional_target_type,
    cpp_standard_smart_pointer_target_type, cpp_standard_weak_pointer_target_type,
};
use crate::patching::c_validation::references::type_qualifiers::cpp_strip_leading_cv_qualifiers;
use crate::patching::c_validation::references::types::{CppLocalBinding, CppStandardUnwrap};

use super::super::super::binding_lookup::{
    cpp_local_binding_name_from_expression, cpp_visible_local_binding,
};
use super::{
    cpp_expected_error_nested_arrow_member_receiver,
    cpp_expected_error_optional_arrow_member_receiver,
    cpp_expected_error_optional_value_member_receiver, cpp_expected_local_binding_error_receiver,
    cpp_optional_wrapper_type_from_expression, cpp_standard_optional_value_member_receiver,
    cpp_standard_value_member_receiver, cpp_strip_expected_error_access,
};

pub(in crate::patching::c_validation::references) fn cpp_expected_weak_pointer_lock_receiver(
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

pub(in crate::patching::c_validation::references) fn cpp_optional_smart_pointer_arrow_member_receiver(
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

pub(in crate::patching::c_validation::references) fn cpp_expected_error_smart_pointer_arrow_member_receiver(
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

pub(in crate::patching::c_validation::references) fn cpp_smart_pointer_get_receiver(
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

pub(in crate::patching::c_validation::references) fn cpp_smart_pointer_dereference_receiver(
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

pub(in crate::patching::c_validation::references) fn cpp_smart_pointer_wrapper_type(
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
