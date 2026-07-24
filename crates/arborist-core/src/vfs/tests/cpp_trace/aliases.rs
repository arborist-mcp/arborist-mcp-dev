use super::*;

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
