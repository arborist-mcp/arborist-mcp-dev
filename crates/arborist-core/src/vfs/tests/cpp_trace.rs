use super::*;

#[test]
fn applies_position_edits_in_sequence() {
    let file = temp_file("def value() -> int:\n    return 10\n");
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .apply_position_edits(
            &file,
            &[
                PositionEdit {
                    start: Position { row: 1, column: 11 },
                    end: Position { row: 1, column: 13 },
                    new_text: "20".to_string(),
                },
                PositionEdit {
                    start: Position { row: 1, column: 0 },
                    end: Position { row: 1, column: 0 },
                    new_text: "# staged\n".to_string(),
                },
            ],
        )
        .unwrap();

    assert!(result.source.contains("return 20"));
    assert!(result.source.contains("# staged"));
    assert!(result.dirty);
}

#[test]
fn traces_symbol_graph_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.patch_node(
        &helper_path,
        "helper",
        "def helper(value: int) -> int:\n    return branch(value)\n",
        None,
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
        .unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "branch")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "leaf")
    );
    assert!(
        fs::read_to_string(&helper_path)
            .unwrap()
            .contains("return leaf")
    );
}

#[test]
fn traces_cpp_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    Counter(int value) {}\n};\nCounter caller(int value) { return Counter{}; }\n}\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace lib { class Counter { public: Counter(int value) {} }; }\nnamespace api { using namespace lib; Counter caller(int value) { return Counter{value}; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["lib::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_new_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nint caller(int value) { auto counter = new api::Counter(value); return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_default_new_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller() { return 0; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter() {} }; }\nint caller() { auto counter = new api::Counter; return 0; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter()"]
    );
}

#[test]
fn traces_cpp_braced_initializer_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nint caller(int value) { api::Counter counter{value}; return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("alias.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using First = api::Counter; using Alias = First; int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_typedef_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("alias.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { typedef api::Counter Alias; int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_cv_qualified_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("alias.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = const volatile api::Counter; int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_this_member_template_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: template <typename T> T adjust(T value) { return value; } int caller(int value) { return this->template adjust<int>(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(T)"]
    );
}

#[test]
fn traces_cpp_this_member_template_specializations_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: template <typename T> T adjust(T value) { return value; } int caller(int value) { return this->template adjust< int >(value); } }; template <> int Counter::adjust<int>(int value) { return value + 1; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust<int>(int)"]
    );
}

#[test]
fn traces_cpp_this_member_lvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) && { return value + 1; } int caller(int value) { return this->adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) &"]
    );
}

