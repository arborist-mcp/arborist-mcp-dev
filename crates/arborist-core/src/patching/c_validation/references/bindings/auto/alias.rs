use super::super::super::super::cpp_syntax::{
    cpp_receiver_call_argument, cpp_typed_receiver_call, strip_cpp_outer_parentheses,
};
use super::super::super::super::cpp_types::{
    CppThisMemberReceiver, cpp_temporary_type_path, cpp_this_receiver_for_type,
};
use super::super::super::super::cpp_wrappers::{
    cpp_standard_optional_target_type, cpp_standard_reference_wrapper_target_type,
    cpp_standard_smart_pointer_target_type, cpp_standard_weak_pointer_target_type,
};
use super::super::super::type_qualifiers::*;
use super::super::super::types::{
    CppBindingType, CppLocalBinding, CppMemberAccess, CppStandardUnwrap,
};
use super::super::super::{
    cpp_addressable_local_binding_name, cpp_addressable_local_object_receiver,
    cpp_local_binding_name_from_expression, cpp_smart_pointer_dereference_receiver,
    cpp_visible_local_binding,
};

use super::copy::*;

pub(in super::super::super::super) fn cpp_address_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::to_address") {
        return cpp_to_address_binding(argument, byte_offset, local_bindings);
    }
    let argument = cpp_receiver_call_argument(expression, "std::addressof")
        .or_else(|| expression.strip_prefix('&').map(str::trim))?;
    cpp_addressable_local_object_receiver(argument, byte_offset, local_bindings)
}

pub(in super::super::super) fn cpp_to_address_binding(
    expression: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    let binding_name = cpp_local_binding_name_from_expression(expression)?;
    let binding = cpp_visible_local_binding(binding_name, byte_offset, local_bindings)?;
    if binding.access == CppMemberAccess::Pointer {
        return Some((binding.type_name.clone(), binding.receiver));
    }
    if binding.standard_unwrap == Some(CppStandardUnwrap::SmartPointer) {
        let target = cpp_standard_smart_pointer_target_type(&binding.type_name)?;
        return Some((
            cpp_temporary_type_path(target)?,
            cpp_this_receiver_for_type(target, Some(false))?,
        ));
    }
    None
}

pub(in super::super::super) fn cpp_auto_reference_alias_binding(
    expression: &str,
    type_prefix: &str,
    type_suffix: &str,
    byte_offset: usize,
    local_bindings: &[CppLocalBinding],
) -> Option<(String, CppThisMemberReceiver)> {
    if !matches!(cpp_strip_cv_qualifiers(type_suffix), "&" | "&&") {
        return None;
    }
    if let Some((type_name, receiver)) =
        cpp_smart_pointer_dereference_receiver(expression, byte_offset, local_bindings)
    {
        let receiver = if cpp_auto_reference_alias_is_const(type_prefix, type_suffix) {
            CppThisMemberReceiver::ConstLvalue
        } else {
            cpp_named_reference_alias_receiver(receiver)
        };
        return Some((type_name, receiver));
    }
    if let Some((type_name, receiver)) =
        cpp_auto_reference_wrapper_get_alias_binding(expression, byte_offset, local_bindings)
    {
        let receiver = if cpp_auto_reference_alias_is_const(type_prefix, type_suffix) {
            CppThisMemberReceiver::ConstLvalue
        } else {
            cpp_named_reference_alias_receiver(receiver)
        };
        return Some((type_name, receiver));
    }
    if let Some((type_name, receiver)) =
        cpp_auto_optional_alias_binding(expression, byte_offset, local_bindings)
    {
        let receiver = if cpp_auto_reference_alias_is_const(type_prefix, type_suffix) {
            CppThisMemberReceiver::ConstLvalue
        } else {
            cpp_named_reference_alias_receiver(receiver)
        };
        return Some((type_name, receiver));
    }
    let (type_name, binding, force_const, dereferenced_pointer) =
        cpp_auto_reference_alias_target_binding(expression, byte_offset, local_bindings)?;
    if (binding.standard_unwrap.is_some()
        && !(dereferenced_pointer
            && binding.standard_unwrap == Some(CppStandardUnwrap::SmartPointer)))
        || !(binding.access == CppMemberAccess::Object || dereferenced_pointer)
    {
        return None;
    }
    let receiver = if force_const || cpp_auto_reference_alias_is_const(type_prefix, type_suffix) {
        CppThisMemberReceiver::ConstLvalue
    } else {
        cpp_named_reference_alias_receiver(binding.receiver)
    };
    Some((type_name, receiver))
}

