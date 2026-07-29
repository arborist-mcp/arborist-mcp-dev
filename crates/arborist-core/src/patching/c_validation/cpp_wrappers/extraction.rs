use super::template_arguments::{
    cpp_first_template_argument, cpp_has_exactly_two_top_level_template_arguments,
    cpp_nth_template_argument, cpp_second_template_argument, cpp_standard_template_arguments,
    cpp_template_arguments_have_top_level_comma,
};
use super::type_normalization::normalized_cpp_typed_get_type;

pub(in crate::patching::c_validation) fn cpp_standard_smart_pointer_target_type(
    type_name: &str,
) -> Option<&str> {
    ["std::unique_ptr", "std::shared_ptr"]
        .into_iter()
        .find_map(|pointer_type| {
            cpp_standard_template_arguments(type_name, pointer_type)
                .and_then(cpp_first_template_argument)
        })
}

pub(in crate::patching::c_validation) fn cpp_standard_reference_wrapper_target_type(
    type_name: &str,
) -> Option<&str> {
    cpp_standard_template_arguments(type_name, "std::reference_wrapper")
        .filter(|arguments| !cpp_template_arguments_have_top_level_comma(arguments))
        .and_then(cpp_first_template_argument)
}

pub(in crate::patching::c_validation) fn cpp_standard_weak_pointer_target_type(
    type_name: &str,
) -> Option<&str> {
    cpp_standard_template_arguments(type_name, "std::weak_ptr")
        .filter(|arguments| !cpp_template_arguments_have_top_level_comma(arguments))
        .and_then(cpp_first_template_argument)
}

pub(in crate::patching::c_validation) fn cpp_standard_optional_target_type(
    type_name: &str,
) -> Option<&str> {
    cpp_standard_template_arguments(type_name, "std::optional")
        .filter(|arguments| !cpp_template_arguments_have_top_level_comma(arguments))
        .and_then(cpp_first_template_argument)
}

pub(in crate::patching::c_validation) fn cpp_standard_expected_target_type(
    type_name: &str,
) -> Option<&str> {
    let arguments = cpp_standard_template_arguments(type_name, "std::expected")?;
    cpp_has_exactly_two_top_level_template_arguments(arguments)
        .then(|| cpp_first_template_argument(arguments))?
}

pub(in crate::patching::c_validation) fn cpp_standard_expected_error_type(
    type_name: &str,
) -> Option<&str> {
    let arguments = cpp_standard_template_arguments(type_name, "std::expected")?;
    cpp_has_exactly_two_top_level_template_arguments(arguments)
        .then(|| cpp_second_template_argument(arguments))?
}

pub(in crate::patching::c_validation) fn cpp_standard_sequence_element_type(
    type_name: &str,
) -> Option<&str> {
    [
        "std::array",
        "std::deque",
        "std::list",
        "std::span",
        "std::vector",
    ]
    .into_iter()
    .find_map(|sequence_type| {
        cpp_standard_template_arguments(type_name, sequence_type)
            .and_then(cpp_first_template_argument)
    })
}

pub(in crate::patching::c_validation) fn cpp_standard_indexable_sequence_element_type(
    type_name: &str,
) -> Option<&str> {
    ["std::array", "std::deque", "std::span", "std::vector"]
        .into_iter()
        .find_map(|sequence_type| {
            cpp_standard_template_arguments(type_name, sequence_type)
                .and_then(cpp_first_template_argument)
        })
}

pub(in crate::patching::c_validation) fn cpp_standard_contiguous_sequence_element_type(
    type_name: &str,
) -> Option<&str> {
    ["std::array", "std::span", "std::vector"]
        .into_iter()
        .find_map(|sequence_type| {
            cpp_standard_template_arguments(type_name, sequence_type)
                .and_then(cpp_first_template_argument)
        })
}

pub(in crate::patching::c_validation) fn cpp_standard_indexed_element_type(
    type_name: &str,
    index: usize,
) -> Option<&str> {
    ["std::tuple", "std::pair", "std::variant"]
        .into_iter()
        .find_map(|tuple_type| {
            cpp_standard_template_arguments(type_name, tuple_type)
                .and_then(|arguments| cpp_nth_template_argument(arguments, index))
        })
}

pub(in crate::patching::c_validation) fn cpp_standard_typed_get_element_type<'a>(
    container_type: &'a str,
    requested_type: &str,
) -> Option<&'a str> {
    let arguments = ["std::tuple", "std::pair", "std::variant"]
        .into_iter()
        .find_map(|tuple_type| cpp_standard_template_arguments(container_type, tuple_type))?;
    let requested_type = normalized_cpp_typed_get_type(requested_type);
    let mut matching_element = None;
    let mut index = 0usize;
    while let Some(element_type) = cpp_nth_template_argument(arguments, index) {
        if normalized_cpp_typed_get_type(element_type) == requested_type {
            if matching_element.is_some() {
                return None;
            }
            matching_element = Some(element_type);
        }
        index += 1;
    }
    matching_element
}
