use super::*;

#[test]
fn resolves_cpp_get_if_pointer_bindings_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("get_if_pointer_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint auto_get_if_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get_if<Counter>(&current); return nested->adjust(value); }\nint auto_star_get_if_caller(std::variant<Counter, Value> current, int value) { auto* nested = std::get_if<Counter>(&current); return nested->adjust(value); }\nint decltype_auto_get_if_caller(std::variant<Counter, Value> current, int value) { decltype(auto) nested = std::get_if<Counter>(&current); return nested->adjust(value); }\nint auto_const_get_if_caller(const std::variant<Counter, Value> current, int value) { auto nested = std::get_if<const Counter>(&current); return nested->adjust(value); }\nint auto_dynamic_pointer_cast_caller(std::shared_ptr<Value> current, int value) { auto nested = std::dynamic_pointer_cast<Counter>(current); return nested->adjust(value); }\nint decltype_auto_dynamic_pointer_cast_caller(std::shared_ptr<Value> current, int value) { decltype(auto) nested = std::dynamic_pointer_cast<Counter>(current); return nested->adjust(value); }\nint auto_static_pointer_cast_caller(std::shared_ptr<Value> current, int value) { auto nested = std::static_pointer_cast<Counter>(current); return nested->adjust(value); }\nint auto_const_pointer_cast_caller(std::shared_ptr<const Counter> current, int value) { auto nested = std::const_pointer_cast<Counter>(current); return nested->adjust(value); }\nint auto_any_cast_pointer_caller(std::any current, int value) { auto nested = std::any_cast<Counter>(&current); return nested->adjust(value); }\nint auto_any_cast_value_caller(std::any current, int value) { auto nested = std::any_cast<Counter>(current); return nested.adjust(value); }\nint decltype_auto_any_cast_value_caller(std::any current, int value) { decltype(auto) nested = std::any_cast<Counter>(current); return nested.adjust(value); }\nint auto_variant_get_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get<Counter>(current); return nested.adjust(value); }\nint decltype_auto_variant_get_caller(std::variant<Counter, Value> current, int value) { decltype(auto) nested = std::get<Counter>(current); return nested.adjust(value); }\nint auto_get_if_then_member_caller(std::variant<std::unique_ptr<Counter>, Value> current, int value) { auto nested = std::get_if<std::unique_ptr<Counter>>(&current); return (*nested)->adjust(value); }\nint decltype_auto_get_if_unique_caller(std::variant<std::unique_ptr<Counter>, Value> current, int value) { decltype(auto) nested = std::get_if<std::unique_ptr<Counter>>(&current); return (*nested)->adjust(value); }\nint auto_get_if_value_caller(std::variant<Counter, Value> current, int value) { auto nested = std::get_if<Counter>(&current); return nested->adjust(value); }\nint direct_to_address_raw_caller(Counter* current, int value) { return std::to_address(current)->adjust(value); }\nint auto_to_address_raw_caller(Counter* current, int value) { auto nested = std::to_address(current); return nested->adjust(value); }\nint decltype_auto_to_address_smart_caller(std::unique_ptr<Counter> current, int value) { decltype(auto) nested = std::to_address(current); return nested->adjust(value); }\nint auto_to_address_const_smart_caller(std::unique_ptr<const Counter> current, int value) { auto nested = std::to_address(current); return nested->adjust(value); }\nint vector_front_caller(std::vector<Counter> current, int value) { return current.front().adjust(value); }\nint vector_back_caller(std::vector<Counter> current, int value) { return current.back().adjust(value); }\nint array_at_caller(std::array<Counter, 2> current, int value) { return current.at(0).adjust(value); }\nint span_const_front_caller(std::span<const Counter> current, int value) { return current.front().adjust(value); }\nint const_vector_back_caller(const std::vector<Counter> current, int value) { return current.back().adjust(value); }\nint auto_tuple_get_caller(std::tuple<Value, Counter> current, int value) { auto nested = std::get<1>(current); return nested.adjust(value); }\nint decltype_auto_tuple_get_caller(std::tuple<Value, Counter> current, int value) { decltype(auto) nested = std::get<1>(current); return nested.adjust(value); }\nint auto_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { auto nested = std::get<0>(current); return nested.adjust(value); }\nint decltype_auto_const_pair_get_caller(const std::pair<Counter, Value> current, int value) { decltype(auto) nested = std::get<0>(current); return nested.adjust(value); }\nint auto_tuple_get_unique_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { auto nested = std::get<1>(current); return nested->adjust(value); }\nint decltype_auto_tuple_get_unique_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { decltype(auto) nested = std::get<1>(current); return nested->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::auto_get_if_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_star_get_if_caller",
            "api::Counter::adjust(int) &",
        ),
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
            "api::auto_get_if_value_caller",
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
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
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

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (caller, expected_callee) in expected_callees {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
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
