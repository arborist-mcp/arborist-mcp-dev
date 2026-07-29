use super::super::super::super::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::super::super::cpp_wrappers::{
    cpp_standard_expected_error_type, cpp_standard_expected_target_type,
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_smart_pointer_target_type,
};
use super::super::super::std_get::*;
use super::super::super::type_qualifiers::*;
use super::super::super::types::{CppLocalBinding, CppStandardUnwrap};
use super::super::binding_lookup::cpp_visible_local_binding;

mod helpers;
mod optional;
mod smart_pointers;

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

pub(in super::super::super) fn cpp_standard_expected_error_member_receiver(
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

pub(in super::super::super) fn cpp_expected_error_nested_arrow_member_receiver(
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

pub(in super::super::super) fn cpp_expected_error_optional_arrow_member_receiver(
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

pub(in super::super::super) fn cpp_expected_error_optional_value_member_receiver(
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

pub(in super::super::super) fn cpp_expected_error_optional_dereference_receiver(
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

pub(in super::super::super) fn cpp_expected_error_type_from_wrapper(
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

pub(in super::super::super) fn cpp_expected_local_binding_error_receiver(
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