#[test]
fn traces_cpp_temporary_member_rvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) && { return value + 1; } int adjust(int value) const & { return value + 2; } int adjust(int value) const && { return value + 3; } }; using Alias = Counter; using Second = Alias; int caller(int value) { return Counter{}.adjust(value); } int alias_caller(int value) { return Alias{}.adjust(value); } int chained_alias_caller(int value) { return Second{}.adjust(value); } int moved_caller(int value) { return std::move(Counter{}).adjust(value); } int cast_rvalue_caller(int value) { return static_cast<Counter&&>(Counter{}).adjust(value); } int cast_const_lvalue_caller(int value) { return static_cast<Counter const &>(Counter{}).adjust(value); } int cast_const_rvalue_caller(int value) { return static_cast<const Counter&&>(Counter{}).adjust(value); } int forward_rvalue_caller(int value) { return std::forward<Counter>(Counter{}).adjust(value); } int forward_const_lvalue_caller(int value) { return std::forward<Counter const &>(Counter{}).adjust(value); } int forward_const_rvalue_caller(int value) { return std::forward<const Counter&&>(Counter{}).adjust(value); } }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::caller", "api::Counter::adjust(int) &&"),
        ("api::alias_caller", "api::Counter::adjust(int) &&"),
        ("api::chained_alias_caller", "api::Counter::adjust(int) &&"),
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
        ("api::cast_rvalue_caller", "api::Counter::adjust(int) &&"),
        (
            "api::cast_const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::cast_const_rvalue_caller",
            "api::Counter::adjust(int) const &&",
        ),
        ("api::forward_rvalue_caller", "api::Counter::adjust(int) &&"),
        (
            "api::forward_const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::forward_const_rvalue_caller",
            "api::Counter::adjust(int) const &&",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_local_parameter_and_pointer_member_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; using Alias = Counter; int local_caller(int value) { Alias current{}; return current.adjust(value); } int postfix_const_caller(int value) { Alias const current{}; return current.adjust(value); } int static_caller(int value) { static Alias current{}; return current.adjust(value); } int auto_caller(int value) { auto current = Alias{}; return current.adjust(value); } int auto_direct_list_caller(int value) { auto current{Alias{}}; return current.adjust(value); } int deduced_pointer_caller(int value) { auto current = new Alias{}; return current->adjust(value); } int parenthesized_deduced_pointer_caller(int value) { auto current = new Alias(); return current->adjust(value); } int default_deduced_pointer_caller(int value) { auto current = new Alias; return current->adjust(value); } int pointee_const_deduced_pointer_caller(int value) { auto current = new const Alias{}; return current->adjust(value); } int postfix_pointee_const_deduced_pointer_caller(int value) { auto current = new Alias const{}; return current->adjust(value); } int make_unique_caller(int value) { auto current = std::make_unique<Alias>(); return current->adjust(value); } int make_shared_caller(int value) { auto current = std::make_shared<Alias>(); return current->adjust(value); } int const_deduced_pointer_caller(int value) { const auto current = new Alias{}; return current->adjust(value); } int auto_pointer_caller(int value) { auto* current = new Alias{}; return current->adjust(value); } int const_auto_pointer_caller(int value) { const auto* current = new Alias{}; return current->adjust(value); } int const_auto_caller(int value) { const auto current = Alias{}; return current.adjust(value); } int parameter_caller(const Alias& current, int value) { return current.adjust(value); } int postfix_const_parameter_caller(Alias const& current, int value) { return current.adjust(value); } int rvalue_reference_caller(Alias&& current, int value) { return current.adjust(value); } int moved_rvalue_reference_caller(Alias&& current, int value) { return std::move(current).adjust(value); } int pointer_caller(Alias* current, int value) { return current->adjust(value); } int const_pointer_caller(Alias* const current, int value) { return current->adjust(value); } int postfix_const_pointer_caller(Alias const* current, int value) { return current->adjust(value); } int pointer_reference_caller(Alias* const& current, int value) { return current->adjust(value); } int dereference_caller(Alias* current, int value) { return (*current).adjust(value); } int range_caller() { for (Alias current : values) { return current.adjust(1); } return 0; } int moved_caller(Alias& current, int value) { return std::move(current).adjust(value); } }\n",
        ),
    )
    .unwrap();

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
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_standard_smart_pointer_member_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; struct Deleter {}; using Alias = Counter; int unique_caller(int value) { std::unique_ptr<Alias> current; return current->adjust(value); } int unique_get_caller(int value) { std::unique_ptr<Alias> current; return current.get()->adjust(value); } int auto_unique_caller(int value) { auto current = std::unique_ptr<Alias>{}; return current->adjust(value); } int auto_reference_alias_caller(int value) { Alias target{}; auto& current = target; return current.adjust(value); } int auto_const_reference_alias_caller(int value) { Alias target{}; const auto& current = target; return current.adjust(value); } int auto_forwarding_reference_alias_caller(int value) { const Alias target{}; auto&& current = target; return current.adjust(value); } int copy_list_caller(int value) { auto current = {Alias{}}; return current.adjust(value); } int moved_unique_dereference_caller(int value) { std::unique_ptr<Alias> current; return (*std::move(current)).adjust(value); } int reference_wrapper_get_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); return current.get().adjust(value); } int const_reference_wrapper_get_caller(int value) { const Alias target{}; std::reference_wrapper<const Alias> current(target); return current.get().adjust(value); } int auto_parenthesized_reference_wrapper_caller(int value) { Alias target{}; auto current = (std::reference_wrapper<Alias>(target)); return current.get().adjust(value); } int ref_factory_caller(int value) { Alias target{}; return std::ref(target).get().adjust(value); } int parenthesized_ref_factory_caller(int value) { Alias target{}; return (std::ref(target)).get().adjust(value); } int cref_factory_caller(int value) { Alias target{}; return std::cref(target).get().adjust(value); } int ref_as_const_factory_caller(int value) { Alias target{}; return std::ref(std::as_const(target)).get().adjust(value); } int auto_ref_factory_caller(int value) { Alias target{}; auto current = std::ref(target); return current.get().adjust(value); } int auto_cref_factory_caller(int value) { Alias target{}; auto current = std::cref(target); return current.get().adjust(value); } int auto_ref_as_const_factory_caller(int value) { Alias target{}; auto current = std::ref(std::as_const(target)); return current.get().adjust(value); } int optional_arrow_caller(int value) { std::optional<Alias> current; return current->adjust(value); } int auto_optional_arrow_caller(int value) { auto current = std::optional<Alias>{}; return current->adjust(value); } int nested_optional_unique_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return (*current)->adjust(value); } int nested_optional_unique_value_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return current.value()->adjust(value); } int moved_optional_arrow_caller(int value) { std::optional<Alias> current; return std::move(current)->adjust(value); } int const_optional_arrow_caller(int value) { std::optional<Alias> current; return std::as_const(current)->adjust(value); } int optional_dereference_caller(int value) { std::optional<Alias> current; return (*current).adjust(value); } int moved_optional_value_caller(int value) { std::optional<Alias> current; return std::move(current).value().adjust(value); } int moved_optional_dereference_caller(int value) { std::optional<Alias> current; return (*std::move(current)).adjust(value); } int const_optional_value_caller(int value) { const std::optional<Alias> current{}; return current.value().adjust(value); } int const_optional_dereference_caller(int value) { const std::optional<Alias> current{}; return (*current).adjust(value); } int addressof_caller(int value) { Alias current{}; return std::addressof(current)->adjust(value); } int const_addressof_caller(int value) { const Alias current{}; return std::addressof(current)->adjust(value); } int auto_addressof_caller(int value) { Alias current{}; auto pointer = std::addressof(current); return pointer->adjust(value); } int auto_const_addressof_caller(int value) { const Alias current{}; auto pointer = std::addressof(current); return pointer->adjust(value); } int auto_native_addressof_caller(int value) { Alias current{}; auto pointer = &current; return pointer->adjust(value); } int auto_const_native_addressof_caller(int value) { const Alias current{}; auto pointer = &current; return pointer->adjust(value); } int custom_unique_caller(int value) { std::unique_ptr<Alias, Deleter> current; return current->adjust(value); } int shared_caller(int value) { std::shared_ptr<Alias> current; return current->adjust(value); } int const_unique_caller(int value) { std::unique_ptr<const Alias> current; return current->adjust(value); } int const_unique_get_caller(int value) { std::unique_ptr<const Alias> current; return current.get()->adjust(value); } }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::unique_caller", "api::Counter::adjust(int) &"),
        ("api::unique_get_caller", "api::Counter::adjust(int) &"),
        ("api::auto_unique_caller", "api::Counter::adjust(int) &"),
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
        (
            "api::moved_unique_dereference_caller",
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
            "api::auto_parenthesized_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
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
        ("api::optional_arrow_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_unique_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_unique_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_optional_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::optional_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_dereference_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::const_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_optional_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::addressof_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_addressof_caller",
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
        ("api::custom_unique_caller", "api::Counter::adjust(int) &"),
        ("api::shared_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_unique_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_unique_get_caller",
            "api::Counter::adjust(int) const &",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
        vfs.trace_symbol_graph(&workspace, "api::copy_list_caller", TraceDirection::Both)
            .unwrap()
            .callees
            .is_empty()
    );
}

#[test]
fn traces_cpp_moved_this_member_rvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) && { return value + 1; } int adjust(int value) & { return value; } int caller(int value) && { return std::move(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) &&"]
    );
}

