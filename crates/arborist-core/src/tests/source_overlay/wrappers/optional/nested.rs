use super::*;

#[test]
fn traces_cpp_expected_optional_error_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int arrow_caller(std::expected<Value, std::optional<Counter>> current, int value) { return current.error()->adjust(value); } int moved_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { return std::move(current).error().value().adjust(value); } int const_dereference_caller(const std::expected<Value, std::optional<Counter>> current, int value) { return (*current.error()).adjust(value); } int auto_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = current.error().value(); return error_value.adjust(value); } int const_auto_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error_value = current.error().value(); return error_value.adjust(value); } int copied_const_source_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = current.error().value(); return error_value.adjust(value); } int auto_pointer_value_caller(std::expected<Value, std::optional<std::shared_ptr<Counter>>> current, int value) { auto error_value = current.error().value(); return error_value->adjust(value); } int auto_dereference_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = *current.error(); return error_value.adjust(value); } int const_auto_dereference_value_caller(std::expected<Value, std::optional<Counter>> current, int value) { const auto error_value = *current.error(); return error_value.adjust(value); } int copied_const_source_dereference_value_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto error_value = *current.error(); return error_value.adjust(value); } int value_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error_value = current.error().value(); return error_value.adjust(value); } int decltype_value_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error_value = current.error().value(); return error_value.adjust(value); } int const_value_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error_value = current.error().value(); return error_value.adjust(value); } int dereference_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error_value = *current.error(); return error_value.adjust(value); } int decltype_dereference_alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { decltype(auto) error_value = *current.error(); return error_value.adjust(value); } int const_dereference_alias_caller(const std::expected<Value, std::optional<Counter>> current, int value) { auto&& error_value = *current.error(); return error_value.adjust(value); } int alias_caller(std::expected<Value, std::optional<Counter>> current, int value) { auto& error = current.error(); return error->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::arrow_caller", "api::Counter::adjust(int) &"),
        ("api::moved_value_caller", "api::Counter::adjust(int) &&"),
        (
            "api::const_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
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
            "api::auto_pointer_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_auto_dereference_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::copied_const_source_dereference_value_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::decltype_value_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_value_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::decltype_dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::alias_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_expected_optional_smart_pointer_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("expected_optional_sp.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int error_value_get_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return current.error().value().get()->adjust(value); } int error_dereference_get_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return (*current.error()).get()->adjust(value); } int value_value_get_caller(std::expected<std::optional<std::shared_ptr<Counter>>, Value> current, int value) { return current.value().value().get()->adjust(value); } int value_dereference_get_caller(std::expected<std::optional<std::shared_ptr<Counter>>, Value> current, int value) { return (*current.value()).get()->adjust(value); } int error_value_arrow_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return current.error().value()->adjust(value); } int error_dereference_arrow_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { return (*current.error())->adjust(value); } int const_error_pointee_caller(std::expected<Value, std::optional<std::shared_ptr<const Counter>>> current, int value) { return (*current.error()).get()->adjust(value); } int get_copy_caller(std::expected<Value, std::optional<std::unique_ptr<Counter>>> current, int value) { auto pointer = current.error().value().get(); return pointer->adjust(value); } }\n";
    for (caller, expected_callee) in [
        ("api::error_value_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::error_dereference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::value_value_get_caller", "api::Counter::adjust(int) &"),
        (
            "api::value_dereference_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::error_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::error_dereference_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_error_pointee_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::get_copy_caller", "api::Counter::adjust(int) &"),
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
fn traces_cpp_optional_expected_nested_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("optional_expected_nested.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int value_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); } int dereference_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return (*current).value().adjust(value); } int value_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { return current.value().error().adjust(value); } int arrow_value_caller(std::optional<std::expected<Counter, Value>> current, int value) { return current->value().adjust(value); } int smart_pointer_value_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return current.value().value().get()->adjust(value); } int smart_pointer_arrow_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); } int nested_optional_value_arrow_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return (*current).value()->adjust(value); } int nested_optional_value_value_caller(std::optional<std::expected<std::optional<Counter>, Value>> current, int value) { return current.value().value().value().adjust(value); } int const_value_value_caller(const std::optional<std::expected<Counter, Value>> current, int value) { return current.value().value().adjust(value); } int const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { return current->error().adjust(value); } int arrow_error_smart_pointer_get_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error().get()->adjust(value); } int arrow_error_smart_pointer_arrow_caller(std::optional<std::expected<Value, std::unique_ptr<Counter>>> current, int value) { return current->error()->adjust(value); } int arrow_error_reference_wrapper_caller(std::optional<std::expected<Value, std::reference_wrapper<Counter>>> current, int value) { return current->error().get().adjust(value); } int arrow_error_weak_pointer_caller(std::optional<std::expected<Value, std::weak_ptr<Counter>>> current, int value) { return current->error().lock()->adjust(value); } int auto_arrow_error_caller(std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); } int auto_const_arrow_error_caller(const std::optional<std::expected<Value, Counter>> current, int value) { auto nested = current->error(); return nested.adjust(value); } }\n";
    for (caller, expected_callee) in [
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
fn traces_cpp_nested_optional_expected_calls_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("nested_optional_expected.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (*current)->value().adjust(value); } int nested_opt_opt_exp_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current.value().value().value().adjust(value); } int nested_opt_opt_exp_double_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return current->value()->value().adjust(value); } int nested_opt_opt_exp_deref_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return (**current).value().adjust(value); } int nested_opt_opt_exp_error_arrow_caller(std::optional<std::optional<std::expected<Value, Counter>>> current, int value) { return (*current)->error().adjust(value); } int nested_opt_opt_exp_auto_value_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { auto nested = (*current)->value(); return nested.adjust(value); } int exp_opt_exp_error_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return current.value().value().error().adjust(value); } int exp_opt_exp_error_arrow_caller(std::expected<std::optional<std::expected<Value, Counter>>, Value> current, int value) { return (*current)->error().adjust(value); } int opt_exp_error_opt_sp_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { return current->error()->adjust(value); } int opt_exp_error_opt_weak_arrow_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { return current->error()->lock()->adjust(value); } int opt_exp_error_opt_ref_get_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { return current->error()->get().adjust(value); } int opt_exp_opt_exp_error_caller(std::optional<std::expected<std::optional<std::expected<Value, Counter>>, Value>> current, int value) { return current->value()->error().adjust(value); } int exp_error_opt_exp_value_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return current.error()->value().adjust(value); } int exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { return (*current.error())->adjust(value); } int auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { auto nested = current->error(); return nested->adjust(value); } int decltype_auto_exp_error_opt_exp_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error())->value(); return nested.adjust(value); } int decltype_auto_opt_exp_error_opt_sp_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->adjust(value); } int decltype_auto_exp_error_opt_exp_arrow_caller(std::expected<Value, std::optional<std::expected<Counter, Value>>> current, int value) { decltype(auto) nested = (*current.error()); return nested->adjust(value); } int decltype_auto_opt_exp_error_opt_weak_lock_caller(std::optional<std::expected<Value, std::optional<std::weak_ptr<Counter>>>> current, int value) { decltype(auto) nested = current->error()->lock(); return nested->adjust(value); } int decltype_auto_opt_exp_value_sp_get_caller(std::optional<std::expected<std::unique_ptr<Counter>, Value>> current, int value) { decltype(auto) pointer = current->value().get(); return pointer->adjust(value); } int decltype_auto_const_opt_exp_error_opt_sp_get_caller(const std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error().value().get(); return pointer->adjust(value); } int decltype_auto_opt_exp_error_opt_sp_get_arrow_caller(std::optional<std::expected<Value, std::optional<std::shared_ptr<Counter>>>> current, int value) { decltype(auto) pointer = current->error()->get(); return pointer->adjust(value); } int decltype_auto_opt_opt_sp_arrow_caller(std::optional<std::optional<std::unique_ptr<Counter>>> current, int value) { decltype(auto) nested = *current; return nested->adjust(value); } int decltype_auto_opt_exp_error_opt_sp_deref_arrow_caller(std::optional<std::expected<Value, std::optional<std::unique_ptr<Counter>>>> current, int value) { decltype(auto) nested = *current->error(); return nested->adjust(value); } int auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { auto nested = current->error(); return nested->get().adjust(value); } int decltype_auto_opt_exp_error_opt_ref_via_nested_caller(std::optional<std::expected<Value, std::optional<std::reference_wrapper<Counter>>>> current, int value) { decltype(auto) nested = current->error(); return nested->get().adjust(value); } int moved_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::move(*current)->value().adjust(value); } int as_const_nested_opt_opt_exp_arrow_caller(std::optional<std::optional<std::expected<Counter, Value>>> current, int value) { return std::as_const(*current)->value().adjust(value); } }\n";
    for (caller, expected_callee) in [
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
            "api::nested_opt_opt_exp_auto_value_caller",
            "api::Counter::adjust(int) &",
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
        (
            "api::moved_nested_opt_opt_exp_arrow_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::as_const_nested_opt_opt_exp_arrow_caller",
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
