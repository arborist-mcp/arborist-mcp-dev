use super::*;

#[test]
fn validates_trace_context_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n";
    let result = validate_patch_with_trace_context_from_index(
        &db_path,
        &caller,
        source,
        "orchestrate",
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        None,
        TraceDirection::Both,
    )
    .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.patch.file, normalize_string_path(&caller));
    assert!(result.trace_error.is_none());
    assert_eq!(
        result.trace_validation.as_ref().map(|value| value.allowed),
        Some(true)
    );
    let trace = result.trace.expect("trace should be present");
    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    let impact = result.impact.expect("impact should be available");
    assert_eq!(impact.affected_symbol_count, 1);
    assert_eq!(impact.added_callees.len(), 1);
    assert_eq!(impact.added_callees[0].semantic_path, "helper");
    assert!(impact.removed_callees.is_empty());
}

#[test]
fn traces_symbol_graph_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller,
        source,
        "orchestrate",
        TraceDirection::Both,
    )
    .unwrap();

    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
    assert!(
        fs::read_to_string(&caller)
            .unwrap()
            .contains("return value + 1")
    );
}

#[test]
fn index_source_overlay_skips_byte_range_validation_against_stale_disk_source() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(
        &caller,
        "from helper import helper\n\n\ndef orchestrate() -> int:\n    return helper()\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    fs::write(&caller, "def stale():\n    return 0\n").unwrap();
    let source = "from helper import helper\n\n\ndef orchestrate() -> int:\n    return helper()\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller,
        source,
        "orchestrate",
        TraceDirection::Both,
    )
    .expect("source overlays must replace stale disk content before range validation");

    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

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

#[test]
fn traces_cpp_auto_reference_aliases_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_alias_caller(int value) { Alias target{}; auto&& alias = std::move(target); return alias.adjust(value); }\nint reference_wrapper_alias_caller(int value) { Alias target{}; std::reference_wrapper<Alias> wrapper(target); auto& alias = wrapper.get(); return alias.adjust(value); }\nint optional_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::as_const(*current); return alias.adjust(value); }\nint smart_pointer_alias_caller(int value) { std::shared_ptr<const Alias> current; auto&& alias = *current; return alias.adjust(value); }\n}\n";
    for (caller, expected_callee) in [
        ("api::moved_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::reference_wrapper_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::optional_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::smart_pointer_alias_caller",
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
fn traces_cpp_forwarded_base_alias_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { Derived target{}; auto&& alias = std::forward<Base&&>(target); return alias.adjust(value); }\n}\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        source,
        "api::caller",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
    );
}

#[test]
fn traces_cpp_addressof_reference_aliases_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; using Alias = Counter; int caller(int value) { Alias target{}; auto& alias = *std::addressof(target); return alias.adjust(value); } int const_caller(int value) { const Alias target{}; auto&& alias = *std::addressof(target); return alias.adjust(value); } int wrapped_const_caller(int value) { Alias target{}; auto& alias = *std::addressof(std::as_const(target)); return alias.adjust(value); } int native_caller(int value) { Alias target{}; auto& alias = *&target; return alias.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        (
            "api::wrapped_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::native_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_cast_addressof_reference_aliases_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Base { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; class Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } }; int caller(int value) { Derived target{}; auto& alias = *std::addressof(static_cast<Base&>(target)); return alias.adjust(value); } int const_caller(int value) { Derived target{}; auto& alias = *std::addressof(std::as_const(static_cast<const Base&>(target))); return alias.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::caller", "api::Base::adjust(int) &"),
        ("api::const_caller", "api::Base::adjust(int) const &"),
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
fn traces_cpp_volatile_const_member_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) volatile const & { return value + 1; } int const_caller(int value) volatile const { return adjust(value); } }; int caller(int value) { const Counter current{}; return current.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::caller", "api::Counter::adjust(int) volatile const &"),
        (
            "api::Counter::const_caller(int) volatile const",
            "api::Counter::adjust(int) volatile const &",
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
fn traces_cpp_decltype_auto_reference_aliases_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; using Alias = Counter; int copied_caller(int value) { Alias target{}; decltype(auto) alias = target; return alias.adjust(value); } int copied_const_caller(int value) { const Alias target{}; decltype(auto) alias = target; return alias.adjust(value); } int parenthesized_caller(int value) { Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); } int const_caller(int value) { const Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); } int moved_caller(int value) { Alias target{}; decltype(auto) alias = std::move(target); return alias.adjust(value); } int pointer_caller(int value) { Alias* pointer = nullptr; decltype(auto) alias = *pointer; return alias.adjust(value); } int optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = current.value(); return alias.adjust(value); } int wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = current.get(); return alias.adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::copied_caller", "api::Counter::adjust(int) &"),
        (
            "api::copied_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::parenthesized_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
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
fn preserves_cpp_decltype_auto_parenthesized_binding_access_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } }; using Alias = Counter; int pointer_caller(int value) { Alias* current = nullptr; decltype(auto) alias = (current); return alias->adjust(value); } int optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = (current); return alias->adjust(value); } int wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = (current); return alias.get().adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
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
fn trace_symbol_graph_at_position_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let trace = trace_symbol_graph_at_position_from_index_with_source(
        &db_path,
        &caller,
        source,
        &Position { row: 3, column: 5 },
        TraceDirection::Both,
    )
    .unwrap();

    assert_eq!(trace.symbol.semantic_path, "orchestrate");
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn traces_cpp_auto_reference_alias_at_position_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::as_const(*current); return alias.adjust(value); }\n}\n";
    let trace = trace_symbol_graph_at_position_from_index_with_source(
        &db_path,
        &source_path,
        source,
        &Position { row: 7, column: 5 },
        TraceDirection::Both,
    )
    .unwrap();

    assert_eq!(trace.symbol.semantic_path, "api::alias_caller");
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &"],
    );
}