#[test]
fn traces_cpp_nested_this_member_receivers_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) && { return value + 1; } int adjust(int value) const & { return value + 2; } int adjust(int value) const && { return value + 3; } int parenthesized_caller(int value) { return (((*this))).adjust(value); } int moved_caller(int value) { return (std::move(static_cast<Counter &>(*this))).adjust(value); } int const_moved_caller(int value) { return std::move(std::as_const(((*this)))).adjust(value); } int forwarded_caller(int value) { return ((std::forward<Counter const &>(((*this))))).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        (
            "api::Counter::parenthesized_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::Counter::moved_caller", "api::Counter::adjust(int) &&"),
        (
            "api::Counter::const_moved_caller",
            "api::Counter::adjust(int) const &&",
        ),
        (
            "api::Counter::forwarded_caller",
            "api::Counter::adjust(int) const &",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_cast_this_member_rvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) && { return value + 1; } int adjust(int value) & { return value; } int caller(int value) && { return static_cast< Counter && >(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) &&"]
    );
}

#[test]
fn traces_cpp_const_cast_this_member_rvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) const && { return value + 1; } int adjust(int value) && { return value; } int caller(int value) && { return static_cast<const Counter&&>(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &&"]
    );
}

#[test]
fn traces_cpp_const_cast_this_member_lvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) const & { return value + 1; } int adjust(int value) & { return value; } int caller(int value) { return static_cast<Counter const &>(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &"]
    );
}

