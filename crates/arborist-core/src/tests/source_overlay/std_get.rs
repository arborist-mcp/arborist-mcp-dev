use super::*;

#[test]
fn traces_cpp_get_if_pointer_bindings_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("get_if_pointer_bindings.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int auto_get_if_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get_if<Counter>(&current); return nested->adjust(value); } int decltype_auto_get_if_caller(std::variant<Counter, Value> current, int value) { decltype(auto) nested = std::get_if<Counter>(&current); return nested->adjust(value); } int auto_const_get_if_caller(const std::variant<Counter, Value> current, int value) { auto nested = std::get_if<const Counter>(&current); return nested->adjust(value); } int auto_dynamic_pointer_cast_caller(std::shared_ptr<Value> current, int value) { auto nested = std::dynamic_pointer_cast<Counter>(current); return nested->adjust(value); } int decltype_auto_dynamic_pointer_cast_caller(std::shared_ptr<Value> current, int value) { decltype(auto) nested = std::dynamic_pointer_cast<Counter>(current); return nested->adjust(value); } int auto_static_pointer_cast_caller(std::shared_ptr<Value> current, int value) { auto nested = std::static_pointer_cast<Counter>(current); return nested->adjust(value); } int auto_const_pointer_cast_caller(std::shared_ptr<const Counter> current, int value) { auto nested = std::const_pointer_cast<Counter>(current); return nested->adjust(value); } int auto_any_cast_pointer_caller(std::any current, int value) { auto nested = std::any_cast<Counter>(&current); return nested->adjust(value); } int auto_any_cast_value_caller(std::any current, int value) { auto nested = std::any_cast<Counter>(current); return nested.adjust(value); } int decltype_auto_any_cast_value_caller(std::any current, int value) { decltype(auto) nested = std::any_cast<Counter>(current); return nested.adjust(value); } int auto_variant_get_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get<Counter>(current); return nested.adjust(value); } int decltype_auto_variant_get_caller(std::variant<Counter, Value> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); } int auto_get_if_then_member_caller(std::variant<std::unique_ptr<Counter>, Value> current, int value) { auto nested = std::get_if<std::unique_ptr<Counter>>(&current); return (*nested)->adjust(value); } int decltype_auto_get_if_unique_caller(std::variant<std::unique_ptr<Counter>, Value> current, int value) { decltype(auto) nested = std::get_if<std::unique_ptr<Counter>>(&current); return (*nested)->adjust(value); } int direct_to_address_raw_caller(Counter* current, int value) { return std::to_address(current)->adjust(value); } int auto_to_address_raw_caller(Counter* current, int value) { auto nested = std::to_address(current); return nested->adjust(value); } int decltype_auto_to_address_smart_caller(std::unique_ptr<Counter> current, int value) { decltype(auto) nested = std::to_address(current); return nested->adjust(value); } int auto_to_address_const_smart_caller(std::unique_ptr<const Counter> current, int value) { auto nested = std::to_address(current); return nested->adjust(value); } int vector_front_caller(std::vector<Counter> current, int value) { return current.front().adjust(value); } int vector_back_caller(std::vector<Counter> current, int value) { return current.back().adjust(value); } int array_at_caller(std::array<Counter, 2> current, int value) { return current.at(0).adjust(value); } int span_const_front_caller(std::span<const Counter> current, int value) { return current.front().adjust(value); } int const_vector_back_caller(const std::vector<Counter> current, int value) { return current.back().adjust(value); } int auto_tuple_get_caller(std::tuple<Value, Counter> current, int value) { auto nested = std::get<1>(current); return nested.adjust(value); } int decltype_auto_tuple_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(current); return nested.adjust(value); } int auto_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { auto nested = std::get<0>(current); return nested.adjust(value); } int decltype_auto_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { decltype(auto) nested = std::get<0>(current); return nested.adjust(value); } int auto_tuple_get_unique_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { auto nested = std::get<1>(current); return nested->adjust(value); } int decltype_auto_tuple_get_unique_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { decltype(auto) nested = std::get<1>(current); return nested->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::auto_get_if_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_get_if_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_get_if_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_dynamic_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_dynamic_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_static_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_pointer_cast_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_any_cast_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_any_cast_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_any_cast_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_variant_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_get_if_then_member_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_get_if_unique_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::direct_to_address_raw_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_to_address_raw_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_to_address_smart_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_to_address_const_smart_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::vector_front_caller", "api::Counter::adjust(int) &"),
        ("api::vector_back_caller", "api::Counter::adjust(int) &"),
        ("api::array_at_caller", "api::Counter::adjust(int) &"),
        (
            "api::span_const_front_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_vector_back_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::auto_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_pair_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_const_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_tuple_get_unique_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_tuple_get_unique_caller",
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
fn traces_cpp_direct_indexed_tuple_get_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("direct_indexed_tuple_get.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_tuple_get_caller(std::tuple<Value, Counter> current, int value) { return std::get<1>(current).adjust(value); } int direct_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { return std::get<0>(current).adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::direct_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::direct_const_pair_get_caller",
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
fn traces_cpp_direct_indexed_tuple_get_smart_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("direct_indexed_tuple_get_smart_pointer.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_indexed_tuple_get_smart_pointer_get_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(current).get()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<0>(current).get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
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
            "{caller}"
        );
    }
}

