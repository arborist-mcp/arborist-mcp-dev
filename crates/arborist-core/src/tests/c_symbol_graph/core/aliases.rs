use super::*;

#[test]
fn resolves_cpp_auto_reference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("auto_reference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint mutable_alias_caller(int value) { Alias target{}; auto& current = target; return current.adjust(value); }\nint const_alias_caller(int value) { Alias target{}; const auto& current = target; return current.adjust(value); }\nint postfix_const_alias_caller(int value) { Alias target{}; auto const& current = target; return current.adjust(value); }\nint forwarding_alias_caller(int value) { const Alias target{}; auto&& current = target; return current.adjust(value); }\nint moved_alias_caller(int value) { Alias target{}; auto&& current = std::move(target); return current.adjust(value); }\nint as_const_alias_caller(int value) { Alias target{}; auto&& current = std::as_const(target); return current.adjust(value); }\nint forwarded_alias_caller(int value) { Alias target{}; auto&& current = std::forward<Alias&&>(target); return current.adjust(value); }\nint const_forwarded_alias_caller(int value) { Alias target{}; auto&& current = std::forward<const Alias&&>(target); return current.adjust(value); }\nint cast_alias_caller(int value) { Alias target{}; auto&& current = static_cast<Alias&&>(target); return current.adjust(value); }\nint const_cast_alias_caller(int value) { Alias target{}; auto&& current = static_cast<const Alias&&>(target); return current.adjust(value); }\nint pointer_alias_caller(Alias* pointer, int value) { auto& current = *pointer; return current.adjust(value); }\nint const_pointer_alias_caller(const Alias* pointer, int value) { auto&& current = *pointer; return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::mutable_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_const_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forwarding_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::moved_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::as_const_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::forwarded_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_forwarded_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::cast_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_cast_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::pointer_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_pointer_alias_caller",
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
fn resolves_cpp_forwarded_base_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("forwarded_base_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { Derived target{}; auto&& alias = std::forward<Base&&>(target); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
    );
}

#[test]
fn resolves_cpp_addressof_reference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("addressof_reference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } };\nusing Alias = Counter;\nint caller(int value) { Alias target{}; auto& alias = *std::addressof(target); return alias.adjust(value); }\nint const_caller(int value) { const Alias target{}; auto&& alias = *std::addressof(target); return alias.adjust(value); }\nint wrapped_const_caller(int value) { Alias target{}; auto& alias = *std::addressof(std::as_const(target)); return alias.adjust(value); }\nint native_caller(int value) { Alias target{}; auto& alias = *&target; return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        (
            "api::wrapped_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::native_caller", "api::Counter::adjust(int) &"),
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
fn resolves_cpp_cast_addressof_reference_aliases_with_the_cast_static_type() {
    let dir = temporary_dir();
    let source = dir.join("cast_addressof_reference_alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Base { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { Derived target{}; auto& alias = *std::addressof(static_cast<Base&>(target)); return alias.adjust(value); }\nint const_caller(int value) { Derived target{}; auto& alias = *std::addressof(std::as_const(static_cast<const Base&>(target))); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Base::adjust(int) &"),
        ("api::const_caller", "api::Base::adjust(int) const &"),
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
fn resolves_cpp_volatile_const_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("volatile_const_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter { public: int adjust(int value) & { return value; } int adjust(int value) volatile const & { return value + 1; } int const_caller(int value) volatile const { return adjust(value); } };\nint caller(int value) { const Counter current{}; return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::caller", "api::Counter::adjust(int) volatile const &"),
        (
            "api::Counter::const_caller(int) volatile const",
            "api::Counter::adjust(int) volatile const &",
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
fn resolves_cpp_decltype_auto_reference_alias_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("decltype_auto_reference_alias_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } };\nusing Alias = Counter;\nint copied_caller(int value) { Alias target{}; decltype(auto) alias = target; return alias.adjust(value); }\nint copied_const_caller(int value) { const Alias target{}; decltype(auto) alias = target; return alias.adjust(value); }\nint parenthesized_caller(int value) { Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); }\nint const_caller(int value) { const Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); }\nint moved_caller(int value) { Alias target{}; decltype(auto) alias = std::move(target); return alias.adjust(value); }\nint pointer_caller(int value) { Alias* pointer = nullptr; decltype(auto) alias = *pointer; return alias.adjust(value); }\nint optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = current.value(); return alias.adjust(value); }\nint wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = current.get(); return alias.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
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
fn preserves_cpp_decltype_auto_parenthesized_binding_access_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("decltype_auto_parenthesized_bindings.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter { public: int adjust(int value) & { return value; } };\nusing Alias = Counter;\nint pointer_caller(int value) { Alias* current = nullptr; decltype(auto) alias = (current); return alias->adjust(value); }\nint optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = (current); return alias->adjust(value); }\nint wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = (current); return alias.get().adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
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