#[test]
fn traces_cpp_as_const_this_member_lvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) const & { return value + 1; } int adjust(int value) & { return value; } int caller(int value) { return std::as_const(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &"]
    );
}

#[test]
fn traces_cpp_forward_this_member_calls_with_value_categories_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) const & { return value + 3; } int adjust(int value) & { return value + 2; } int adjust(int value) const && { return value + 1; } int adjust(int value) && { return value; } int rvalue_caller(int value) { return std::forward<Counter>(*this).adjust(value); } int const_lvalue_caller(int value) { return std::forward<Counter const &>(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let expected_callees = [
        (
            "api::Counter::rvalue_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::Counter::const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
            .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
        );
    }
}

#[test]
fn traces_cpp_header_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn does_not_trace_cpp_header_type_aliases_moved_after_the_caller_in_virtual_source() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "namespace app { int caller(int value) { Alias counter{value}; return value; } }\n#include \"aliases.hpp\"\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn traces_cpp_type_aliases_from_virtual_local_headers() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &header,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_qualified_namespace_aliases_from_virtual_local_headers_in_order() {
    let workspace = temp_workspace();
    let header = workspace.join("imports.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &caller,
        "#include \"imports.hpp\"\nint caller() { return detail::convert(1); }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &header,
        Some(
            "namespace implementation { int convert(int value) { return value; } }\nnamespace detail = implementation;\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["implementation::convert(int)"]
    );

    vfs.open_file(
        &caller,
        Some("int caller() { return detail::convert(1); }\n#include \"imports.hpp\"\n"),
    )
    .unwrap();
    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn traces_cpp_template_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("alias.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\nnamespace app { template <typename T> using Alias = api::Box<T>; int caller(int value) { Alias<int> box{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );
}

#[test]
fn traces_cpp_template_braced_initializer_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("box.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\nint caller(int value) { api::Box<int> box{value}; return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box<int>::Box(int)"]
    );
}