#[test]
fn traces_cpp_typed_get_standard_value_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("typed_get_standard_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } int adjust(int value) const && { return value + 3; } }; int optional_value_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(current).value().adjust(value); } int expected_value_caller(std::variant<Value, std::expected<Counter, Value>> current, int value) { return std::get<std::expected<Counter, Value>>(current).value().adjust(value); } int const_expected_error_caller(const std::variant<Value, std::expected<Value, Counter>> current, int value) { return std::get<std::expected<Value, Counter>>(current).error().adjust(value); } int moved_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::move(current)).adjust(value); } int const_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::as_const(current)).adjust(value); } int forwarded_typed_get_caller(std::variant<Value, Counter> current, int value) { return std::get<Counter>(std::forward<std::variant<Value, Counter>&&>(current)).adjust(value); } int moved_optional_value_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(std::move(current)).value().adjust(value); } int moved_expected_error_caller(std::variant<Value, std::expected<Value, Counter>> current, int value) { return std::get<std::expected<Value, Counter>>(std::move(current)).error().adjust(value); } int moved_optional_arrow_caller(std::variant<Value, std::optional<Counter>> current, int value) { return std::get<std::optional<Counter>>(std::move(current))->adjust(value); } int moved_expected_arrow_caller(std::variant<Value, std::expected<Counter, Value>> current, int value) { return std::get<std::expected<Counter, Value>>(std::move(current))->adjust(value); } int optional_unique_caller(std::variant<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<std::optional<std::unique_ptr<Counter>>>(current)->adjust(value); } int expected_const_shared_caller(std::variant<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<std::expected<std::shared_ptr<const Counter>, Value>>(current)->adjust(value); } int shared_get_caller(std::variant<Value, std::shared_ptr<Counter>> current, int value) { return std::get<std::shared_ptr<Counter>>(current).get()->adjust(value); } int const_shared_get_caller(std::variant<std::shared_ptr<const Counter>, Value> current, int value) { return std::get<std::shared_ptr<const Counter>>(current).get()->adjust(value); } int moved_expected_value_get_caller(std::variant<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<std::expected<std::unique_ptr<Counter>, Value>>(std::move(current)).value().get()->adjust(value); } int const_expected_error_get_caller(std::variant<Value, std::expected<Value, std::shared_ptr<const Counter>>> current, int value) { return std::get<std::expected<Value, std::shared_ptr<const Counter>>>(std::as_const(current)).error().get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::optional_value_caller", "api::Counter::adjust(int) &"),
        ("api::expected_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_expected_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_typed_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::const_typed_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_typed_get_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_expected_error_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_expected_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::optional_unique_caller", "api::Counter::adjust(int) &"),
        (
            "api::expected_const_shared_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::shared_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_expected_value_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_expected_error_get_caller",
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
fn traces_cpp_typed_get_top_level_cv_spellings_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("typed_get_top_level_cv.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int postfix_const_caller(std::variant<Value, Counter const> current, int value) { return std::get<const Counter>(current).adjust(value); } int postfix_volatile_caller(std::variant<Value, volatile Counter> current, int value) { return std::get<Counter volatile>(current).adjust(value); } int get_if_postfix_const_caller(std::variant<Value, Counter const> current, int value) { return std::get_if<const Counter>(&current)->adjust(value); } int get_if_postfix_volatile_caller(std::variant<Value, volatile Counter> current, int value) { return std::get_if<Counter volatile>(&current)->adjust(value); } int pointer_const_caller(std::variant<Value, const Counter*> current, int value) { return std::get<Counter const*>(current)->adjust(value); } int const_pointer_caller(std::variant<Value, Counter* const> current, int value) { return std::get<Counter* const>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_volatile_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::get_if_postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::get_if_postfix_volatile_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointer_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_pointer_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_typed_get_expected_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("typed_get_expected_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_value_caller(std::variant<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { return std::get<std::expected<std::weak_ptr<Counter>, Value>>(std::move(current)).value().lock()->adjust(value); } int weak_error_caller(std::variant<Value, std::expected<Value, std::weak_ptr<const Counter>>> current, int value) { return std::get<std::expected<Value, std::weak_ptr<const Counter>>>(std::as_const(current)).error().lock()->adjust(value); } int reference_value_caller(std::variant<Value, std::expected<std::reference_wrapper<Counter>, Value>> current, int value) { return std::get<std::expected<std::reference_wrapper<Counter>, Value>>(std::forward<std::variant<Value, std::expected<std::reference_wrapper<Counter>, Value>>&&>(current)).value().get().adjust(value); } int reference_error_caller(std::variant<Value, std::expected<Value, std::reference_wrapper<const Counter>>> current, int value) { return std::get<std::expected<Value, std::reference_wrapper<const Counter>>>(std::as_const(current)).error().get().adjust(value); } int optional_weak_value_caller(std::variant<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { return std::get<std::expected<std::optional<std::weak_ptr<Counter>>, Value>>(std::move(current)).value()->lock()->adjust(value); } int optional_reference_error_caller(std::variant<Value, std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>> current, int value) { return std::get<std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>>(std::as_const(current)).error()->get().adjust(value); } int optional_smart_value_caller(std::variant<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<std::expected<std::optional<std::unique_ptr<Counter>>, Value>>(std::forward<std::variant<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>>&&>(current)).value()->get()->adjust(value); } int optional_smart_error_caller(std::variant<Value, std::expected<Value, std::optional<std::shared_ptr<const Counter>>>> current, int value) { return std::get<std::expected<Value, std::optional<std::shared_ptr<const Counter>>>>(std::as_const(current)).error()->adjust(value); } int moved_sequence_value_caller(std::variant<Value, std::expected<std::vector<Counter>, Value>> current, int value) { return std::get<std::expected<std::vector<Counter>, Value>>(std::move(current)).value().front().adjust(value); } int const_sequence_error_caller(std::variant<Value, std::expected<Value, std::deque<Counter>>> current, int value) { return std::get<std::expected<Value, std::deque<Counter>>>(std::as_const(current)).error().at(0).adjust(value); } int sequence_value_data_caller(std::variant<Value, std::expected<std::span<Counter>, Value>> current, int value) { return std::get<std::expected<std::span<Counter>, Value>>(current).value().data()->adjust(value); } int const_sequence_error_data_caller(std::variant<Value, std::expected<Value, std::array<Counter, 2>>> current, int value) { return std::get<std::expected<Value, std::array<Counter, 2>>>(std::as_const(current)).error().data()->adjust(value); } int auto_sequence_data_caller(std::variant<Value, std::expected<std::vector<Counter>, Value>> current, int value) { auto pointer = std::get<std::expected<std::vector<Counter>, Value>>(current).value().data(); return pointer->adjust(value); } int decltype_auto_const_sequence_data_caller(std::variant<Value, std::expected<Value, std::span<Counter>>> current, int value) { decltype(auto) pointer = std::get<std::expected<Value, std::span<Counter>>>(std::as_const(current)).error().data(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::weak_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::weak_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::reference_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::reference_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::optional_weak_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_reference_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::optional_smart_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_smart_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_sequence_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_sequence_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::sequence_value_data_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_sequence_error_data_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_sequence_data_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_const_sequence_data_caller",
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
fn binds_cpp_typed_get_expected_optional_wrappers_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("typed_get_expected_optional_wrapper_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_caller(std::variant<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { decltype(auto) nested = std::get<std::expected<std::optional<std::weak_ptr<Counter>>, Value>>(current).value()->lock(); return nested->adjust(value); } int smart_caller(std::variant<Value, std::expected<Value, std::optional<std::shared_ptr<const Counter>>>> current, int value) { auto pointer = std::get<std::expected<Value, std::optional<std::shared_ptr<const Counter>>>>(current).error()->get(); return pointer->adjust(value); } int reference_caller(std::variant<Value, std::expected<std::optional<std::reference_wrapper<const Counter>>, Value>> current, int value) { decltype(auto) nested = std::get<std::expected<std::optional<std::reference_wrapper<const Counter>>, Value>>(current).value()->get(); return nested.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::weak_caller", "api::Counter::adjust(int) &"),
        ("api::smart_caller", "api::Counter::adjust(int) const &"),
        ("api::reference_caller", "api::Counter::adjust(int) const &"),
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
fn preserves_cpp_decltype_auto_typed_get_receiver_categories_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("decltype_auto_typed_get_receiver_categories.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int const_get_caller(const std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); } int rvalue_reference_get_caller(std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(std::move(current)); return nested.adjust(value); } int moved_get_caller(std::variant<Value, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(std::move(current)); return std::move(nested).adjust(value); } int optional_get_caller(const std::variant<Value, std::optional<Counter>> current, int value) { decltype(auto) nested = std::get<std::optional<Counter>>(current); return nested.value().adjust(value); } int typed_expected_weak_caller(std::variant<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { decltype(auto) nested = std::get<std::expected<std::weak_ptr<Counter>, Value>>(current).value().lock(); return nested->adjust(value); } int typed_expected_const_reference_caller(std::variant<Value, std::expected<Value, std::reference_wrapper<const Counter>>> current, int value) { decltype(auto) nested = std::get<std::expected<Value, std::reference_wrapper<const Counter>>>(current).error().get(); return nested.adjust(value); } int typed_expected_auto_value_caller(std::variant<Value, std::expected<Counter, Value>> current, int value) { auto nested = std::get<std::expected<Counter, Value>>(current).value(); return nested.adjust(value); } int typed_expected_decltype_auto_error_caller(const std::variant<Value, std::expected<Value, Counter>> current, int value) { decltype(auto) nested = std::get<std::expected<Value, Counter>>(current).error(); return nested.adjust(value); } int typed_expected_auto_optional_value_caller(std::variant<Value, std::expected<std::optional<Counter>, Value>> current, int value) { auto nested = std::get<std::expected<std::optional<Counter>, Value>>(current).value(); return nested->adjust(value); } int typed_expected_decltype_auto_sequence_error_caller(const std::variant<Value, std::expected<Value, std::vector<Counter>>> current, int value) { decltype(auto) nested = std::get<std::expected<Value, std::vector<Counter>>>(current).error().front(); return nested.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::const_get_caller", "api::Counter::adjust(int) const &"),
        (
            "api::rvalue_reference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::moved_get_caller", "api::Counter::adjust(int) &&"),
        (
            "api::optional_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_expected_weak_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_expected_const_reference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_expected_auto_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_expected_decltype_auto_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::typed_expected_auto_optional_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::typed_expected_decltype_auto_sequence_error_caller",
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
fn does_not_trace_invalid_cpp_typed_get_bindings_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("invalid_typed_get_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) { return value; } }; int missing_auto_caller(std::variant<Value, Counter> current, int value) { auto nested = std::get<std::unique_ptr<Counter>>(current); return nested->adjust(value); } int duplicate_decltype_auto_caller(std::tuple<Counter, Counter> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); } }\n";
    for caller in [
        "api::missing_auto_caller",
        "api::duplicate_decltype_auto_caller",
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

#[test]
fn traces_cpp_indexed_tuple_get_reference_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::reference_wrapper<Counter>> current, int value) { return std::get<1>(current).get().adjust(value); } int const_pair_get_caller(std::pair<std::reference_wrapper<const Counter>, Value> current, int value) { return std::get<0>(current).get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_raw_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, Counter*> current, int value) { return std::get<1>(current)->adjust(value); } int const_pair_get_caller(std::pair<const Counter*, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_optional_value_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_optional_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_value_pair_get_caller(std::pair<std::optional<const Counter>, Value> current, int value) { return std::get<0>(current).value().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_value_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_value_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_value.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current).value().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<const Counter, Value>, Value> current, int value) { return std::get<0>(current).value().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_value_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_value_smart_pointer_arrow_calls_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_value_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int optional_unique_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_indexed_tuple_get_expected_smart_pointer_get_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().get()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error().get()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_indexed_tuple_get_expected_raw_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<Counter*, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<const Counter*, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, Counter*>> current, int value) { return std::get<1>(current).error()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, const Counter*>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_optional_raw_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_optional_raw_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<Counter*>, Value>> current, int value) { return std::get<1>(current).value()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<const Counter*>, Value>, Value> current, int value) { return std::get<0>(current).value()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<Counter*>>> current, int value) { return std::get<1>(current).error()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<const Counter*>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_error_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_error.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(current).error().adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Value, Counter>> current, int value) { return std::get<1>(current).error().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, const Counter>, Value> current, int value) { return std::get<0>(current).error().adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_weak_pointer_lock_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(current).lock()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(current).lock()->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::weak_ptr<const Counter>, Value> current, int value) { return std::get<0>(current).lock()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_optional_arrow_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_optional_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<Counter>> current, int value) { return std::get<1>(current)->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::optional<const Counter>, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointee_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_error_smart_pointer_arrow_calls_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_error_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current).error()->adjust(value); } int optional_shared_pair_get_caller(std::pair<std::expected<Value, std::optional<std::shared_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<Value, std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error()->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_indexed_tuple_get_expected_weak_pointer_lock_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { return std::get<1>(current).value().lock()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::weak_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().lock()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::weak_ptr<Counter>>> current, int value) { return std::get<1>(current).error().lock()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::weak_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current).error().lock()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_optional_weak_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_optional_weak_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->lock()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::weak_ptr<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->lock()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { return std::get<1>(current).error()->lock()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::weak_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->lock()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_optional_reference_wrapper_calls_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_optional_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::reference_wrapper<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->get().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::reference_wrapper<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->get().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { return std::get<1>(current).error()->get().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_optional_smart_pointer_get_calls_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_optional_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::optional<std::shared_ptr<Counter>>, Value>> current, int value) { return std::get<1>(current).value()->get()->adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::optional<std::unique_ptr<const Counter>>, Value>, Value> current, int value) { return std::get<0>(current).value()->get()->adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::optional<std::shared_ptr<Counter>>>> current, int value) { return std::get<1>(current).error()->get()->adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::optional<std::unique_ptr<const Counter>>>, Value> current, int value) { return std::get<0>(current).error()->get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_sequence_element_access_calls_from_unsaved_source_overlay()
{
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_sequence_element_access.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::vector<Counter>, Value>> current, int value) { return std::get<1>(current).value()[0].adjust(value); } int moved_value_tuple_get_caller(std::tuple<Value, std::expected<std::vector<Counter>, Value>> current, int value) { return std::get<1>(std::move(current)).value().front().adjust(value); } int const_value_pair_get_caller(const std::pair<std::expected<std::vector<Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().front().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::deque<Counter>>> current, int value) { return std::get<1>(current).error().at(0).adjust(value); } int const_error_pair_get_caller(const std::pair<std::expected<Value, std::list<Counter>>, Value> current, int value) { return std::get<0>(current).error().back().adjust(value); } int value_data_tuple_get_caller(std::tuple<Value, std::expected<std::span<Counter>, Value>> current, int value) { return std::get<1>(current).value().data()->adjust(value); } int const_error_data_pair_get_caller(const std::pair<std::expected<Value, std::array<Counter, 2>>, Value> current, int value) { return std::get<0>(current).error().data()->adjust(value); } int wrapped_const_error_data_pair_get_caller(std::pair<std::expected<Value, std::array<Counter, 2>>, Value> current, int value) { return std::get<0>(std::as_const(current)).error().data()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::moved_value_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::value_data_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_data_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::wrapped_const_error_data_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_sequence_data_pointer_bindings_from_unsaved_source_overlay()
 {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_sequence_data_pointer.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int auto_value_caller(std::tuple<Value, std::expected<std::vector<Counter>, Value>> current, int value) { auto pointer = std::get<1>(current).value().data(); return pointer->adjust(value); } int decltype_auto_const_error_caller(const std::pair<std::expected<Value, std::span<Counter>>, Value> current, int value) { decltype(auto) pointer = std::get<0>(current).error().data(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::auto_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_auto_const_error_caller",
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
fn traces_cpp_indexed_tuple_get_expected_reference_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_reference_wrapper.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_tuple_get_caller(std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>> current, int value) { return std::get<1>(current).value().get().adjust(value); } int const_value_pair_get_caller(std::pair<std::expected<std::reference_wrapper<const Counter>, Value>, Value> current, int value) { return std::get<0>(current).value().get().adjust(value); } int error_tuple_get_caller(std::tuple<Value, std::expected<Value, std::reference_wrapper<Counter>>> current, int value) { return std::get<1>(current).error().get().adjust(value); } int const_error_pair_get_caller(std::pair<std::expected<Value, std::reference_wrapper<const Counter>>, Value> current, int value) { return std::get<0>(current).error().get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::error_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_error_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_arrow_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int mutable_tuple_get_caller(std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<Counter, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_pointee_pair_get_caller(std::pair<std::expected<const Counter, Value>, Value> current, int value) { return std::get<0>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::mutable_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_tuple_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointee_pair_get_caller",
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
fn traces_cpp_indexed_tuple_get_optional_smart_pointer_arrow_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_optional_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::optional<std::shared_ptr<const Counter>>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(current)->adjust(value); } int moved_tuple_get_caller(std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int forwarded_tuple_get_caller(std::tuple<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::optional<std::unique_ptr<Counter>>>&&>(current))->adjust(value); } int as_const_tuple_get_caller(std::tuple<Value, std::optional<std::shared_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current))->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
        ("api::moved_tuple_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::forwarded_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::as_const_tuple_get_caller",
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
fn traces_cpp_indexed_tuple_get_expected_smart_pointer_arrow_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("indexed_tuple_get_expected_smart_pointer_arrow.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int unique_tuple_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current)->adjust(value); } int const_shared_pair_get_caller(std::pair<std::expected<std::shared_ptr<const Counter>, Value>, Value> current, int value) { return std::get<0>(current)->adjust(value); } int const_tuple_get_caller(const std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(current)->adjust(value); } }\n";
    for (caller, expected_callee) in [
        (
            "api::unique_tuple_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_shared_pair_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::const_tuple_get_caller", "api::Counter::adjust(int) &"),
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

