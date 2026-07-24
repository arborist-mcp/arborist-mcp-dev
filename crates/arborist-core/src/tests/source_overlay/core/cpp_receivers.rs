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

#[test]
fn traces_cpp_indexable_sequence_element_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexable_sequence_elements.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int vector_index_caller(std::vector<Counter> current, int value) { return current[0].adjust(value); } int vector_nested_index_caller(std::vector<Counter> current, std::array<int, 1> indexes, int value) { return current[indexes[0]].adjust(value); } int span_index_caller(std::span<const Counter> current, int value) { return current[0].adjust(value); } int array_index_caller(std::array<Counter, 2> current, int value) { return current[1].adjust(value); } int const_deque_index_caller(const std::deque<Counter> current, int value) { return current[0].adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::vector_index_caller", "api::Counter::adjust(int) &"),
        (
            "api::vector_nested_index_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::span_index_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::array_index_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_deque_index_caller",
            "api::Counter::adjust(int) const &",
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

#[test]
fn traces_cpp_wrapped_sequence_receiver_categories_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_sequence_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_front_caller(std::vector<Counter> current, int value) { return std::move(current).front().adjust(value); } int const_back_caller(std::vector<Counter> current, int value) { return std::as_const(current).back().adjust(value); } int forwarded_subscript_caller(std::array<Counter, 2> current, int value) { return std::forward<std::array<Counter, 2>&&>(current)[0].adjust(value); } int moved_data_caller(std::span<Counter> current, int value) { return std::move(current).data()->adjust(value); } int const_data_caller(std::vector<Counter> current, int value) { return std::as_const(current).data()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_front_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_back_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_subscript_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::moved_data_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_data_caller",
            "api::Counter::adjust(int) const &",
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

#[test]
fn traces_cpp_contiguous_sequence_data_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("contiguous_sequence_data.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int inline_data_caller(std::vector<Counter> current, int value) { return current.data()->adjust(value); } int auto_data_caller(std::array<Counter, 2> current, int value) { auto pointer = current.data(); return pointer->adjust(value); } int decltype_auto_data_caller(std::vector<Counter> current, int value) { decltype(auto) pointer = current.data(); return pointer->adjust(value); } int const_span_data_caller(std::span<const Counter> current, int value) { auto pointer = current.data(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::inline_data_caller", "api::Counter::adjust(int) &"),
        ("api::auto_data_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_data_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_span_data_caller",
            "api::Counter::adjust(int) const &",
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

#[test]
fn traces_cpp_indexed_get_receiver_categories_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_get_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } int adjust(int value) const && { return value + 3; } }; int moved_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::move(current)).adjust(value); } int const_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::as_const(current)).adjust(value); } int forwarded_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(std::forward<std::tuple<Value, Counter>&&>(current)).adjust(value); } int decltype_auto_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(std::move(current)); return nested.adjust(value); } int decltype_auto_moved_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(std::move(current)); return std::move(nested).adjust(value); } int moved_optional_value_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(std::move(current)).value().adjust(value); } int moved_optional_arrow_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int moved_expected_value_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(std::move(current)).value().adjust(value); } int moved_expected_arrow_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int moved_expected_error_caller(std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(std::move(current)).error().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_get_caller", "api::Counter::adjust(int) &&"),
        ("api::const_get_caller", "api::Counter::adjust(int) const &"),
        ("api::forwarded_get_caller", "api::Counter::adjust(int) &&"),
        (
            "api::decltype_auto_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_moved_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_expected_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_error_caller",
            "api::Counter::adjust(int) &&",
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

#[test]
fn traces_cpp_direct_indexed_variant_get_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("direct_indexed_variant_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<1>(current).adjust(value); } int const_variant_get_caller(const std::variant<Counter, Value> current, int value) { return std::get<0>(current).adjust(value); } int direct_typed_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int const_typed_variant_get_caller(const std::variant<Counter, Value> current, int value) { return std::get<Counter>(current).adjust(value); } int typed_tuple_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int typed_unique_variant_get_caller(std::variant<Value, std::unique_ptr<Counter>> current, int value) { return std::get<std::unique_ptr<Counter>>(current)->adjust(value); } int typed_const_shared_variant_get_caller(std::variant<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<std::shared_ptr<const Counter>>(current)->adjust(value); } int typed_raw_pointer_variant_get_caller(std::variant<Value, Counter*> current, int value) { return std::get<Counter*>(current)->adjust(value); } int typed_const_reference_variant_get_caller(std::variant<std::reference_wrapper<const Counter>, Value> current, int value) { return std::get<std::reference_wrapper<const Counter>>(current).get().adjust(value); } int typed_weak_pointer_variant_get_caller(std::variant<Value, std::weak_ptr<Counter>> current, int value) { return std::get<std::weak_ptr<Counter>>(current).lock()->adjust(value); } int typed_optional_variant_get_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(current)->adjust(value); } int typed_const_expected_variant_get_caller(const std::variant<std::expected<Counter, Value>, Value> current, int value) { return std::get<std::expected<Counter, Value>>(current)->adjust(value); } int invalid_missing_typed_variant_get_caller(std::variant<Value, Counter> current, int value) { return std::get<std::unique_ptr<Counter>>(current)->adjust(value); } int invalid_duplicate_typed_tuple_get_caller(std::tuple<Counter, Counter> current, int value) { return std::get<Counter>(current).adjust(value); } int auto_variant_get_caller(std::variant<Value, Counter> current, int value) { auto nested = std::get<1>(current); return nested.adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::direct_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::direct_typed_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_typed_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::typed_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::typed_unique_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_shared_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_raw_pointer_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_reference_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_weak_pointer_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_optional_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_const_expected_variant_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_variant_get_caller",
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
    for caller in [
        "api::invalid_missing_typed_variant_get_caller",
        "api::invalid_duplicate_typed_tuple_get_caller",
    ] {
        let trace = trace_symbol_graph_from_index_with_source(
            &db_path,
            &source_path,
            source,
            caller,
            TraceDirection::Both,
        )
        .unwrap();
        assert!(trace.callees.is_empty(), "{caller}");
    }
}