#[test]
fn reads_symbol_context_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "def orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n";
    let context = read_symbol_context_from_index_with_source(
        &db_path,
        &caller,
        source,
        "helper",
        TraceDirection::Callers,
    )
    .unwrap();

    assert_eq!(context.read.symbol.semantic_path, "helper");
    assert_eq!(context.trace.symbol.semantic_path, "helper");
    assert_eq!(context.trace.callers.len(), 1);
    assert_eq!(context.trace.callers[0].semantic_path, "orchestrate");
}

#[test]
fn searches_symbols_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let results = search_symbols_from_index_with_source_filtered(
        &db_path,
        &helper,
        source,
        "helper_alias",
        10,
        None,
        None,
    )
    .unwrap();

    assert_eq!(results.total_matches, 1);
    assert_eq!(results.matches.len(), 1);
    assert_eq!(results.matches[0].semantic_path, "helper_alias");
    assert!(
        fs::read_to_string(&helper)
            .unwrap()
            .contains("def helper() -> int")
    );
    assert!(
        !fs::read_to_string(&helper)
            .unwrap()
            .contains("helper_alias")
    );
}

#[test]
fn lists_symbols_from_index_with_unsaved_source_overlay() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let listed =
        list_symbols_from_index_with_source_filtered(&db_path, &helper, source, 10, None, None)
            .unwrap();

    assert_eq!(listed.total_symbols, 2);
    assert_eq!(listed.symbols.len(), 2);
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
}

#[test]
fn symbol_query_context_applies_multiple_workspace_overlays_without_writing_disk() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&caller, "def orchestrate() -> int:\n    return 0\n").unwrap();

    let helper_source =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let caller_source = "from helper import helper_alias\n\n\ndef orchestrate() -> int:\n    return helper_alias()\n";
    let context = SymbolQueryContext::workspace(&dir)
        .unwrap()
        .with_source_overlay(&helper, helper_source)
        .unwrap()
        .with_source_overlay(&caller, caller_source)
        .unwrap();

    let search = context
        .search_symbols("helper_alias", 10, None, None)
        .unwrap();
    assert_eq!(search.total_matches, 1);

    let trace = context
        .trace_symbol_graph("orchestrate", TraceDirection::Callees)
        .unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );

    let listed = context.list_symbols(10, None, None).unwrap();
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
    assert!(
        !fs::read_to_string(&helper)
            .unwrap()
            .contains("helper_alias")
    );
    assert!(
        !fs::read_to_string(&caller)
            .unwrap()
            .contains("helper_alias")
    );
}

