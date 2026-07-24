use super::*;

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
