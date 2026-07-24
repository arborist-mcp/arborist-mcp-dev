use super::*;

#[test]
fn traces_cpp_wrapped_indexed_get_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_weak_caller(std::tuple<Value, std::weak_ptr<Counter>> current, int value) { return std::get<1>(std::move(current)).lock()->adjust(value); } int const_weak_caller(std::tuple<Value, std::weak_ptr<const Counter>> current, int value) { return std::get<1>(std::as_const(current)).lock()->adjust(value); } int forwarded_reference_caller(std::tuple<Value, std::reference_wrapper<Counter>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::reference_wrapper<Counter>>&&>(current)).get().adjust(value); } int const_reference_caller(std::tuple<Value, std::reference_wrapper<const Counter>> current, int value) { return std::get<1>(std::as_const(current)).get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_weak_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_weak_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarded_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_reference_caller",
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
fn traces_cpp_wrapped_indexed_get_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_smart_caller(std::tuple<Value, std::unique_ptr<Counter>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int const_smart_caller(std::tuple<Value, std::shared_ptr<const Counter>> current, int value) { return std::get<1>(std::as_const(current))->adjust(value); } int forwarded_raw_caller(std::tuple<Value, Counter*> current, int value) { return std::get<1>(std::forward<std::tuple<Value, Counter*>&&>(current))->adjust(value); } int const_raw_caller(std::tuple<Value, const Counter*> current, int value) { return std::get<1>(std::as_const(current))->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::moved_smart_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_smart_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_raw_caller", "api::Counter::adjust(int) &"),
        ("api::const_raw_caller", "api::Counter::adjust(int) const &"),
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
fn traces_cpp_wrapped_indexed_get_expected_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int direct_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current))->adjust(value); } int value_caller(std::tuple<Value, std::expected<std::shared_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::shared_ptr<Counter>, Value>>&&>(current)).value()->adjust(value); } int error_caller(std::tuple<Value, std::expected<Value, std::shared_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::error_caller", "api::Counter::adjust(int) const &"),
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
fn traces_cpp_wrapped_indexed_get_expected_raw_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_raw_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_caller(std::tuple<Value, std::expected<Counter*, Value>> current, int value) { return std::get<1>(std::move(current)).value()->adjust(value); } int error_caller(std::tuple<Value, std::expected<Value, Counter*>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<Value, Counter*>>&&>(current)).error()->adjust(value); } int const_value_caller(std::tuple<Value, std::expected<const Counter*, Value>> current, int value) { return std::get<1>(std::as_const(current)).value()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::value_caller", "api::Counter::adjust(int) &"),
        ("api::error_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_caller",
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
fn traces_cpp_wrapped_indexed_get_expected_optional_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_optional_pointers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int smart_value_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::move(current)).value()->adjust(value); } int smart_error_caller(std::tuple<Value, std::expected<Value, std::optional<std::shared_ptr<const Counter>>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } int raw_value_caller(std::tuple<Value, std::expected<std::optional<Counter*>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::optional<Counter*>, Value>>&&>(current)).value()->adjust(value); } int raw_error_caller(std::tuple<Value, std::expected<Value, std::optional<const Counter*>>> current, int value) { return std::get<1>(std::as_const(current)).error()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::smart_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::smart_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::raw_value_caller", "api::Counter::adjust(int) &"),
        ("api::raw_error_caller", "api::Counter::adjust(int) const &"),
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
fn traces_cpp_wrapped_indexed_get_expected_optional_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_optional_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_value_caller(std::tuple<Value, std::expected<std::optional<std::weak_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::move(current)).value()->lock()->adjust(value); } int reference_error_caller(std::tuple<Value, std::expected<Value, std::optional<std::reference_wrapper<const Counter>>>> current, int value) { return std::get<1>(std::as_const(current)).error()->get().adjust(value); } int smart_value_caller(std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::optional<std::unique_ptr<Counter>>, Value>>&&>(current)).value()->get()->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::weak_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::reference_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::smart_value_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_wrapped_indexed_get_expected_wrapper_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("wrapped_indexed_get_expected_wrappers.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int weak_value_caller(std::tuple<Value, std::expected<std::weak_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current)).value().lock()->adjust(value); } int weak_error_caller(std::tuple<Value, std::expected<Value, std::weak_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().lock()->adjust(value); } int reference_value_caller(std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>> current, int value) { return std::get<1>(std::forward<std::tuple<Value, std::expected<std::reference_wrapper<Counter>, Value>>&&>(current)).value().get().adjust(value); } int reference_error_caller(std::tuple<Value, std::expected<Value, std::reference_wrapper<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().get().adjust(value); } int smart_value_get_caller(std::tuple<Value, std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return std::get<1>(std::move(current)).value().get()->adjust(value); } int smart_error_get_caller(std::tuple<Value, std::expected<Value, std::shared_ptr<const Counter>>> current, int value) { return std::get<1>(std::as_const(current)).error().get()->adjust(value); } }\n";
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
        ("api::smart_value_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::smart_error_get_caller",
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
