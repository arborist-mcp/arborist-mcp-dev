mod extraction;
mod template_arguments;
mod type_normalization;

pub(super) use extraction::{
    cpp_standard_contiguous_sequence_element_type, cpp_standard_expected_error_type,
    cpp_standard_expected_target_type, cpp_standard_indexable_sequence_element_type,
    cpp_standard_indexed_element_type, cpp_standard_optional_target_type,
    cpp_standard_reference_wrapper_target_type, cpp_standard_sequence_element_type,
    cpp_standard_smart_pointer_target_type, cpp_standard_typed_get_element_type,
    cpp_standard_weak_pointer_target_type,
};

#[cfg(test)]
mod tests;