#[test]
fn traces_cpp_template_new_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("box.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\nint caller(int value) { auto box = new api::Box<int>(value); return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box<int>::Box(int)"]
    );
}

#[test]
fn traces_cpp_template_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("box.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\nint caller(int value) { auto box = api::Box<int>{value}; return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );
}

#[test]
fn trace_patch_context_uses_unsaved_workspace_overrides() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let consumer_path = workspace.join("consumer.py");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &consumer_path,
        "def consume(value: int) -> int:\n    return value\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &consumer_path,
        Some(
            "from caller import orchestrate\n\n\ndef consume(value: int) -> int:\n    return orchestrate(value)\n",
        ),
    )
    .unwrap();

    let result = vfs
        .validate_patch_with_trace_context(
            &workspace,
            &caller_path,
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, result.patch.resolved_symbol_id);
    assert!(result.trace_error.is_none());
    assert_eq!(
        result
            .trace_validation
            .as_ref()
            .map(|validation| validation.allowed),
        Some(true)
    );

    let trace = result.trace.expect("trace result should be present");
    assert!(
        trace
            .callees
            .iter()
            .find(|symbol| symbol.semantic_path == "helper")
            .is_some()
    );
    assert!(
        trace
            .callers
            .iter()
            .find(|symbol| symbol.semantic_path == "consume")
            .is_some()
    );

    let consumer_snapshot = vfs.read_file(&consumer_path).unwrap();
    assert!(consumer_snapshot.dirty);
    assert!(
        consumer_snapshot
            .source
            .contains("return orchestrate(value)")
    );
    let consumer_disk = fs::read_to_string(&consumer_path).unwrap();
    assert!(consumer_disk.contains("return value"));
    assert!(!consumer_disk.contains("return orchestrate(value)"));
}

#[test]
fn trace_patch_context_rejects_unresolved_crlf_patch_bindings() {
    let workspace = temp_workspace();
    let caller_path = workspace.join("caller.py");
    let original_source = "def orchestrate(value: int) -> int:\r\n    return value + 1\r\n";

    fs::write(&caller_path, original_source).unwrap();

    let mut vfs = VirtualFileSystem::new();
    let result = vfs
        .validate_patch_with_trace_context(
            &workspace,
            &caller_path,
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(!result.patch.applied);
    assert_eq!(result.patch.validation.commit_gate.status, "rejected");
    assert_eq!(
        result.patch.validation.unresolved_identifiers,
        vec!["missing_helper"]
    );
    assert!(result.trace.is_none());
    assert!(result.trace_validation.is_none());
    assert_eq!(
        result.trace_error.as_deref(),
        Some("trace skipped because patch validation rejected the patch")
    );

    let snapshot = vfs.read_file(&caller_path).unwrap();
    assert_eq!(snapshot.source, original_source);
    assert!(!snapshot.dirty);
}

#[test]
fn trace_symbol_graph_ignores_virtual_files_in_skipped_dirs() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let venv_path = workspace.join("VENV").join("installed.py");

    fs::create_dir_all(venv_path.parent().unwrap()).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&venv_path, Some("def installed() -> int:\n    return 2\n"))
        .unwrap();

    assert!(
        vfs.trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
            .is_ok()
    );
    assert!(
        vfs.trace_symbol_graph(&workspace, "installed", TraceDirection::Both)
            .is_err()
    );
}

#[test]
fn trace_symbol_graph_ignores_virtual_files_in_sibling_workspace_prefix() {
    let dir = temp_workspace();
    let workspace = dir.join("project");
    let sibling = dir.join("project-extra");
    let helper_path = workspace.join("helper.py");
    let sibling_path = sibling.join("installed.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&sibling).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &sibling_path,
        Some("def installed() -> int:\n    return 2\n"),
    )
    .unwrap();

    assert!(
        vfs.trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
            .is_ok()
    );
    assert!(
        vfs.trace_symbol_graph(&workspace, "installed", TraceDirection::Both)
            .is_err()
    );
}

