use super::*;

#[test]
fn resolves_cpp_template_type_alias_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\nnamespace app { template <typename T> using Alias = api::Box<T>; int caller(int value) { Alias<int> box{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );
}

#[test]
fn does_not_trace_cpp_type_aliases_declared_after_the_caller() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    fs::write(
        &source,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { int caller(int value) { Alias counter{value}; return value; } using Alias = api::Counter; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn does_not_trace_cpp_type_aliases_from_unrelated_files() {
    let dir = temporary_dir();
    let definitions = dir.join("counter.cpp");
    let aliases = dir.join("aliases.cpp");
    let caller = dir.join("caller.cpp");
    fs::write(
        &definitions,
        "namespace api { class Counter { public: Counter(int value) {} }; }\n",
    )
    .unwrap();
    fs::write(&aliases, "namespace app { using Alias = api::Counter; }\n").unwrap();
    fs::write(
        &caller,
        "namespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn does_not_trace_unresolved_cpp_type_aliases_as_constructor_dependencies() {
    let dir = temporary_dir();
    let source = dir.join("alias.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace app { using Missing = external::Counter; int caller(int value) { Missing counter{value}; return value; } }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "app::caller", TraceDirection::Both).unwrap();
    assert!(trace.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "app::caller", TraceDirection::Both).unwrap();
    assert!(persisted_trace.callees.is_empty());
}

#[test]
fn resolves_cpp_template_braced_initializer_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("box.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\nint caller(int value) { api::Box<int> box{value}; return value; }\n",
    )
    .unwrap();

    let expected_callee = "api::Box<int>::Box(int)";
    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
}

#[test]
fn resolves_cpp_template_new_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("box.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\nint caller(int value) { auto box = new api::Box<int>(value); return value; }\n",
    )
    .unwrap();

    let expected_callee = "api::Box<int>::Box(int)";
    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
}

#[test]
fn resolves_cpp_template_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("box.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\nint caller(int value) { auto box = api::Box<int>{value}; return value; }\n",
    )
    .unwrap();

    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );
}

#[test]
fn prefers_cpp_template_specialization_constructors_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("box.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int left, int right) {} };\n}\nint caller(int value) { auto box = api::Box<int>{value, value}; return value; }\n",
    )
    .unwrap();

    let expected_callee = "api::Box<int>::Box(int,int)";
    let trace = trace_symbol_graph(&dir, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "caller", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec![expected_callee]
    );
}

#[test]
fn resolves_cpp_imported_constructor_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace lib { class Counter { public: Counter(int value) {} }; }\nnamespace api { using namespace lib; Counter namespace_caller(int value) { return Counter(value); } }\nnamespace vendor = lib;\nnamespace app { using vendor::Counter; Counter declaration_caller(int value) { return Counter(value); } }\n",
    )
    .unwrap();

    for caller in ["api::namespace_caller", "app::declaration_caller"] {
        let trace = trace_symbol_graph(&dir, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec!["lib::Counter::Counter(int)"]
        );
    }

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for caller in ["api::namespace_caller", "app::declaration_caller"] {
        let trace = trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Both).unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec!["lib::Counter::Counter(int)"]
        );
    }
}

