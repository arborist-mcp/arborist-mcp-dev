use super::super::super::cpp_syntax::{
    compact_cpp_expression, cpp_constructor_type_text, cpp_receiver_call_argument,
    cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::type_qualifiers::is_cpp_identifier;
use super::super::types::{CppLocalBinding, CppMemberAccess};

pub(in super::super) fn cpp_local_binding_name_from_expression(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if is_cpp_identifier(expression) {
        return Some(expression);
    }
    ["std::move", "std::as_const"]
        .into_iter()
        .find_map(|wrapper| {
            cpp_receiver_call_argument(expression, wrapper)
                .and_then(cpp_local_binding_name_from_expression)
        })
        .or_else(|| {
            cpp_typed_receiver_call(expression, "std::forward")
                .and_then(|(_, argument)| cpp_local_binding_name_from_expression(argument))
        })
}

pub(in super::super) fn cpp_addressable_local_object_receiver(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(binding) = cpp_visible_local_binding(expression, byte_offset, local_bindings)
        && binding.access == CppMemberAccess::Object
        && binding.standard_unwrap.is_none()
    {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings).map(
            |(type_name, receiver)| {
                let receiver = match receiver {
                    CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                        CppThisMemberReceiver::Lvalue
                    }
                    CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                        CppThisMemberReceiver::ConstLvalue
                    }
                };
                (type_name, receiver)
            },
        );
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            cpp_temporary_type_path(type_name)?,
            cpp_addressable_receiver(cpp_this_receiver_for_type(type_name, Some(true))?),
        ));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "static_cast") {
        cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings)?;
        return Some((
            cpp_temporary_type_path(type_name)?,
            cpp_addressable_receiver(cpp_this_receiver_for_type(type_name, None)?),
        ));
    }
    None
}

pub(in super::super) fn cpp_addressable_receiver(
    receiver: CppThisMemberReceiver,
) -> CppThisMemberReceiver {
    match receiver {
        CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
            CppThisMemberReceiver::Lvalue
        }
        CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
            CppThisMemberReceiver::ConstLvalue
        }
    }
}

pub(in super::super) fn cpp_addressable_local_binding_name(expression: &str) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let argument = cpp_receiver_call_argument(expression, "std::addressof")
        .or_else(|| expression.strip_prefix('&').map(str::trim))?;
    cpp_addressable_local_binding_name_from_expression(argument)
}

pub(in super::super) fn cpp_addressable_local_binding_name_from_expression(
    expression: &str,
) -> Option<&str> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if is_cpp_identifier(expression) {
        return Some(expression);
    }
    for wrapper in ["std::move", "std::as_const"] {
        if let Some(argument) = cpp_receiver_call_argument(expression, wrapper) {
            return cpp_addressable_local_binding_name_from_expression(argument);
        }
    }
    for function_name in ["std::forward", "static_cast"] {
        if let Some((_, argument)) = cpp_typed_receiver_call(expression, function_name) {
            return cpp_addressable_local_binding_name_from_expression(argument);
        }
    }
    None
}

pub(in super::super::super) fn cpp_visible_local_binding<'a>(
    name: &str,
    byte_offset: usize,
    local_bindings: &'a [CppLocalBinding],
) -> Option<&'a CppLocalBinding> {
    if !is_cpp_identifier(name) {
        return None;
    }
    local_bindings
        .iter()
        .filter(|binding| {
            binding.name == name
                && binding.declaration_start < byte_offset
                && (binding.scope_range.0..binding.scope_range.1).contains(&byte_offset)
        })
        .min_by_key(|binding| {
            (
                binding.scope_range.1.saturating_sub(binding.scope_range.0),
                usize::MAX.saturating_sub(binding.declaration_start),
            )
        })
}

pub(in super::super::super) fn cpp_temporary_type_from_expression(
    expression: &str,
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = expression.trim();
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_temporary_type_from_expression(strip_cpp_outer_parentheses(argument)).map(
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
    for function_name in ["static_cast", "std::forward"] {
        if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, function_name) {
            cpp_temporary_type_from_expression(strip_cpp_outer_parentheses(argument))?;
            return Some((
                cpp_temporary_type_path(type_name)?,
                cpp_this_receiver_for_type(type_name, Some(true))?,
            ));
        }
    }
    cpp_constructor_type_text(expression).map(|type_name| {
        (
            compact_cpp_expression(type_name),
            CppThisMemberReceiver::Rvalue,
        )
    })
}

pub(in super::super::super) fn cpp_this_receiver_from_expression(
    receiver: &str,
) -> Option<CppThisMemberReceiver> {
    let receiver = strip_cpp_outer_parentheses(receiver.trim());
    match compact_cpp_expression(receiver).as_str() {
        "this" | "*this" => return Some(CppThisMemberReceiver::Lvalue),
        _ => {}
    }

    if let Some(argument) = cpp_receiver_call_argument(receiver, "std::move") {
        return cpp_this_receiver_from_expression(argument).map(|receiver| match receiver {
            CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                CppThisMemberReceiver::Rvalue
            }
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                CppThisMemberReceiver::ConstRvalue
            }
        });
    }
    if let Some(argument) = cpp_receiver_call_argument(receiver, "std::as_const") {
        return cpp_this_receiver_from_expression(argument)
            .map(|_| CppThisMemberReceiver::ConstLvalue);
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(receiver, "static_cast") {
        cpp_this_receiver_from_expression(argument)?;
        return cpp_this_receiver_for_type(type_name, None);
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(receiver, "std::forward") {
        cpp_this_receiver_from_expression(argument)?;
        return cpp_this_receiver_for_type(type_name, Some(true));
    }
    None
}
