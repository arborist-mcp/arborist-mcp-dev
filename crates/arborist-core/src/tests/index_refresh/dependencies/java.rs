use super::*;

#[test]
fn refreshes_java_unique_explicit_import_dependents() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let importer = source_dir.join("Main.java");
    let helper = source_dir.join("Helper.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &importer,
        "package com.example;\nimport com.example.Helper;\nclass Main { Helper helper; }\n",
    )
    .unwrap();
    fs::write(&helper, "package com.example; class Helper {}\n").unwrap();
    fs::write(&unrelated, "package com.example; class Unrelated {}\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &helper,
        "package com.example; class Helper { int value; }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_java_explicit_static_import_dependents() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let importer = source_dir.join("Main.java");
    let helper = source_dir.join("Helper.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &importer,
        "package com.example;\nimport static com.example.Helper.utility;\nclass Main { int value() { return utility(); } }\n",
    )
    .unwrap();
    fs::write(
        &helper,
        "package com.example; class Helper { static int utility() { return 1; } }\n",
    )
    .unwrap();
    fs::write(&unrelated, "package com.example; class Unrelated {}\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &helper,
        "package com.example; class Helper { static int utility() { return 2; } }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_java_same_package_static_interface_callers() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller = source_dir.join("Main.java");
    let interface = source_dir.join("Tools.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller,
        "package com.example; class Main { int caller() { return Tools.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &interface,
        "package com.example; interface Tools { int utility(int value); }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.example; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let target = "com::example::Tools::utility";
    let before = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(before.callers.is_empty());

    fs::write(
        &interface,
        "package com.example; interface Tools { static int utility(int value) { return value; } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &interface).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let after = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(after.callers.len(), 1);
    assert_eq!(after.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn refreshes_java_same_package_default_interface_callers() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller = source_dir.join("Main.java");
    let interface = source_dir.join("Defaults.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller,
        "package com.example; class Main implements Defaults { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &interface,
        "package com.example; interface Defaults { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.example; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let target = "com::example::Defaults::helper";
    let before = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(before.callers.is_empty());

    fs::write(
        &interface,
        "package com.example; interface Defaults { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &interface).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);

    let after = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(after.callers.len(), 1);
    assert_eq!(after.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn refreshes_java_default_interface_inheritance_dependents() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller = source_dir.join("Main.java");
    let child = source_dir.join("Child.java");
    let root = source_dir.join("Root.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller,
        "package com.example; class Main implements Child { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &child,
        "package com.example; interface Child extends Root {}
",
    )
    .unwrap();
    fs::write(
        &root,
        "package com.example; interface Root { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.example; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let target = "com::example::Root::helper";
    assert!(
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers)
            .unwrap()
            .callers
            .is_empty()
    );

    fs::write(
        &root,
        "package com.example; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &root).unwrap();
    assert_eq!(stats.indexed_files, 4);
    assert_eq!(stats.rebuilt_files, 3);
    assert_eq!(stats.reused_files, 1);

    let after = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(after.callers.len(), 1);
    assert_eq!(after.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn refreshes_java_same_package_static_type_callers() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller = source_dir.join("Main.java");
    let helper = source_dir.join("Helper.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller,
        "package com.example; class Main { int caller() { return Helper.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &helper,
        "package com.example; class Helper { int utility(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.example; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let target = "com::example::Helper::utility";
    let before = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(before.callers.is_empty());

    fs::write(
        &helper,
        "package com.example; class Helper { static int utility(int value) { return value; } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let after = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(after.callers.len(), 1);
    assert_eq!(after.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn refreshes_java_same_package_outer_static_type_callers() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller = source_dir.join("Main.java");
    let outer = source_dir.join("Outer.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller,
        "package com.example; class Main { int caller() { return Outer.Helper.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &outer,
        "package com.example; class Outer { static class Helper { int utility(int value) { return value; } } }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.example; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let target = "com::example::Outer::Helper::utility";
    let before = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(before.callers.is_empty());

    fs::write(
        &outer,
        "package com.example; class Outer { static class Helper { static int utility(int value) { return value; } } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &outer).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let after = trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(after.callers.len(), 1);
    assert_eq!(after.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn refreshes_java_explicit_imported_outer_superclass_dependents() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let child = child_dir.join("Child.java");
    let outer = outer_dir.join("Outer.java");
    let unrelated = child_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &child,
        "package com.child; import com.base.Outer; class Child extends Outer.Base { int caller() { return helper(); } }
",
    )
    .unwrap();
    fs::write(
        &outer,
        "package com.base; class Outer { static class Base { int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.child; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &outer,
        "package com.base; class Outer { static class Base { int helper() { return 2; } int added() { return 3; } } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &outer).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_java_same_package_outer_superclass_dependents() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let child = source_dir.join("Child.java");
    let outer = source_dir.join("Outer.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &child,
        "package com.example; class Child extends Outer.Base { int caller() { return helper(); } }
",
    )
    .unwrap();
    fs::write(
        &outer,
        "package com.example; class Outer { static class Base { int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.example; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &outer,
        "package com.example; class Outer { static class Base { int helper() { return 2; } int added() { return 3; } } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &outer).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_java_same_package_simple_generic_superclass_dependents() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let child = source_dir.join("Child.java");
    let base = source_dir.join("Base.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &child,
        "package com.example; class Child extends Base<String> { int caller() { return helper(); } }
",
    )
    .unwrap();
    fs::write(
        &base,
        "package com.example; class Base<T> { int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.example; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &base,
        "package com.example; class Base<T> { int helper() { return 2; } int added() { return 3; } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &base).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_java_qualified_generic_superclass_dependents() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let child = child_dir.join("Child.java");
    let base = base_dir.join("Base.java");
    let unrelated = child_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &child,
        "package com.child; class Child extends com.base.Base<String> { int caller() { return helper(); } }
",
    )
    .unwrap();
    fs::write(
        &base,
        "package com.base; class Base<T> { int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.child; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &base,
        "package com.base; class Base<T> { int helper() { return 2; } int added() { return 3; } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &base).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_java_qualified_simple_superclass_dependents() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let child = child_dir.join("Child.java");
    let base = base_dir.join("Base.java");
    let unrelated = child_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &child,
        "package com.child; class Child extends com.base.Base { int caller() { return helper(); } }
",
    )
    .unwrap();
    fs::write(
        &base,
        "package com.base; class Base { int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.child; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &base,
        "package com.base; class Base { int helper() { return 2; } int added() { return 3; } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &base).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_java_same_package_simple_superclass_dependents() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let child = source_dir.join("Child.java");
    let base = source_dir.join("Base.java");
    let unrelated = source_dir.join("Unrelated.java");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &child,
        "package com.example; class Child extends Base { int caller() { return helper(); } }
",
    )
    .unwrap();
    fs::write(
        &base,
        "package com.example; class Base { int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package com.example; class Unrelated {}
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &base,
        "package com.example; class Base { int helper() { return 2; } int added() { return 3; } }
",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &base).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}
