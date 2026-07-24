use super::super::super::super::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::super::super::cpp_wrappers::{
    cpp_standard_expected_error_type, cpp_standard_expected_target_type,
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_smart_pointer_target_type, cpp_standard_weak_pointer_target_type,
};
use super::super::super::std_get::*;
use super::super::super::type_qualifiers::*;
use super::super::super::types::{
    CppBindingType, CppLocalBinding, CppMemberAccess, CppStandardUnwrap,
};
use super::super::super::{
    cpp_expected_error_nested_arrow_member_receiver,
    cpp_expected_error_optional_arrow_member_receiver,
    cpp_expected_error_optional_dereference_receiver,
    cpp_expected_error_optional_value_member_receiver, cpp_expected_local_binding_error_receiver,
    cpp_expected_reference_wrapper_get_receiver, cpp_optional_local_binding_receiver,
    cpp_smart_pointer_dereference_receiver, cpp_smart_pointer_get_receiver,
    cpp_standard_optional_dereference_receiver, cpp_standard_optional_value_member_receiver,
    cpp_standard_reference_factory_get_receiver, cpp_standard_value_member_receiver,
    cpp_standard_wrapper_get_binding, cpp_strip_expected_error_access,
    cpp_strip_optional_value_access,
};

pub(super) fn cpp_auto_expected_error_copy_binding(
    expression: &str,
    type_prefix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<CppBindingType> {
    // Support both .error() and nested peels such as current->error() where the
    // optional around expected is first unwrapped by operator->.
    // For binding copies, peel only the layers made explicit by the initializer
    // so decltype(auto) nested = (*current.error()) keeps the expected unwrap
    // needed for later nested->member().
    let (type_name, _) = if let Some(receiver) = cpp_strip_expected_error_access(expression) {
        cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)?
    } else if let Some(receiver) = expression.strip_prefix('*').map(str::trim) {
        let receiver = strip_cpp_outer_parentheses(receiver);
        let error_receiver = cpp_strip_expected_error_access(receiver)?;
        let (type_name, error_receiver) =
            cpp_expected_local_binding_error_receiver(error_receiver, byte_offset, local_bindings)?;
        // *current.error() on a smart-pointer error peels the pointee. Do not keep
        // SmartPointer unwrap metadata; by-value auto should bind a plain object.
        if let Some(target) = cpp_standard_smart_pointer_target_type(&type_name) {
            return cpp_copied_standard_binding_type(target, type_prefix);
        }
        // Otherwise peel one optional/expected layer for forms such as
        // decltype(auto) nested = (*current.error()).
        cpp_standard_value_member_receiver(&type_name, error_receiver, true)
            .or(Some((type_name, error_receiver)))?
    } else if let Some((type_name, receiver)) =
        cpp_expected_error_nested_arrow_member_receiver(expression, byte_offset, local_bindings)
    {
        (type_name, receiver)
    } else if let Some((type_name, receiver)) =
        cpp_expected_error_optional_arrow_member_receiver(expression, byte_offset, local_bindings)
    {
        (type_name, receiver)
    } else {
        return None;
    };
    cpp_copied_standard_binding_type(&type_name, type_prefix)
}

