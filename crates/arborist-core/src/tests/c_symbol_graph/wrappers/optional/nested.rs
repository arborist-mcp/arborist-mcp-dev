use super::*;

#[test]
fn resolves_cpp_optional_expected_nested_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("optional_expected_nested_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nint value_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); }\nint dereference_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return (*current).value().adjust(value); }\nint value_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { return current.value().error().adjust(value); }\nint arrow_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current->value().adjust(value); }\nint smart_pointer_value_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return current.value().value().get()->adjust(value); }\nint smart_pointer_arrow_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); }\nint nested_optional_value_arrow_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); }\nint nested_optional_value_value_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return current.value().value().value().adjust(value); }\nint const_value_value_caller(const std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); }\nint const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { return current->error().adjust(value); }\nint arrow_error_smart_pointer_get_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error().get()->adjust(value); }\nint arrow_error_smart_pointer_arrow_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error()->adjust(value); }\nint arrow_error_reference_wrapper_caller(std::optional<std::expected<Value, std::reference_wrapper<Counter>>> current, int value) { return current->error().get().adjust(value); }\nint arrow_error_weak_pointer_caller(std::optional<std::expected<Value, std::weak_ptr<Counter>>> current, int value) { return current->error().lock()->adjust(value); }\nint auto_arrow_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); }\nint auto_const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::value_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_error_caller", "api::Counter::adjust(int) &"),
        ("api::arrow_value_caller", "api::Counter::adjust(int) &"),
        (
            "api::smart_pointer_value_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::smart_pointer_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_value_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_value_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_arrow_error_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::arrow_error_smart_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_smart_pointer_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::arrow_error_weak_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_arrow_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_arrow_error_caller",
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

#[test]
fn resolves_cpp_nested_optional_expected_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("nested_optional_expected_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Value {};\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n    int adjust(int value) && { return value + 2; }\n};\nint nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (*current)->value().adjust(value); }\nint nested_opt_opt_exp_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current.value().value().value().adjust(value); }\nint nested_opt_opt_exp_double_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current->value()->value().adjust(value); }\nint nested_opt_opt_exp_deref_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (**current).value().adjust(value); }\nint nested_opt_opt_exp_error_arrow_caller(std::optional<std::optional<std::expected<Value, Counter>>> current, int value) { return (*current)->error().adjust(value); }\nint nested_opt_opt_exp_error_value_caller(std::optional<std::optional<std::expected<Value, Counter>>> current, int value) { return current.value().value().error().adjust(value); }\nint nested_opt_opt_exp_auto_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { auto nested = (*current)->value(); return nested.adjust(value); }\nint moved_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::move(*current)->value().adjust(value); }\nint as_const_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::as_const(*current)->value().adjust(value); }\nint exp_opt_exp_error_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return current.value().value().error().adjust(value); }\nint exp_opt_exp_error_arrow_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return (*current)->error().adjust(value); }\nint opt_exp_error_opt_sp_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { return current->error()->adjust(value); }\nint opt_exp_error_opt_sp_get_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { return current->error().value().get()->adjust(value); }\nint opt_exp_error_opt_weak_arrow_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { return current->error()->lock()->adjust(value); }\nint opt_exp_error_opt_ref_get_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { return current->error()->get().adjust(value); }\nint opt_exp_opt_exp_error_caller(std::optional<std::expected<std::optional<std::expected<Value, Counter>>, Value>> current, int value) { return current->value()->error().adjust(value); }\nint exp_error_opt_exp_value_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return current.error()->value().adjust(value); }\nint exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return (*current.error())->adjust(value); }\nint auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { auto nested = current->error(); return nested->adjust(value); }\nint decltype_auto_exp_error_opt_exp_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error())->value(); return nested.adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->adjust(value); }\nint decltype_auto_exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error()); return nested->adjust(value); }\nint decltype_auto_opt_exp_error_opt_weak_lock_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error()->lock(); return nested->adjust(value); }\nint decltype_auto_opt_exp_value_sp_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { decltype(auto) pointer = current->value().get(); return pointer->adjust(value); }\nint decltype_auto_const_opt_exp_error_opt_sp_get_caller(const std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error().value().get(); return pointer->adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_get_arrow_caller(std::optional<std::expected<Value, std::optional<std::shared_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error()->get(); return pointer->adjust(value); }\nint decltype_auto_opt_opt_sp_arrow_caller(std::optional<std::optional<std::unique_ptr<Counter>>> current, int value) { decltype(auto) nested = *current; return nested->adjust(value); }\nint decltype_auto_opt_exp_error_opt_sp_deref_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = *current->error(); return nested->adjust(value); }\nint auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { auto nested = current->error(); return nested->get().adjust(value); }\nint decltype_auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->get().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        (
            "api::nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_double_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_deref_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_error_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_error_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_opt_opt_exp_auto_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::as_const_nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::exp_opt_exp_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_opt_exp_error_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_sp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_sp_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_weak_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_error_opt_ref_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::opt_exp_opt_exp_error_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_error_opt_exp_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::exp_error_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_opt_exp_error_opt_sp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_exp_error_opt_exp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_exp_error_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_weak_lock_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_value_sp_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_const_opt_exp_error_opt_sp_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_get_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_opt_sp_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_sp_deref_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_opt_exp_error_opt_ref_via_nested_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_auto_opt_exp_error_opt_ref_via_nested_caller",
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