pub(in super::super::super) fn cpp_reference_alias_binding_type(
    type_name: &str,
    receiver: CppThisMemberReceiver,
) -> Option<CppBindingType> {
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
    if let Some(target) = cpp_standard_optional_target_type(type_name) {
        let receiver = match receiver {
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue => {
                CppThisMemberReceiver::ConstLvalue
            }
            CppThisMemberReceiver::Lvalue | CppThisMemberReceiver::Rvalue => {
                cpp_this_receiver_for_type(target, Some(false))?
            }
        };
        return Some((
            cpp_temporary_type_path(target)?,
            None,
            None,
            receiver,
            CppMemberAccess::Object,
            Some(CppStandardUnwrap::Optional),
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
    Some((
        type_name.to_string(),
        None,
        None,
        receiver,
        CppMemberAccess::Object,
        None,
    ))
}

pub(in super::super::super::super) fn cpp_named_reference_alias_receiver(
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

pub(in super::super::super) fn cpp_auto_reference_alias_target_binding<'a>(
    expression: &str,
    byte_offset: usize,
    local_bindings: &'a [CppLocalBinding],
) -> Option<(String, &'a CppLocalBinding, bool, bool)> {
    let expression = strip_cpp_outer_parentheses(expression.trim());
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::as_const") {
        let (type_name, binding, _, dereferenced_pointer) =
            cpp_auto_reference_alias_target_binding(argument, byte_offset, local_bindings)?;
        return Some((type_name, binding, true, dereferenced_pointer));
    }
    if let Some(argument) = cpp_receiver_call_argument(expression, "std::move") {
        return cpp_auto_reference_alias_target_binding(argument, byte_offset, local_bindings);
    }
    if let Some((forwarded_type, argument)) = cpp_typed_receiver_call(expression, "std::forward") {
        let (_, binding, _, dereferenced_pointer) =
            cpp_auto_reference_alias_target_binding(argument, byte_offset, local_bindings)?;
        let receiver = cpp_this_receiver_for_type(forwarded_type, Some(true))?;
        let force_const = matches!(
            receiver,
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
        );
        return Some((
            cpp_temporary_type_path(forwarded_type)?,
            binding,
            force_const,
            dereferenced_pointer,
        ));
    }
    if let Some((type_name, argument)) = cpp_typed_receiver_call(expression, "static_cast") {
        let (_, binding, _, dereferenced_pointer) =
            cpp_auto_reference_alias_target_binding(argument, byte_offset, local_bindings)?;
        let receiver = cpp_this_receiver_for_type(type_name, None)?;
        let force_const = matches!(
            receiver,
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
        );
        return Some((
            cpp_temporary_type_path(type_name)?,
            binding,
            force_const,
            dereferenced_pointer,
        ));
    }
    if let Some(pointer_name) = expression.strip_prefix('*').map(str::trim)
        && let Some(binding) = cpp_visible_local_binding(pointer_name, byte_offset, local_bindings)
        && binding.access == CppMemberAccess::Pointer
        && matches!(
            binding.standard_unwrap,
            None | Some(CppStandardUnwrap::SmartPointer)
        )
    {
        return Some((binding.type_name.clone(), binding, false, true));
    }
    if let Some(pointer_expression) = expression.strip_prefix('*').map(str::trim)
        && let Some((type_name, receiver)) =
            cpp_address_binding(pointer_expression, byte_offset, local_bindings)
        && let Some(binding) = cpp_visible_local_binding(
            cpp_addressable_local_binding_name(pointer_expression)?,
            byte_offset,
            local_bindings,
        )
    {
        let force_const = matches!(
            receiver,
            CppThisMemberReceiver::ConstLvalue | CppThisMemberReceiver::ConstRvalue
        );
        return Some((type_name, binding, force_const, true));
    }
    cpp_visible_local_binding(expression, byte_offset, local_bindings)
        .map(|binding| (binding.type_name.clone(), binding, false, false))
}

pub(in super::super::super) fn cpp_auto_reference_alias_is_const(
    type_prefix: &str,
    type_suffix: &str,
) -> bool {
    type_prefix.split_whitespace().any(|part| part == "const")
        || cpp_declarator_suffix_has_const_qualifier(type_suffix)
}

pub(in super::super::super) fn cpp_declarator_suffix_has_const_qualifier(
    mut type_suffix: &str,
) -> bool {
    loop {
        if type_suffix.strip_prefix("const").is_some() {
            return true;
        }
        if let Some(remaining) = type_suffix.strip_prefix("volatile") {
            type_suffix = remaining;
        } else {
            return false;
        }
    }
}
