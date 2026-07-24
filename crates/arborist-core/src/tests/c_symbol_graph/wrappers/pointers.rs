use super::*;

#[test]
fn resolves_cpp_weak_pointer_lock_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("weak_pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint direct_caller(std::weak_ptr<Alias> current, int value) { return current.lock()->adjust(value); }\nint local_caller(std::weak_ptr<Alias> current, int value) { auto shared = current.lock(); return shared->adjust(value); }\nint const_wrapper_caller(const std::weak_ptr<Alias> current, int value) { return current.lock()->adjust(value); }\nint auto_const_wrapper_caller(int value) { const auto current = std::weak_ptr<Alias>{}; return current.lock()->adjust(value); }\nint const_caller(std::weak_ptr<const Alias> current, int value) { return current.lock()->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::direct_caller", "api::Counter::adjust(int) &"),
        ("api::local_caller", "api::Counter::adjust(int) &"),
        ("api::const_wrapper_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
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
fn resolves_cpp_wrapped_weak_pointer_lock_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_weak_pointer_lock.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_caller(std::weak_ptr<Counter> current, int value) { return std::move(current).lock()->adjust(value); } int const_caller(std::weak_ptr<Counter> current, int value) { return std::as_const(current).lock()->adjust(value); } int forwarded_caller(std::weak_ptr<Counter> current, int value) { return std::forward<std::weak_ptr<Counter>&&>(current).lock()->adjust(value); } int const_pointee_caller(std::weak_ptr<const Counter> current, int value) { return std::move(current).lock()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
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
fn resolves_cpp_wrapped_reference_wrapper_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_reference_wrapper_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; int moved_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::move(current).get().adjust(value); } int const_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::as_const(current).get().adjust(value); } int forwarded_caller(Counter& target, int value) { std::reference_wrapper<Counter> current(target); return std::forward<std::reference_wrapper<Counter>&&>(current).get().adjust(value); } int const_pointee_caller(const Counter& target, int value) { std::reference_wrapper<const Counter> current(target); return std::move(current).get().adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
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
fn resolves_cpp_wrapped_smart_pointer_get_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_smart_pointer_get.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int moved_caller(std::shared_ptr<Counter> current, int value) { return std::move(current).get()->adjust(value); } int const_caller(std::shared_ptr<Counter> current, int value) { return std::as_const(current).get()->adjust(value); } int forwarded_caller(std::shared_ptr<Counter> current, int value) { return std::forward<std::shared_ptr<Counter>&&>(current).get()->adjust(value); } int const_pointee_caller(std::shared_ptr<const Counter> current, int value) { return std::move(current).get()->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) &"),
        ("api::forwarded_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointee_caller",
            "api::Counter::adjust(int) const &",
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
fn resolves_cpp_direct_standard_pointer_cast_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("direct_standard_pointer_cast.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { class Value {}; class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; int get_if_caller(std::variant<Counter, Value> current, int value) { return std::get_if<Counter>(&current)->adjust(value); } int const_get_if_caller(std::variant<Counter, Value> current, int value) { return std::get_if<Counter>(std::addressof(std::as_const(current)))->adjust(value); } int any_cast_caller(std::any current, int value) { return std::any_cast<Counter>(&current)->adjust(value); } int const_any_cast_caller(std::any current, int value) { return std::any_cast<Counter>(std::addressof(std::as_const(current)))->adjust(value); } }\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::get_if_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_get_if_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::any_cast_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_any_cast_caller",
            "api::Counter::adjust(int) const &",
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
fn resolves_cpp_reference_wrapper_get_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("reference_wrapper_get_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint wrapper_alias_caller(int value) { Alias target{}; std::reference_wrapper<Alias> wrapper(target); auto& current = wrapper.get(); return current.adjust(value); }\nint const_wrapper_alias_caller(int value) { const Alias target{}; std::reference_wrapper<const Alias> wrapper(target); auto&& current = wrapper.get(); return current.adjust(value); }\nint ref_alias_caller(int value) { Alias target{}; auto&& current = std::ref(target).get(); return current.adjust(value); }\nint cref_alias_caller(int value) { Alias target{}; auto&& current = std::cref(target).get(); return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::wrapper_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_wrapper_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::ref_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::cref_alias_caller",
            "api::Counter::adjust(int) const &",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_smart_pointer_dereference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("smart_pointer_dereference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint unique_alias_caller(int value) { std::unique_ptr<Alias> current; auto& alias = *current; return alias.adjust(value); }\nint const_shared_alias_caller(int value) { std::shared_ptr<const Alias> current; auto&& alias = *current; return alias.adjust(value); }\nint unique_copy_caller(int value) { std::unique_ptr<Alias> current; auto alias = *current; return alias.adjust(value); }\nint const_shared_copy_caller(int value) { std::shared_ptr<const Alias> current; auto alias = *current; return alias.adjust(value); }\nint unique_get_copy_caller(int value) { std::unique_ptr<Alias> current; auto pointer = current.get(); return pointer->adjust(value); }\nint const_shared_get_copy_caller(int value) { std::shared_ptr<const Alias> current; auto pointer = current.get(); return pointer->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::unique_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::unique_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_copy_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::unique_get_copy_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_get_copy_caller",
            "api::Counter::adjust(int) const &",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_pointer_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint parameter_caller(Alias* current, int value) { return current->adjust(value); }\nint const_parameter_caller(const Alias* current, int value) { return current->adjust(value); }\nint postfix_const_parameter_caller(Alias const* current, int value) { return current->adjust(value); }\nint const_pointer_parameter_caller(Alias* const current, int value) { return current->adjust(value); }\nint pointer_reference_caller(Alias* const& current, int value) { return current->adjust(value); }\nint const_pointer_local_caller(int value) { Alias* const current = nullptr; return current->adjust(value); }\nint local_caller(int value) { Alias* current = nullptr; return current->adjust(value); }\nint dereference_caller(Alias* current, int value) { return (*current).adjust(value); }\nint addressof_local_caller(int value) { Alias current{}; return std::addressof(current)->adjust(value); }\nint addressof_const_local_caller(int value) { const Alias current{}; return std::addressof(current)->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::parameter_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_pointer_parameter_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointer_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_pointer_local_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::local_caller", "api::Counter::adjust(int) &"),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::addressof_local_caller", "api::Counter::adjust(int) &"),
        (
            "api::addressof_const_local_caller",
            "api::Counter::adjust(int) const &",
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
        let persisted_trace =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            persisted_trace
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
fn resolves_cpp_wrapped_pointer_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("wrapped_pointer_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_parameter_caller(Alias* current, int value) { return std::move(current)->adjust(value); }\nint as_const_parameter_caller(Alias* current, int value) { return std::as_const(current)->adjust(value); }\nint forwarded_const_parameter_caller(const Alias* current, int value) { return std::forward<const Alias*&>(current)->adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::moved_parameter_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_parameter_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::forwarded_const_parameter_caller",
            "api::Counter::adjust(int) const &",
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
