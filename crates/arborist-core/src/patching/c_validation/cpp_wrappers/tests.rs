use super::{
    cpp_standard_contiguous_sequence_element_type, cpp_standard_expected_error_type,
    cpp_standard_expected_target_type, cpp_standard_indexable_sequence_element_type,
    cpp_standard_indexed_element_type, cpp_standard_optional_target_type,
    cpp_standard_reference_wrapper_target_type, cpp_standard_sequence_element_type,
    cpp_standard_smart_pointer_target_type, cpp_standard_typed_get_element_type,
};

#[test]
fn extracts_standard_wrapper_target_types() {
    assert_eq!(
        cpp_standard_smart_pointer_target_type("std::unique_ptr<Wrapper<Alias, Tag>, Deleter>"),
        Some("Wrapper<Alias, Tag>")
    );
    assert_eq!(
        cpp_standard_smart_pointer_target_type("std::shared_ptr<const Counter>"),
        Some("const Counter")
    );
    assert!(cpp_standard_smart_pointer_target_type("std::unique_ptr<>").is_none());
    assert!(cpp_standard_smart_pointer_target_type("std::shared_ptr<Counter> trailing").is_none());

    assert_eq!(
        cpp_standard_reference_wrapper_target_type("std::reference_wrapper<const Counter>"),
        Some("const Counter")
    );
    assert!(cpp_standard_reference_wrapper_target_type("std::reference_wrapper<>").is_none());
    assert!(
        cpp_standard_reference_wrapper_target_type("std::reference_wrapper<Counter, Tag>")
            .is_none()
    );

    assert_eq!(
        cpp_standard_optional_target_type("std::optional<const Wrapper<Counter, Tag>>"),
        Some("const Wrapper<Counter, Tag>")
    );
    assert!(cpp_standard_optional_target_type("std::optional<>").is_none());
    assert!(cpp_standard_optional_target_type("std::optional<Counter> trailing").is_none());
    assert!(cpp_standard_optional_target_type("std::optional<Counter, Tag>").is_none());
}

#[test]
fn extracts_standard_expected_value_and_error_types() {
    assert_eq!(
        cpp_standard_expected_target_type("std::expected<Counter, Error>"),
        Some("Counter")
    );
    assert_eq!(
        cpp_standard_expected_target_type("std::expected<std::vector<int>, Error>"),
        Some("std::vector<int>")
    );
    assert_eq!(
        cpp_standard_expected_target_type("std::expected<Counter, void (*)(int, int)>"),
        Some("Counter")
    );
    assert_eq!(
        cpp_standard_expected_error_type("std::expected<Counter, Error>"),
        Some("Error")
    );
    assert_eq!(
        cpp_standard_expected_error_type("std::expected<Counter, void (*)(int, int)>"),
        Some("void (*)(int, int)")
    );
    assert_eq!(
        cpp_standard_expected_error_type("std::expected<Value, Wrapper<Error, Tag>>"),
        Some("Wrapper<Error, Tag>")
    );
    assert!(cpp_standard_expected_target_type("std::expected<Counter>").is_none());
    assert!(cpp_standard_expected_error_type("std::expected<Counter>").is_none());
    assert!(cpp_standard_expected_target_type("std::expected<Counter, >").is_none());
    assert!(cpp_standard_expected_target_type("std::expected<Counter, Error, Extra>").is_none());
}

#[test]
fn extracts_standard_sequence_element_types() {
    assert_eq!(
        cpp_standard_sequence_element_type("std::vector<Wrapper<Alias, Tag>>"),
        Some("Wrapper<Alias, Tag>")
    );
    assert_eq!(
        cpp_standard_sequence_element_type("std::span<const Counter, 4>"),
        Some("const Counter")
    );
    assert_eq!(
        cpp_standard_sequence_element_type("std::array<Counter, 2>"),
        Some("Counter")
    );
    assert!(cpp_standard_sequence_element_type("std::vector<>").is_none());
    assert!(cpp_standard_sequence_element_type("std::set<Counter>").is_none());
    assert_eq!(
        cpp_standard_indexable_sequence_element_type("std::vector<Counter>"),
        Some("Counter")
    );
    assert_eq!(
        cpp_standard_indexable_sequence_element_type("std::span<const Counter, 4>"),
        Some("const Counter")
    );
    assert!(cpp_standard_indexable_sequence_element_type("std::list<Counter>").is_none());
    assert_eq!(
        cpp_standard_contiguous_sequence_element_type("std::array<Counter, 2>"),
        Some("Counter")
    );
    assert!(cpp_standard_contiguous_sequence_element_type("std::deque<Counter>").is_none());
}

#[test]
fn extracts_standard_indexed_element_types() {
    assert_eq!(
        cpp_standard_indexed_element_type("std::tuple<Counter, Wrapper<Alias, Tag>, int>", 1),
        Some("Wrapper<Alias, Tag>")
    );
    assert_eq!(
        cpp_standard_indexed_element_type("std::pair<const Counter, Value>", 0),
        Some("const Counter")
    );
    assert_eq!(
        cpp_standard_indexed_element_type("std::tuple<Value, Counter*>", 1),
        Some("Counter*")
    );
    assert_eq!(
        cpp_standard_indexed_element_type("std::variant<Value, Wrapper<Alias, Tag>>", 1),
        Some("Wrapper<Alias, Tag>")
    );
    assert_eq!(
        cpp_standard_indexed_element_type("std::pair<Counter, Value>", 2),
        None
    );
    assert_eq!(
        cpp_standard_indexed_element_type("std::vector<Counter>", 0),
        None
    );
    assert_eq!(
        cpp_standard_typed_get_element_type(
            "std::tuple<Value, Wrapper<Alias, Tag>>",
            "Wrapper<Alias, Tag>"
        ),
        Some("Wrapper<Alias, Tag>")
    );
    assert_eq!(
        cpp_standard_typed_get_element_type(
            "std::variant<Value, std::shared_ptr< const Counter >>",
            "std::shared_ptr<const Counter>"
        ),
        Some("std::shared_ptr< const Counter >")
    );
    assert_eq!(
        cpp_standard_typed_get_element_type("std::variant<Value, Counter const>", "const Counter"),
        Some("Counter const")
    );
    assert_eq!(
        cpp_standard_typed_get_element_type(
            "std::tuple<Value, volatile Wrapper<const Counter>>",
            "Wrapper<const Counter> volatile"
        ),
        Some("volatile Wrapper<const Counter>")
    );
    assert_eq!(
        cpp_standard_typed_get_element_type("std::tuple<Value, const Counter*>", "Counter const*"),
        Some("const Counter*")
    );
    assert_eq!(
        cpp_standard_typed_get_element_type(
            "std::variant<Value, Counter const * const>",
            "const Counter* const"
        ),
        Some("Counter const * const")
    );
    assert!(
        cpp_standard_typed_get_element_type("std::tuple<Value, const Counter*>", "Counter* const")
            .is_none()
    );
    assert!(cpp_standard_typed_get_element_type("std::pair<Value, Counter>", "Missing").is_none());
    assert!(cpp_standard_typed_get_element_type("std::tuple<Value, Value>", "Value").is_none());
    assert!(cpp_standard_typed_get_element_type("std::vector<Counter>", "Counter").is_none());
}
