use crate::patching::c_validation::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use crate::patching::c_validation::cpp_types::{CppThisMemberReceiver, cpp_this_receiver_for_type};
use crate::patching::c_validation::cpp_wrappers::cpp_standard_optional_target_type;
use crate::patching::c_validation::references::type_qualifiers::cpp_strip_leading_cv_qualifiers;
use crate::patching::c_validation::references::types::{CppLocalBinding, CppStandardUnwrap};

use super::super::super::binding_lookup::cpp_visible_local_binding;
use super::{
    cpp_expected_error_optional_dereference_receiver,
    cpp_expected_error_optional_value_member_receiver, cpp_standard_value_member_receiver,
    cpp_strip_optional_value_access,
};

pub(in crate::patching::c_validation::references) fn cpp_standard_optional_value_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let used_arrow = expression.ends_with("->value()");
    let receiver = cpp_strip_optional_value_access(expression)?;
    let (type_name, receiver) =
        cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings)?;
    // `receiver->value()` first applies operator-> (one optional/expected peel)
    // and then value() (another peel). Keep both peels so nested forms such as
    // optional<optional<expected<T>>> resolve through (*current)->value().
    let (type_name, receiver) = if used_arrow {
        // Preserve the receiver value category so moved wrappers such as
        // std::move(current)->value() still select && overloads.
        match cpp_standard_value_member_receiver(&type_name, receiver, true) {
            Some(peeled) => peeled,
            None => (type_name, receiver),
        }
    } else {
        (type_name, receiver)
    };
    // A trailing .value()/->value() can still be the expected/optional value
    // access on the unwrapped target, for example (*optional).value().
    cpp_standard_value_member_receiver(&type_name, receiver, true).or(Some((type_name, receiver)))
}

pub(in crate::patching::c_validation::references) fn cpp_standard_optional_dereference_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let receiver = strip_cpp_outer_parentheses(expression.strip_prefix('*')?.trim());
    cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings)
}

pub(in crate::patching::c_validation::references) fn cpp_standard_optional_arrow_member_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let (type_name, receiver) =
        cpp_optional_local_binding_receiver(expression, byte_offset, local_bindings)?;
    // Optional/expected bindings store one unwrapped layer. Keep peeling while
    // the remaining type is still optional/expected so nested forms such as
    // optional<expected<optional<T>>> resolve through operator->.
    let mut type_name = type_name;
    let mut receiver = receiver;
    while let Some((next_type, next_receiver)) =
        cpp_standard_value_member_receiver(&type_name, receiver, false)
    {
        type_name = next_type;
        receiver = next_receiver;
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

pub(in crate::patching::c_validation::references) fn cpp_optional_wrapper_type_from_expression(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    cpp_standard_optional_value_member_receiver(expression, byte_offset, local_bindings)
        .or_else(|| {
            cpp_expected_error_optional_value_member_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
        })
        .or_else(|| {
            cpp_standard_optional_dereference_receiver(expression, byte_offset, local_bindings)
        })
        .or_else(|| {
            cpp_expected_error_optional_dereference_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
        })
}

pub(in crate::patching::c_validation::references) fn cpp_optional_local_binding_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && matches!(
            binding.standard_unwrap,
            Some(CppStandardUnwrap::Optional | CppStandardUnwrap::Expected)
        )
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if let Some(receiver) = expression.strip_prefix('*').map(str::trim) {
        // One dereference peels one optional/expected layer. Nested forms such as
        // **optional<optional<expected<T>>> need successive peels for each '*'.
        let (type_name, receiver) = cpp_optional_local_binding_receiver(
            strip_cpp_outer_parentheses(receiver),
            byte_offset,
            local_bindings,
        )?;
        return cpp_standard_value_member_receiver(&type_name, receiver, true)
            .or(Some((type_name, receiver)));
    }
    if let Some(receiver) = cpp_strip_optional_value_access(expression) {
        let (type_name, receiver) =
            cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings)?;
        return cpp_standard_value_member_receiver(&type_name, receiver, true);
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_optional_local_binding_receiver(argument, byte_offset, local_bindings).map(
            |(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Rvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstRvalue
                    }
                };
                (type_name, receiver)
            },
        );
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_optional_local_binding_receiver(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let (target_type, _) =
            cpp_optional_local_binding_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            target_type,
            cpp_this_receiver_for_type(type_name, Some(true))?,
        ));
    }
    None
}

pub(in crate::patching::c_validation::references) fn cpp_optional_wrapper_only_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && binding.standard_unwrap == Some(CppStandardUnwrap::Optional)
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    // expected/optional bindings store one unwrapped layer. When the remaining
    // type is still optional, keep peeling so nested wrappers stay available for
    // later .error() / operator-> resolution.
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && matches!(
            binding.standard_unwrap,
            Some(CppStandardUnwrap::Optional | CppStandardUnwrap::Expected)
        )
        && cpp_standard_optional_target_type(cpp_strip_leading_cv_qualifiers(&binding.type_name))
            .is_some()
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if let Some(receiver) = expression.strip_prefix('*').map(str::trim) {
        let (type_name, receiver) = cpp_optional_wrapper_only_receiver(
            strip_cpp_outer_parentheses(receiver),
            byte_offset,
            local_bindings,
        )?;
        return cpp_standard_value_member_receiver(&type_name, receiver, true)
            .or(Some((type_name, receiver)));
    }
    if let Some(receiver) = cpp_strip_optional_value_access(expression) {
        let (type_name, receiver) =
            cpp_optional_wrapper_only_receiver(receiver, byte_offset, local_bindings)?;
        return cpp_standard_value_member_receiver(&type_name, receiver, true)
            .or(Some((type_name, receiver)));
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_optional_wrapper_only_receiver(argument, byte_offset, local_bindings).map(
            |(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Rvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstRvalue
                    }
                };
                (type_name, receiver)
            },
        );
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_optional_wrapper_only_receiver(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let (target_type, _) =
            cpp_optional_wrapper_only_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            target_type,
            cpp_this_receiver_for_type(type_name, Some(true))?,
        ));
    }
    None
}
