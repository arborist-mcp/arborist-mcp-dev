use super::super::cpp_types::{
    CppThisMemberReceiver, cpp_this_receiver_for_type, cpp_top_level_pointer_pointee,
};
use super::super::cpp_wrappers::{
    cpp_standard_expected_target_type, cpp_standard_optional_target_type,
    cpp_standard_reference_wrapper_target_type, cpp_standard_smart_pointer_target_type,
    cpp_standard_weak_pointer_target_type,
};
use super::types::CppStandardUnwrap;

pub(super) fn cpp_standard_wrapper_target_type(
    type_name: &str,
) -> Option<(&str, CppStandardUnwrap)> {
    cpp_standard_smart_pointer_target_type(type_name)
        .map(|target| (target, CppStandardUnwrap::SmartPointer))
        .or_else(|| {
            cpp_standard_weak_pointer_target_type(type_name)
                .map(|target| (target, CppStandardUnwrap::WeakPointer))
        })
        .or_else(|| {
            cpp_standard_reference_wrapper_target_type(type_name)
                .map(|target| (target, CppStandardUnwrap::ReferenceWrapper))
        })
        .or_else(|| {
            cpp_standard_optional_target_type(type_name)
                .map(|target| (target, CppStandardUnwrap::Optional))
        })
        .or_else(|| {
            cpp_standard_expected_target_type(type_name)
                .map(|target| (target, CppStandardUnwrap::Expected))
        })
}

pub(super) fn cpp_binding_type_prefix_is_supported(type_prefix: &str) -> bool {
    type_prefix.split_whitespace().all(|part| {
        matches!(
            part,
            "const"
                | "volatile"
                | "auto"
                | "register"
                | "static"
                | "thread_local"
                | "extern"
                | "mutable"
        )
    })
}

pub(super) fn cpp_auto_binding_type_is_supported(type_name: &str) -> bool {
    let mut parts = type_name.split_whitespace();
    let Some(first) = parts.next() else {
        return false;
    };
    first == "auto" && parts.all(|part| matches!(part, "const" | "volatile"))
}

pub(super) fn cpp_binding_type_qualifier_prefix(type_prefix: &str) -> String {
    type_prefix
        .split_whitespace()
        .filter(|part| matches!(*part, "const" | "volatile"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn cpp_pointer_declarator_suffix(type_suffix: &str) -> bool {
    let type_suffix = cpp_strip_cv_qualifiers(type_suffix);
    let Some(type_suffix) = type_suffix.strip_prefix('*') else {
        return false;
    };
    let qualifiers = cpp_strip_cv_qualifiers(type_suffix);
    let reference_count = qualifiers.chars().count();
    matches!(reference_count, 0..=2) && qualifiers.chars().all(|character| character == '&')
}

pub(super) fn cpp_object_declarator_suffix(type_suffix: &str) -> bool {
    let type_suffix = cpp_strip_cv_qualifiers(type_suffix);
    matches!(type_suffix, "" | "&" | "&&")
}

pub(super) fn cpp_strip_cv_qualifiers(mut type_suffix: &str) -> &str {
    loop {
        if let Some(remaining) = type_suffix.strip_prefix("const") {
            type_suffix = remaining;
        } else if let Some(remaining) = type_suffix.strip_prefix("volatile") {
            type_suffix = remaining;
        } else {
            return type_suffix;
        }
    }
}

pub(super) fn cpp_strip_leading_cv_qualifiers(mut type_name: &str) -> &str {
    loop {
        let trimmed = type_name.trim_start();
        if let Some(remaining) = trimmed.strip_prefix("const") {
            type_name = remaining;
        } else if let Some(remaining) = trimmed.strip_prefix("volatile") {
            type_name = remaining;
        } else {
            return trimmed;
        }
    }
}

pub(super) fn cpp_named_binding_receiver_for_type(
    type_name: &str,
) -> Option<CppThisMemberReceiver> {
    let type_name = type_name.trim_end().trim_end_matches('&').trim_end();
    cpp_this_receiver_for_type(type_name, Some(false))
}

pub(super) fn cpp_pointer_binding_receiver_for_type(
    type_name: &str,
) -> Option<CppThisMemberReceiver> {
    let pointee_type = cpp_top_level_pointer_pointee(type_name)?;
    cpp_this_receiver_for_type(pointee_type, Some(false))
}

pub(super) fn is_cpp_identifier(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some(character) if character.is_ascii_alphabetic() || character == '_')
        && characters.all(|character| character.is_ascii_alphanumeric() || character == '_')
}