#[test]
fn traces_cpp_auto_reference_alias_wrappers_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_alias_caller(int value) { Alias target{}; auto&& current = std::move(target); return current.adjust(value); }\nint as_const_alias_caller(int value) { Alias target{}; auto&& current = std::as_const(target); return current.adjust(value); }\nint forwarded_alias_caller(int value) { Alias target{}; auto&& current = std::forward<Alias&&>(target); return current.adjust(value); }\nint const_forwarded_alias_caller(int value) { Alias target{}; auto&& current = std::forward<const Alias&&>(target); return current.adjust(value); }\nint cast_alias_caller(int value) { Alias target{}; auto&& current = static_cast<Alias&&>(target); return current.adjust(value); }\nint const_cast_alias_caller(int value) { Alias target{}; auto&& current = static_cast<const Alias&&>(target); return current.adjust(value); }\nint pointer_alias_caller(Alias* pointer, int value) { auto& current = *pointer; return current.adjust(value); }\nint const_pointer_alias_caller(const Alias* pointer, int value) { auto&& current = *pointer; return current.adjust(value); }\n}\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
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
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_reference_wrapper_get_aliases_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint wrapper_alias_caller(int value) { Alias target{}; std::reference_wrapper<Alias> wrapper(target); auto& current = wrapper.get(); return current.adjust(value); }\nint const_wrapper_alias_caller(int value) { const Alias target{}; std::reference_wrapper<const Alias> wrapper(target); auto&& current = wrapper.get(); return current.adjust(value); }\nint ref_alias_caller(int value) { Alias target{}; auto&& current = std::ref(target).get(); return current.adjust(value); }\nint cref_alias_caller(int value) { Alias target{}; auto&& current = std::cref(target).get(); return current.adjust(value); }\n}\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
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
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_optional_value_aliases_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint value_alias_caller(int value) { std::optional<Alias> current; auto& alias = current.value(); return alias.adjust(value); }\nint const_value_alias_caller(int value) { const std::optional<Alias> current{}; auto&& alias = current.value(); return alias.adjust(value); }\nint moved_value_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::move(current).value(); return alias.adjust(value); }\n}\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::value_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_value_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_value_alias_caller",
            "api::Counter::adjust(int) &",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_optional_dereference_aliases_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint dereference_alias_caller(int value) { std::optional<Alias> current; auto& alias = *current; return alias.adjust(value); }\nint const_dereference_alias_caller(int value) { const std::optional<Alias> current{}; auto&& alias = *current; return alias.adjust(value); }\nint moved_dereference_alias_caller(int value) { std::optional<Alias> current; auto&& alias = *std::move(current); return alias.adjust(value); }\n}\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        (
            "api::dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_dereference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_dereference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_optional_wrapped_aliases_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint moved_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::move(*current); return alias.adjust(value); }\nint as_const_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::as_const(*current); return alias.adjust(value); }\nint forwarded_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::forward<Alias&&>(*current); return alias.adjust(value); }\nint const_forwarded_alias_caller(int value) { std::optional<Alias> current; auto&& alias = std::forward<const Alias&&>(*current); return alias.adjust(value); }\n}\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
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
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_forwarded_base_alias_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { Derived target{}; auto&& alias = std::forward<Base&&>(target); return alias.adjust(value); }\n}\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::caller", TraceDirection::Both)
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
fn traces_cpp_forwarded_optional_base_alias_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { std::optional<Derived> current; auto&& alias = std::forward<Base&&>(*current); return alias.adjust(value); }\n}\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::caller", TraceDirection::Both)
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
fn traces_cpp_cast_optional_base_alias_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&source, Some("namespace api { class Base { public: int adjust(int value) & { return value; } }; class Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } }; int caller(int value) { std::optional<Derived> current; auto&& alias = static_cast<Base&&>(*current); return alias.adjust(value); } }\n")).unwrap();
    let trace = vfs
        .trace_symbol_graph(&workspace, "api::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"]
    );
}