#[test]
fn resolves_cpp_auto_constructor_member_calls_across_live_and_persisted_queries() {
    let dir = temporary_dir();
    let source = dir.join("auto_constructor_member_calls.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nstruct Deleter {};\nusing Alias = Counter;\nAlias make_counter() { return Alias{}; }\nint lvalue_caller(int value) { auto current = Alias{}; return current.adjust(value); }\nint auto_reference_alias_caller(int value) { Alias target{}; auto& current = target; return current.adjust(value); }\nint auto_const_reference_alias_caller(int value) { Alias target{}; const auto& current = target; return current.adjust(value); }\nint auto_forwarding_reference_alias_caller(int value) { const Alias target{}; auto&& current = target; return current.adjust(value); }\nint direct_list_caller(int value) { auto current{Alias{}}; return current.adjust(value); }\nint copy_list_caller(int value) { auto current = {Alias{}}; return current.adjust(value); }\nint deduced_pointer_caller(int value) { auto current = new Alias{}; return current->adjust(value); }\nint parenthesized_deduced_pointer_caller(int value) { auto current = new Alias(); return current->adjust(value); }\nint default_deduced_pointer_caller(int value) { auto current = new Alias; return current->adjust(value); }\nint pointee_const_deduced_pointer_caller(int value) { auto current = new const Alias{}; return current->adjust(value); }\nint postfix_pointee_const_deduced_pointer_caller(int value) { auto current = new Alias const{}; return current->adjust(value); }\nint make_unique_caller(int value) { auto current = std::make_unique<Alias>(); return current->adjust(value); }\nint make_shared_caller(int value) { auto current = std::make_shared<Alias>(); return current->adjust(value); }\nint auto_unique_pointer_caller(int value) { auto current = std::unique_ptr<Alias>{}; return current->adjust(value); }\nint auto_const_unique_pointer_caller(int value) { const auto current = std::unique_ptr<Alias>{}; return current->adjust(value); }\nint unique_pointer_caller(int value) { std::unique_ptr<Alias> current; return current->adjust(value); }\nint unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return current.get()->adjust(value); }\nint moved_unique_pointer_dereference_caller(int value) { std::unique_ptr<Alias> current; return (*std::move(current)).adjust(value); }\nint as_const_unique_pointer_dereference_caller(int value) { std::unique_ptr<Alias> current; return (*std::as_const(current)).adjust(value); }\nint forwarded_unique_pointer_dereference_caller(int value) { std::unique_ptr<Alias> current; return (*std::forward<std::unique_ptr<Alias>&&>(current)).adjust(value); }\nint reference_wrapper_get_caller(int value) { std::reference_wrapper<Alias> current = *static_cast<Alias*>(nullptr); return current.get().adjust(value); }\nint const_reference_wrapper_get_caller(int value) { std::reference_wrapper<const Alias> current = *static_cast<Alias*>(nullptr); return current.get().adjust(value); }\nint auto_reference_wrapper_caller(int value) { Alias target{}; auto current = std::reference_wrapper<Alias>(target); return current.get().adjust(value); }\nint auto_parenthesized_reference_wrapper_caller(int value) { Alias target{}; auto current = (std::reference_wrapper<Alias>(target)); return current.get().adjust(value); }\nint auto_const_reference_wrapper_caller(int value) { const Alias target{}; auto current = std::reference_wrapper<const Alias>(target); return current.get().adjust(value); }\nint ref_factory_caller(int value) { Alias target{}; return std::ref(target).get().adjust(value); }\nint parenthesized_ref_factory_caller(int value) { Alias target{}; return (std::ref(target)).get().adjust(value); }\nint cref_factory_caller(int value) { Alias target{}; return std::cref(target).get().adjust(value); }\nint ref_as_const_factory_caller(int value) { Alias target{}; return std::ref(std::as_const(target)).get().adjust(value); }\nint auto_ref_factory_caller(int value) { Alias target{}; auto current = std::ref(target); return current.get().adjust(value); }\nint auto_cref_factory_caller(int value) { Alias target{}; auto current = std::cref(target); return current.get().adjust(value); }\nint auto_ref_as_const_factory_caller(int value) { Alias target{}; auto current = std::ref(std::as_const(target)); return current.get().adjust(value); }\nint auto_addressof_caller(int value) { Alias target{}; auto current = std::addressof(target); return current->adjust(value); }\nint auto_const_addressof_caller(int value) { const Alias target{}; auto current = std::addressof(target); return current->adjust(value); }\nint auto_native_addressof_caller(int value) { Alias target{}; auto current = &target; return current->adjust(value); }\nint auto_const_native_addressof_caller(int value) { const Alias target{}; auto current = &target; return current->adjust(value); }\nint nested_wrapped_unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return (std::move(std::as_const(current))).get()->adjust(value); }\nint forwarded_unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return std::forward<std::unique_ptr<Alias>&>(current).get()->adjust(value); }\nint moved_unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return std::move(current).get()->adjust(value); }\nint as_const_unique_pointer_get_caller(int value) { std::unique_ptr<Alias> current; return std::as_const(current).get()->adjust(value); }\nint custom_unique_pointer_caller(int value) { std::unique_ptr<Alias, Deleter> current; return current->adjust(value); }\nint shared_pointer_caller(int value) { std::shared_ptr<Alias> current; return current->adjust(value); }\nint const_unique_pointer_caller(int value) { std::unique_ptr<const Alias> current; return current->adjust(value); }\nint const_deduced_pointer_caller(int value) { const auto current = new Alias{}; return current->adjust(value); }\nint auto_pointer_caller(int value) { auto* current = new Alias{}; return current->adjust(value); }\nint const_auto_pointer_caller(int value) { const auto* current = new Alias{}; return current->adjust(value); }\nint const_lvalue_caller(int value) { const auto current = Alias{}; return current.adjust(value); }\nint const_reference_caller(int value) { const auto& current = Alias{}; return current.adjust(value); }\nint rvalue_reference_caller(int value) { auto&& current = Alias{}; return current.adjust(value); }\nint factory_caller(int value) { auto current = make_counter(); return current.adjust(value); }\n}\n",
    )
    .unwrap();

    let expected_callees = [
        ("api::lvalue_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_reference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_forwarding_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::direct_list_caller", "api::Counter::adjust(int) &"),
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
        (
            "api::auto_unique_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_unique_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::unique_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_unique_pointer_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::as_const_unique_pointer_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::forwarded_unique_pointer_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::reference_wrapper_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_reference_wrapper_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_parenthesized_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_reference_wrapper_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::ref_factory_caller", "api::Counter::adjust(int) &"),
        (
            "api::parenthesized_ref_factory_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::ref_as_const_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_ref_factory_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_ref_as_const_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::auto_addressof_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_addressof_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_native_addressof_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_native_addressof_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::nested_wrapped_unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::forwarded_unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::as_const_unique_pointer_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::custom_unique_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
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
            "api::const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_reference_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::rvalue_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::factory_caller", "api::make_counter()"),
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
    assert!(
        trace_symbol_graph(&dir, "api::copy_list_caller", TraceDirection::Both)
            .unwrap()
            .callees
            .is_empty()
    );
    assert!(
        trace_symbol_graph_from_index(&db_path, "api::copy_list_caller", TraceDirection::Both)
            .unwrap()
            .callees
            .is_empty()
    );
}

#[test]
fn traces_cpp_constructors_and_destructors() {
    let dir = temporary_dir();
    let header = dir.join("counter.hpp");
    let source = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &header,
        "namespace api {\nclass Counter {\npublic:\n    Counter(int value);\n    ~Counter();\n};\n}\n",
    )
    .unwrap();
    fs::write(
        &source,
        "#include \"counter.hpp\"\n\napi::Counter::Counter(int value) {}\napi::Counter::~Counter() {}\n",
    )
    .unwrap();

    let constructor =
        trace_symbol_graph(&dir, "api::Counter::Counter", TraceDirection::Both).unwrap();
    assert_eq!(
        constructor.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
    let destructor =
        trace_symbol_graph(&dir, "api::Counter::~Counter", TraceDirection::Both).unwrap();
    assert_eq!(
        destructor.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_destructor =
        trace_symbol_graph_from_index(&db_path, "api::Counter::~Counter", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_destructor.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}
