use super::*;

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
