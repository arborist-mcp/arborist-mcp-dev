use super::*;

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
