use super::*;

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