#[test]
fn traces_cpp_addressof_reference_aliases_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&source, Some("namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; using Alias = Counter; int caller(int value) { Alias target{}; auto& alias = *std::addressof(target); return alias.adjust(value); } int const_caller(int value) { const Alias target{}; auto&& alias = *std::addressof(target); return alias.adjust(value); } int wrapped_const_caller(int value) { Alias target{}; auto& alias = *std::addressof(std::as_const(target)); return alias.adjust(value); } int native_caller(int value) { Alias target{}; auto& alias = *&target; return alias.adjust(value); } }\n")).unwrap();
    for (caller, expected_callee) in [
        ("api::caller", "api::Counter::adjust(int) &"),
        ("api::const_caller", "api::Counter::adjust(int) const &"),
        (
            "api::wrapped_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::native_caller", "api::Counter::adjust(int) &"),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_cast_addressof_reference_aliases_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Base { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; class Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } }; int caller(int value) { Derived target{}; auto& alias = *std::addressof(static_cast<Base&>(target)); return alias.adjust(value); } int const_caller(int value) { Derived target{}; auto& alias = *std::addressof(std::as_const(static_cast<const Base&>(target))); return alias.adjust(value); } }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::caller", "api::Base::adjust(int) &"),
        ("api::const_caller", "api::Base::adjust(int) const &"),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_volatile_const_member_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) volatile const & { return value + 1; } int const_caller(int value) volatile const { return adjust(value); } }; int caller(int value) { const Counter current{}; return current.adjust(value); } }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::caller", "api::Counter::adjust(int) volatile const &"),
        (
            "api::Counter::const_caller(int) volatile const",
            "api::Counter::adjust(int) volatile const &",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_decltype_auto_reference_aliases_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } }; using Alias = Counter; int copied_caller(int value) { Alias target{}; decltype(auto) alias = target; return alias.adjust(value); } int copied_const_caller(int value) { const Alias target{}; decltype(auto) alias = target; return alias.adjust(value); } int parenthesized_caller(int value) { Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); } int const_caller(int value) { const Alias target{}; decltype(auto) alias = (target); return alias.adjust(value); } int moved_caller(int value) { Alias target{}; decltype(auto) alias = std::move(target); return alias.adjust(value); } int pointer_caller(int value) { Alias* pointer = nullptr; decltype(auto) alias = *pointer; return alias.adjust(value); } int optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = current.value(); return alias.adjust(value); } int wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = current.get(); return alias.adjust(value); } }\n",
        ),
    )
    .unwrap();

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
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn preserves_cpp_decltype_auto_parenthesized_binding_access_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();
    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } }; using Alias = Counter; int pointer_caller(int value) { Alias* current = nullptr; decltype(auto) alias = (current); return alias->adjust(value); } int optional_caller(int value) { std::optional<Alias> current; decltype(auto) alias = (current); return alias->adjust(value); } int wrapper_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); decltype(auto) alias = (current); return alias.get().adjust(value); } }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::optional_caller", "api::Counter::adjust(int) &"),
        ("api::wrapper_caller", "api::Counter::adjust(int) &"),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
fn traces_cpp_smart_pointer_dereference_aliases_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\nclass Counter {\npublic:\n    int adjust(int value) & { return value; }\n    int adjust(int value) const & { return value + 1; }\n};\nusing Alias = Counter;\nint unique_alias_caller(int value) { std::unique_ptr<Alias> current; auto& alias = *current; return alias.adjust(value); }\nint const_shared_alias_caller(int value) { std::shared_ptr<const Alias> current; auto&& alias = *current; return alias.adjust(value); }\n}\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::unique_alias_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_shared_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
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
