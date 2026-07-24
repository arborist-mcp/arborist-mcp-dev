use super::*;

#[test]
fn traces_cpp_member_calls_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; using Alias = Counter; int local_caller(int value) { Alias current{}; return current.adjust(value); } int postfix_const_caller(int value) { Alias const current{}; return current.adjust(value); } int static_caller(int value) { static Alias current{}; return current.adjust(value); } int auto_caller(int value) { auto current = Alias{}; return current.adjust(value); } int auto_direct_list_caller(int value) { auto current{Alias{}}; return current.adjust(value); } int deduced_pointer_caller(int value) { auto current = new Alias{}; return current->adjust(value); } int parenthesized_deduced_pointer_caller(int value) { auto current = new Alias(); return current->adjust(value); } int default_deduced_pointer_caller(int value) { auto current = new Alias; return current->adjust(value); } int pointee_const_deduced_pointer_caller(int value) { auto current = new const Alias{}; return current->adjust(value); } int postfix_pointee_const_deduced_pointer_caller(int value) { auto current = new Alias const{}; return current->adjust(value); } int make_unique_caller(int value) { auto current = std::make_unique<Alias>(); return current->adjust(value); } int make_shared_caller(int value) { auto current = std::make_shared<Alias>(); return current->adjust(value); } int unique_pointer_caller(int value) { std::unique_ptr<Alias> current; return current->adjust(value); } int shared_pointer_caller(int value) { std::shared_ptr<Alias> current; return current->adjust(value); } int const_unique_pointer_caller(int value) { std::unique_ptr<const Alias> current; return current->adjust(value); } int const_deduced_pointer_caller(int value) { const auto current = new Alias{}; return current->adjust(value); } int auto_pointer_caller(int value) { auto* current = new Alias{}; return current->adjust(value); } int const_auto_pointer_caller(int value) { const auto* current = new Alias{}; return current->adjust(value); } int const_auto_caller(int value) { const auto current = Alias{}; return current.adjust(value); } int parameter_caller(const Alias& current, int value) { return current.adjust(value); } int postfix_const_parameter_caller(Alias const& current, int value) { return current.adjust(value); } int rvalue_reference_caller(Alias&& current, int value) { return current.adjust(value); } int moved_rvalue_reference_caller(Alias&& current, int value) { return std::move(current).adjust(value); } int pointer_caller(Alias* current, int value) { return current->adjust(value); } int const_pointer_caller(Alias* const current, int value) { return current->adjust(value); } int postfix_const_pointer_caller(Alias const* current, int value) { return current->adjust(value); } int pointer_reference_caller(Alias* const& current, int value) { return current->adjust(value); } int dereference_caller(Alias* current, int value) { return (*current).adjust(value); } int range_caller() { for (Alias current : values) { return current.adjust(1); } return 0; } int moved_caller(Alias& current, int value) { return std::move(current).adjust(value); } }\n";

    for (caller, expected_callee) in [
        ("api::local_caller", "api::Counter::adjust(int) &"),
        (
            "api::postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::static_caller", "api::Counter::adjust(int) &"),
        ("api::auto_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_direct_list_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::deduced_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::parenthesized_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::default_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointee_const_deduced_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_pointee_const_deduced_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::make_unique_caller", "api::Counter::adjust(int) &"),
        ("api::make_shared_caller", "api::Counter::adjust(int) &"),
        ("api::unique_pointer_caller", "api::Counter::adjust(int) &"),
        ("api::shared_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_unique_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::auto_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_auto_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::parameter_caller", "api::Counter::adjust(int) const &"),
        (
            "api::postfix_const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::rvalue_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_rvalue_reference_caller",
            "api::Counter::adjust(int) &&",
        ),
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::const_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::postfix_const_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::pointer_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::range_caller", "api::Counter::adjust(int) &"),
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
    ] {
        let trace = trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            caller,
            TraceDirection::Both,
        )
        .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{caller}",
        );
    }

    assert!(
        fs::read_to_string(&source_path)
            .unwrap()
            .contains("return value")
    );
}

#[test]
fn traces_cpp_nested_standard_value_access_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int auto_value_caller(std::expected<Counter, int> current, int value) { auto current_value = current.value(); return current_value.adjust(value); } int const_auto_value_caller(std::expected<Counter, int> current, int value) { const auto current_value = current.value(); return current_value.adjust(value); } int copied_const_source_value_caller(const std::expected<Counter, int> current, int value) { auto current_value = current.value(); return current_value.adjust(value); } int nested_expected_value_caller(std::expected<std::expected<Counter, int>, int> current, int value) { return current.value().value().adjust(value); } int const_nested_expected_value_caller(const std::expected<std::expected<Counter, int>, int> current, int value) { return current.value().value().adjust(value); } int moved_nested_expected_value_caller(std::expected<std::expected<Counter, int>, int> current, int value) { return std::move(current).value().value().adjust(value); } int auto_nested_expected_value_caller(std::expected<std::expected<Counter, int>, int> current, int value) { auto current_value = current.value(); return current_value.value().adjust(value); } int auto_nested_expected_error_caller(std::expected<std::expected<int, Counter>, int> current, int value) { auto current_value = current.value(); return current_value.error().adjust(value); } int const_auto_nested_expected_error_caller(std::expected<std::expected<int, Counter>, int> current, int value) { const auto current_value = current.value(); return current_value.error().adjust(value); } int nested_optional_value_caller(std::expected<std::optional<Counter>, int> current, int value) { return current.value().value().adjust(value); } int const_nested_optional_value_caller(const std::expected<std::optional<Counter>, int> current, int value) { return current.value().value().adjust(value); } int moved_nested_optional_value_caller(std::expected<std::optional<Counter>, int> current, int value) { return std::move(current).value().value().adjust(value); } int auto_optional_value_caller(std::expected<std::optional<Counter>, int> current, int value) { auto current_value = current.value(); return current_value->adjust(value); } int const_auto_optional_value_caller(std::expected<std::optional<Counter>, int> current, int value) { const auto current_value = current.value(); return current_value->adjust(value); } int auto_pointer_value_caller(std::expected<std::shared_ptr<Counter>, int> current, int value) { auto current_value = current.value(); return current_value->adjust(value); } int get_copy_caller(std::expected<std::unique_ptr<Counter>, int> current, int value) { auto pointer = current.value().get(); return pointer->adjust(value); } int const_get_copy_caller(std::expected<std::shared_ptr<const Counter>, int> current, int value) { auto pointer = current.value().get(); return pointer->adjust(value); } int dereference_copy_caller(std::expected<std::unique_ptr<Counter>, int> current, int value) { auto target = *current.value(); return target.adjust(value); } int const_dereference_copy_caller(std::expected<std::shared_ptr<const Counter>, int> current, int value) { auto target = *current.value(); return target.adjust(value); } int dereference_alias_caller(std::expected<std::unique_ptr<Counter>, int> current, int value) { auto& target = *current.value(); return target.adjust(value); } int const_dereference_alias_caller(const std::expected<std::shared_ptr<Counter>, int> current, int value) { auto&& target = *current.value(); return target.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_expected_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_nested_expected_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_nested_expected_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::auto_nested_expected_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_nested_expected_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_nested_expected_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::nested_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_nested_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_nested_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::auto_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_pointer_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_copy_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
    ] {
        let trace = trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            caller,
            TraceDirection::Both,
        )
        .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{caller}",
        );
    }
}
