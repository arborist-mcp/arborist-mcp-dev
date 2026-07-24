use super::*;

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