#[test]
fn symbol_query_context_applies_multiple_index_overlays() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper() -> int:\n    return 1\n").unwrap();
    fs::write(&caller, "def orchestrate() -> int:\n    return 0\n").unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let helper_source =
        "def helper() -> int:\n    return 1\n\n\ndef helper_alias() -> int:\n    return helper()\n";
    let caller_source = "from helper import helper_alias\n\n\ndef orchestrate() -> int:\n    return helper_alias()\n";
    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&helper, helper_source)
        .unwrap()
        .with_source_overlay(&caller, caller_source)
        .unwrap();

    let search = context
        .search_symbols("helper_alias", 10, None, None)
        .unwrap();
    assert_eq!(search.total_matches, 1);

    let trace = context
        .trace_symbol_graph("orchestrate", TraceDirection::Callees)
        .unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );

    let listed = context.list_symbols(10, None, None).unwrap();
    assert!(
        listed
            .symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "helper_alias")
    );
}

#[test]
fn index_overlay_rejects_inconsistent_indexed_file_counts() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let db_path = dir.join("symbols.db");
    let source = "def helper() -> int:\n    return 1\n";

    fs::write(&helper, source).unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let connection = Connection::open(&db_path).unwrap();
    connection
        .execute(
            "UPDATE metadata SET value = '2' WHERE key = 'indexed_files'",
            [],
        )
        .unwrap();
    drop(connection);

    let error = search_symbols_from_index_with_source_filtered(
        &db_path, &helper, source, "helper", 10, None, None,
    )
    .expect_err("source overlays should reject inconsistent persisted file counts");

    assert!(error.to_string().contains("indexed_files metadata 2"));
    assert!(error.to_string().contains("file_state entries 1"));
}

#[test]
fn symbol_query_context_rejects_workspace_overlay_outside_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&outside, "def outside() -> int:\n    return 2\n")
        .expect_err("workspace contexts should reject overlays outside the workspace");

    assert!(error.to_string().contains("outside workspace"));
}

#[test]
fn symbol_query_context_rejects_workspace_overlay_in_ignored_directory() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let ignored = workspace.join(".venv").join("ignored.py");

    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&ignored, "def ignored() -> int:\n    return 2\n")
        .expect_err("workspace contexts should reject overlays in ignored directories");

    assert!(error.to_string().contains("ignored workspace directory"));
}

#[test]
fn symbol_query_context_rejects_workspace_overlay_with_unsupported_extension() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let unsupported = workspace.join("notes.txt");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();

    let error = SymbolQueryContext::workspace(&workspace)
        .unwrap()
        .with_source_overlay(&unsupported, "not source code")
        .expect_err("workspace contexts should reject unsupported source overlays");

    assert!(error.to_string().contains("not a supported source file"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_outside_indexed_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let outside = dir.join("outside.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    fs::write(&outside, "def outside() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&outside, "def outside() -> int:\n    return 2\n")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject overlays outside the indexed workspace");

    assert!(error.to_string().contains("outside indexed workspace"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_in_ignored_directory() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let ignored = workspace.join("node_modules").join("ignored.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&ignored, "def ignored() -> int:\n    return 2\n")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject overlays in ignored directories");

    assert!(error.to_string().contains("ignored workspace directory"));
}

#[test]
fn symbol_query_context_rejects_index_overlay_with_unsupported_extension() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let indexed = workspace.join("indexed.py");
    let unsupported = workspace.join("notes.txt");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(&indexed, "def indexed() -> int:\n    return 1\n").unwrap();
    rebuild_symbol_index(&workspace, &db_path).unwrap();

    let context = SymbolQueryContext::index(&db_path)
        .unwrap()
        .with_source_overlay(&unsupported, "not source code")
        .unwrap();
    let error = context
        .list_symbols(10, None, None)
        .expect_err("index contexts should reject unsupported source overlays");

    assert!(error.to_string().contains("not a supported source file"));
}

#[test]
fn rejects_trace_context_file_outside_workspace() {
    let dir = temporary_dir();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::write(
        &outside,
        "def top_level(value: int) -> int:\n    return value\n",
    )
    .unwrap();

    let error = validate_patch_with_trace_context_from_path(
        &workspace,
        &outside,
        "top_level",
        "def top_level(value: int) -> int:\n    return value + 1\n",
        None,
        TraceDirection::Both,
    )
    .expect_err("trace context should reject files outside the workspace");

    assert!(error.to_string().contains("outside workspace"));
}
