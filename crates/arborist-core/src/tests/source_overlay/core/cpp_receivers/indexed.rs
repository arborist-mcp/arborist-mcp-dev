use super::*;

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
