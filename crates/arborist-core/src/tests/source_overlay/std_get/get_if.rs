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