pub(super) fn cpp_auto_standard_value_copy_binding(
    expression: &str,
    type_prefix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<CppBindingType> {
    // Binding copies must preserve intermediate wrappers. Local optional/expected
    // bindings already store one unwrapped layer, so plain `.value()` should not
    // peel again. `->value()` still needs both the operator-> peel and the value
    // peel, for example (*optional<optional<expected<T>>> )->value().
    let expression = strip_cpp_outer_parentheses(expression.trim());
    let (type_name, _) = if let Some(receiver) = cpp_strip_optional_value_access(expression) {
        let used_arrow = expression.ends_with("->value()");
        let (type_name, receiver) =
            cpp_optional_local_binding_receiver(receiver, byte_offset, local_bindings).or_else(
                || {
                    cpp_expected_error_optional_value_member_receiver(
                        expression,
                        byte_offset,
                        local_bindings,
                    )
                },
            )?;
        if used_arrow {
            let (type_name, receiver) =
                match cpp_standard_value_member_receiver(&type_name, receiver, true) {
                    Some(peeled) => peeled,
                    None => (type_name, receiver),
                };
            cpp_standard_value_member_receiver(&type_name, receiver, true)
                .or(Some((type_name, receiver)))?
        } else {
            (type_name, receiver)
        }
    } else if let Some((type_name, receiver)) =
        cpp_standard_optional_dereference_receiver(expression, byte_offset, local_bindings)
    {
        (type_name, receiver)
    } else if let Some((type_name, receiver)) =
        cpp_expected_error_optional_dereference_receiver(expression, byte_offset, local_bindings)
    {
        (type_name, receiver)
    } else {
        return None;
    };
    cpp_copied_standard_binding_type(&type_name, type_prefix)
}

pub(super) fn cpp_auto_expected_error_smart_pointer_binding(
    expression: &str,
    type_prefix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<CppBindingType> {
    if let Some((type_name, _)) =
        cpp_smart_pointer_dereference_receiver(expression, byte_offset, local_bindings)
    {
        // By-value auto copy of the pointee drops top-level const.
        return cpp_copied_standard_binding_type(&type_name, type_prefix);
    }
    let (type_name, receiver) =
        cpp_smart_pointer_get_receiver(expression, byte_offset, local_bindings).or_else(|| {
            cpp_typed_standard_get_expected_optional_smart_pointer_get_receiver(
                expression,
                byte_offset,
                local_bindings,
            )
        })?;
    // .get() yields a pointer; pointee constness is preserved under auto.
    let _ = type_prefix;
    Some((
        type_name,
        None,
        None,
        receiver,
        CppMemberAccess::Pointer,
        Some(CppStandardUnwrap::SmartPointer),
    ))
}

pub(super) fn cpp_auto_reference_wrapper_get_copy_binding(
    expression: &str,
    type_prefix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<CppBindingType> {
    let (type_name, _) =
        cpp_auto_reference_wrapper_get_alias_binding(expression, byte_offset, local_bindings)?;
    // By-value auto copy of the referenced object drops top-level const.
    cpp_copied_standard_binding_type(&type_name, type_prefix)
}

pub(super) fn cpp_copied_standard_binding_type(
    type_name: &str,
    type_prefix: &str,
) -> Option<CppBindingType> {
    let type_name = cpp_strip_leading_cv_qualifiers(type_name);
    let type_qualifiers = cpp_binding_type_qualifier_prefix(type_prefix);
    // Prefer concrete wrapper targets first so auto copies of nested wrappers such as
    // optional<unique_ptr<T>> keep usable unwrap metadata.
    if let Some(target) = cpp_standard_smart_pointer_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Pointer,
            Some(CppStandardUnwrap::SmartPointer),
        ));
    }
    if let Some(target) = cpp_standard_reference_wrapper_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::ReferenceWrapper),
        ));
    }
    if let Some(target) = cpp_standard_weak_pointer_target_type(type_name) {
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(target, Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::WeakPointer),
        ));
    }
    if let Some(target) = cpp_standard_optional_target_type(type_name) {
        // Recurse only into nested wrappers such as optional<unique_ptr<T>>.
        // Plain optional<T> must keep Optional unwrap metadata so later
        // error->member() / nested->member() still resolve.
        if let Some(inner) = cpp_copied_standard_binding_type(target, type_prefix)
            && inner.5.is_some()
        {
            return Some(inner);
        }
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            cpp_this_receiver_for_type(&format!("{type_qualifiers} {target}"), Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::Optional),
        ));
    }
    if let Some(target) = cpp_standard_expected_target_type(type_name) {
        let expected_error_type = cpp_standard_expected_error_type(type_name)?.to_string();
        if let Some(inner) = cpp_copied_standard_binding_type(target, type_prefix) {
            if inner.5.is_none() {
                return Some((
                    inner.0,
                    Some(expected_error_type.clone()),
                    cpp_this_receiver_for_type(
                        &format!("{type_qualifiers} {expected_error_type}"),
                        Some(false),
                    ),
                    inner.3,
                    inner.4,
                    Some(CppStandardUnwrap::Expected),
                ));
            }
            return Some(inner);
        }
        return Some((
            cpp_temporary_type_path(target)?,
            Some(expected_error_type.clone()),
            cpp_this_receiver_for_type(
                &format!("{type_qualifiers} {expected_error_type}"),
                Some(false),
            ),
            cpp_this_receiver_for_type(&format!("{type_qualifiers} {target}"), Some(false))?,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::Expected),
        ));
    }
    Some((
        cpp_temporary_type_path(type_name)?,
        None,
        None,
        cpp_this_receiver_for_type(&format!("{type_qualifiers} {type_name}"), Some(false))?,
        CppMemberAccess::Object,
        None,
    ))
}

pub(super) fn cpp_auto_optional_alias_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_auto_optional_alias_binding(argument, byte_offset, local_bindings);
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        return cpp_auto_optional_alias_binding(argument, byte_offset, local_bindings)
            .map(|(type_name, _)| (type_name, CppThisMemberReceiver::ConstLvalue));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let _ = cpp_auto_optional_alias_binding(argument, byte_offset, local_bindings)?;
        return Some((
            cpp_temporary_type_path(type_name)?,
            cpp_this_receiver_for_type(type_name, Some(true))?,
        ));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "static_cast") {
        let _ = cpp_auto_optional_alias_binding(argument, byte_offset, local_bindings)?;
        return Some((
            cpp_temporary_type_path(type_name)?,
            cpp_this_receiver_for_type(type_name, None)?,
        ));
    }
    cpp_standard_optional_value_member_receiver(expression, byte_offset, local_bindings)
        .or_else(|| {
            let receiver = cpp_strip_expected_error_access(expression)?;
            cpp_expected_local_binding_error_receiver(receiver, byte_offset, local_bindings)
        })
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

pub(super) fn cpp_auto_reference_wrapper_get_alias_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    if let Some(binding) =
        cpp_standard_reference_factory_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some(binding);
    }
    if let Some(binding) =
        cpp_expected_reference_wrapper_get_receiver(expression, byte_offset, local_bindings)
    {
        return Some(binding);
    }
    if let Some(binding) = cpp_typed_standard_get_expected_value_reference_wrapper_receiver(
        expression,
        byte_offset,
        local_bindings,
    ) {
        return Some(binding);
    }
    if let Some(binding) = cpp_typed_standard_get_expected_error_reference_wrapper_receiver(
        expression,
        byte_offset,
        local_bindings,
    ) {
        return Some(binding);
    }
    if let Some(binding) = cpp_typed_standard_get_expected_optional_reference_wrapper_receiver(
        expression,
        byte_offset,
        local_bindings,
    ) {
        return Some(binding);
    }
    let (binding, _) = cpp_standard_wrapper_get_binding(expression, byte_offset, local_bindings)?;
    (binding.access == CppMemberAccess::Object
        && binding.standard_unwrap == Some(CppStandardUnwrap::ReferenceWrapper))
    .then(|| (binding.type_name.clone(), binding.receiver))
}
