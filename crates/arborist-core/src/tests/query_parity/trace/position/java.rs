use super::*;
use crate::{trace_symbol_graph_from_index_with_source, trace_symbol_graph_with_source};

#[test]
fn traces_java_unqualified_same_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Counter {\n    int helper() { return 1; }\n    int caller() { return helper(); }\n    int first(int value) { return value; }\n    long first(long value) { return value; }\n    long ambiguous() { return first(1L); }\n}\n",
    )
    .unwrap();

    let helper_path = "com::example::Counter::helper";
    let live = trace_symbol_graph(&dir, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_path);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Counter::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_path);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Counter::caller"
    );

    let overloaded_id = format!(
        "{}::com::example::Counter::first#overload[2]",
        normalize_path(&source_path)
    );
    let overloaded =
        trace_symbol_graph_from_index(&db_path, &overloaded_id, TraceDirection::Callers).unwrap();
    assert!(overloaded.callers.is_empty());
}

#[test]
fn traces_java_explicit_this_constructor_initializers_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Counter {\n    Counter() {}\n    Counter(int value) { this(); }\n    Counter(int... values) {}\n    Counter(boolean first, boolean second) { this(1, 2); }\n}\n",
    )
    .unwrap();

    let target = format!(
        "{}::com::example::Counter::Counter#overload[1]",
        normalize_path(&source_path)
    );
    let delegated_constructor = format!(
        "{}::com::example::Counter::Counter#overload[2]",
        normalize_path(&source_path)
    );
    let params_constructor = format!(
        "{}::com::example::Counter::Counter#overload[3]",
        normalize_path(&source_path)
    );

    let live = trace_symbol_graph(&dir, &target, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, delegated_constructor);
    let params_live =
        trace_symbol_graph(&dir, &params_constructor, TraceDirection::Callers).unwrap();
    assert!(params_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, &target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, delegated_constructor);
    let params_persisted =
        trace_symbol_graph_from_index(&db_path, &params_constructor, TraceDirection::Callers)
            .unwrap();
    assert!(params_persisted.callers.is_empty());
}

#[test]
fn traces_java_explicit_this_constructor_initializers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(&source_path, "package com.example; class Counter {}\n").unwrap();
    let overlay = "package com.example;\nclass Counter {\n    Counter() {}\n    Counter(int value) { this(); }\n}\n";
    let target = format!(
        "{}::com::example::Counter::Counter#overload[1]",
        normalize_path(&source_path)
    );
    let delegated_constructor = format!(
        "{}::com::example::Counter::Counter#overload[2]",
        normalize_path(&source_path)
    );

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        &target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, delegated_constructor);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        &target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, delegated_constructor);
}

#[test]
fn traces_java_explicit_same_file_super_constructor_initializers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Base {\n    Base() {}\n    Base(int value) {}\n    Base(int... values) {}\n    int helper() { return 1; }\n}\nclass Child extends Base {\n    Child() { super(); }\n    Child(int value) { super(value); }\n    Child(boolean first, boolean second) { super(1, 2); }\n    int inheritedCaller() { return super.helper(); }\n    int inheritedBareCaller() { return helper(); }\n}\n",
    )
    .unwrap();
    let file_path = normalize_path(&source_path);
    let base_zero = format!("{file_path}::com::example::Base::Base#overload[1]");
    let base_one = format!("{file_path}::com::example::Base::Base#overload[2]");
    let child_zero = format!("{file_path}::com::example::Child::Child#overload[1]");
    let child_one = format!("{file_path}::com::example::Child::Child#overload[2]");
    let base_params = format!("{file_path}::com::example::Base::Base#overload[3]");

    let live_zero = trace_symbol_graph(&dir, &base_zero, TraceDirection::Callers).unwrap();
    assert_eq!(live_zero.callers.len(), 1);
    assert_eq!(live_zero.callers[0].symbol_id, child_zero);
    let live_one = trace_symbol_graph(&dir, &base_one, TraceDirection::Callers).unwrap();
    assert_eq!(live_one.callers.len(), 1);
    assert_eq!(live_one.callers[0].symbol_id, child_one);
    let live_params = trace_symbol_graph(&dir, &base_params, TraceDirection::Callers).unwrap();
    assert!(live_params.callers.is_empty());
    let helper_live =
        trace_symbol_graph(&dir, "com::example::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 2);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::example::Child::inheritedBareCaller"
    );
    assert_eq!(
        helper_live.callers[1].symbol_id,
        "com::example::Child::inheritedCaller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_zero =
        trace_symbol_graph_from_index(&db_path, &base_zero, TraceDirection::Callers).unwrap();
    assert_eq!(persisted_zero.callers.len(), 1);
    assert_eq!(persisted_zero.callers[0].symbol_id, child_zero);
    let persisted_one =
        trace_symbol_graph_from_index(&db_path, &base_one, TraceDirection::Callers).unwrap();
    assert_eq!(persisted_one.callers.len(), 1);
    assert_eq!(persisted_one.callers[0].symbol_id, child_one);
    let persisted_params =
        trace_symbol_graph_from_index(&db_path, &base_params, TraceDirection::Callers).unwrap();
    assert!(persisted_params.callers.is_empty());
    let helper_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 2);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::example::Child::inheritedBareCaller"
    );
    assert_eq!(
        helper_persisted.callers[1].symbol_id,
        "com::example::Child::inheritedCaller"
    );
}

#[test]
fn traces_java_explicit_same_file_super_constructor_initializers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(&source_path, "package com.example; class Stale {}\n").unwrap();
    let overlay = "package com.example;\nclass Base { Base() {} int helper() { return 1; } }\nclass Child extends Base { Child() { super(); } int inheritedCaller() { return super.helper(); }\n    int inheritedBareCaller() { return helper(); } }\n";
    let target = "com::example::Base::Base";
    let child_constructor = "com::example::Child::Child";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, child_constructor);
    let helper_live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        "com::example::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_live.callers.len(), 2);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::example::Child::inheritedBareCaller"
    );
    assert_eq!(
        helper_live.callers[1].symbol_id,
        "com::example::Child::inheritedCaller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, child_constructor);
    let helper_persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        "com::example::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 2);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::example::Child::inheritedBareCaller"
    );
    assert_eq!(
        helper_persisted.callers[1].symbol_id,
        "com::example::Child::inheritedCaller"
    );
}

#[test]
fn traces_java_same_file_multilevel_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper() { return 1; } }
class Base extends Grand {}
class Child extends Base {
    int bareCaller() { return helper(); }
    int superCaller() { return super.helper(); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    assert_eq!(
        live.callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Child::bareCaller",
            "com::example::Child::superCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Child::bareCaller",
            "com::example::Child::superCaller"
        ]
    );
}

#[test]
fn traces_java_same_file_multilevel_inherited_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Grand { int helper() { return 1; } }
class Base extends Grand {}
class Child extends Base { int caller() { return helper(); } }
";
    let helper_symbol = "com::example::Grand::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Child::caller"
    );
}

#[test]
fn ignores_cyclic_java_same_file_inheritance() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class First extends Second {}
class Second extends First {}
class Child extends First { int caller() { return helper(); } }
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "com::example::Child::caller", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Child::caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_java_same_package_simple_superclasses_across_files() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let grand_path = source_dir.join("Grand.java");
    let base_path = source_dir.join("Base.java");
    let child_path = source_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &grand_path,
        "package com.example; class Grand { int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &base_path,
        "package com.example; class Base extends Grand { Base() {} Base(int value) {} Base(int... values) {} }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; class Child extends Base { Child() { super(); } Child(int value) { super(value); } Child(boolean first, boolean second) { super(1, 2); } int bareCaller() { return helper(); } int superCaller() { return super.helper(); } }
",
    )
    .unwrap();

    let base_file_path = normalize_path(&base_path);
    let child_file_path = normalize_path(&child_path);
    let base_zero = format!("{base_file_path}::com::example::Base::Base#overload[1]");
    let base_one = format!("{base_file_path}::com::example::Base::Base#overload[2]");
    let base_params = format!("{base_file_path}::com::example::Base::Base#overload[3]");
    let child_zero = format!("{child_file_path}::com::example::Child::Child#overload[1]");
    let child_one = format!("{child_file_path}::com::example::Child::Child#overload[2]");
    for (target, caller) in [
        (base_zero.as_str(), child_zero.as_str()),
        (base_one.as_str(), child_one.as_str()),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(live.callers.len(), 1);
        assert_eq!(live.callers[0].symbol_id, caller);
    }
    let live_params = trace_symbol_graph(&dir, &base_params, TraceDirection::Callers).unwrap();
    assert!(live_params.callers.is_empty());
    let helper_live =
        trace_symbol_graph(&dir, "com::example::Grand::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 2);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    for (target, caller) in [
        (base_zero.as_str(), child_zero.as_str()),
        (base_one.as_str(), child_one.as_str()),
    ] {
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(persisted.callers.len(), 1);
        assert_eq!(persisted.callers[0].symbol_id, caller);
    }
    let persisted_params =
        trace_symbol_graph_from_index(&db_path, &base_params, TraceDirection::Callers).unwrap();
    assert!(persisted_params.callers.is_empty());
    let helper_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Grand::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 2);
}

#[test]
fn traces_java_same_package_simple_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let base_path = source_dir.join("Base.java");
    let child_path = source_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &base_path,
        "package com.example; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; class Child extends Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::example::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::example::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Child::Child");
}

#[test]
fn traces_java_explicit_imported_outer_superclasses_across_files() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let outer_path = outer_dir.join("Outer.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.base; class Outer { static class Base { Base() {} int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; import com.base.Outer; class Child extends Outer.Base { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live = trace_symbol_graph(
        &dir,
        "com::base::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live = trace_symbol_graph(
        &dir,
        "com::base::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::base::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::base::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_same_package_outer_superclasses_across_files() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let outer_path = source_dir.join("Outer.java");
    let child_path = source_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { static class Base { Base() {} int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; class Child extends Outer.Base { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live = trace_symbol_graph(
        &dir,
        "com::example::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::example::Child::Child"
    );
    let helper_live = trace_symbol_graph(
        &dir,
        "com::example::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::example::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::example::Child::Child"
    );
    let helper_persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::example::Child::caller"
    );
}

#[test]
fn traces_java_explicit_local_import_simple_generic_superclasses_across_files() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base<T> { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; import com.base.Base; class Child extends Base<String> { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live =
        trace_symbol_graph(&dir, "com::base::Base::Base", TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live =
        trace_symbol_graph(&dir, "com::base::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::Base", TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_qualified_generic_superclasses_across_files() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base<T> { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Child extends com.base.Base<String> { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live =
        trace_symbol_graph(&dir, "com::base::Base::Base", TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live =
        trace_symbol_graph(&dir, "com::base::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::Base", TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_qualified_simple_superclasses_across_files() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Child extends com.base.Base { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live =
        trace_symbol_graph(&dir, "com::base::Base::Base", TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live =
        trace_symbol_graph(&dir, "com::base::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::Base", TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_explicit_local_import_simple_superclasses_across_files() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; import com.base.Base; class Child extends Base { Child() { super(); } int caller() { return super.helper(); } }
",
    )
    .unwrap();

    let constructor_live =
        trace_symbol_graph(&dir, "com::base::Base::Base", TraceDirection::Callers).unwrap();
    assert_eq!(constructor_live.callers.len(), 1);
    assert_eq!(
        constructor_live.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_live =
        trace_symbol_graph(&dir, "com::base::Base::helper", TraceDirection::Callers).unwrap();
    assert_eq!(helper_live.callers.len(), 1);
    assert_eq!(
        helper_live.callers[0].symbol_id,
        "com::child::Child::caller"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let constructor_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::Base", TraceDirection::Callers)
            .unwrap();
    assert_eq!(constructor_persisted.callers.len(), 1);
    assert_eq!(
        constructor_persisted.callers[0].symbol_id,
        "com::child::Child::Child"
    );
    let helper_persisted =
        trace_symbol_graph_from_index(&db_path, "com::base::Base::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(helper_persisted.callers.len(), 1);
    assert_eq!(
        helper_persisted.callers[0].symbol_id,
        "com::child::Child::caller"
    );
}

#[test]
fn traces_java_explicit_imported_outer_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let outer_path = outer_dir.join("Outer.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.base; class Outer { static class Base { Base() {} int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; import com.base.Outer; class Child extends Outer.Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_same_package_outer_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let outer_path = source_dir.join("Outer.java");
    let child_path = source_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { static class Base { Base() {} int helper() { return 1; } } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; class Child extends Outer.Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::example::Outer::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::example::Outer::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Child::Child");
}

#[test]
fn traces_java_explicit_local_import_simple_generic_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base<T> { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; import com.base.Base; class Child extends Base<String> { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_qualified_generic_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base<T> { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; class Child extends com.base.Base<String> { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_qualified_simple_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; class Child extends com.base.Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_explicit_local_import_simple_superclasses_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let child_dir = dir.join("src").join("com").join("child");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &base_path,
        "package com.base; class Base { Base() {} int helper() { return 1; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.child; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.child; import com.base.Base; class Child extends Base { Child() { super(); } int caller() { return helper(); } }
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &child_path,
        overlay,
        "com::base::Base::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Child::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &child_path,
        overlay,
        "com::base::Base::Base",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Child::Child");
}

#[test]
fn traces_java_var_arity_method_hop_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
    Group inner(int value) { return this; }
    Group makeFoo(int value) { return new Group(); }
}
class Util {
    static Group make(int value) { return new Group(); }
}
class Caller {
    Group group = new Group();
    Group makeFoo(int value) { return new Group(); }
    int bareArityFactoryHop() {
        var v = makeFoo(1).entry;
        return v.helper(1);
    }
    int boundArityFactoryHop() {
        var v = group.makeFoo(1).entry;
        return v.helper(1);
    }
    int staticArityFactoryHop() {
        var v = Util.make(1).entry;
        return v.helper(1);
    }
    int arityChainHop() {
        var v = group.makeFoo(1).inner(0).entry;
        return v.helper(1);
    }
    int directArityHop() {
        return group.makeFoo(1).entry.helper(1);
    }
    int arityMemberHop() {
        return group.inner(1).entry.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 6);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::arityChainHop",
            "com::example::Caller::arityMemberHop",
            "com::example::Caller::bareArityFactoryHop",
            "com::example::Caller::boundArityFactoryHop",
            "com::example::Caller::directArityHop",
            "com::example::Caller::staticArityFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 6);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::arityChainHop",
            "com::example::Caller::arityMemberHop",
            "com::example::Caller::bareArityFactoryHop",
            "com::example::Caller::boundArityFactoryHop",
            "com::example::Caller::directArityHop",
            "com::example::Caller::staticArityFactoryHop"
        ]
    );
}

#[test]
fn traces_java_var_arity_method_hop_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Caller {
    Group makeFoo(int value) { return new Group(); }
    int run() {
        var v = makeFoo(1).entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_arity_method_hop_field_receiver_calls_across_files_with_static_import() {
    let dir = temporary_dir();
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_path = factory_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let helper_path = helper_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Helper { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Helper;
public class Util {
    public static Holder make(int value) { return new Holder(); }
    public static class Holder {
        public Helper entry = new Helper();
        public static Holder nestedMake(int value) { return new Holder(); }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.factory.Util.make;
import static pkg.factory.Util.Holder.nestedMake;
public class Caller {
    public int importedArityFactoryHop() {
        var v = make(1).entry;
        return v.helper(1);
    }
    public int importedNestedArityFactoryHop() {
        var v = nestedMake(1).entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedArityFactoryHop",
            "pkg::caller::Caller::importedNestedArityFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedArityFactoryHop",
            "pkg::caller::Caller::importedNestedArityFactoryHop"
        ]
    );
}

#[test]
fn java_var_arity_method_hop_field_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Util {
    static Group make(int value) { return new Group(); }
}
class Caller {
    Group make(int value) { return new Group(); }
    Group makeTwo(int a, int b) { return new Group(); }
    void makeVoid() { }
    int primitive() { return 0; }
    int arityMismatchLow() {
        var v = make().entry;
        return v.helper(1);
    }
    int arityMismatchHigh() {
        var v = make(1, 2).entry;
        return v.helper(1);
    }
    int multiParameterMismatch() {
        var v = makeTwo(1).entry;
        return v.helper(1);
    }
    int staticArityMismatch() {
        var v = Util.make(1, 2).entry;
        return v.helper(1);
    }
    int unknownFactory() {
        var v = missing(1).entry;
        return v.helper(1);
    }
    int voidFactory() {
        var v = makeVoid().entry;
        return v.helper(1);
    }
    int primitiveFactory() {
        var v = primitive().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "arity-mismatched, unknown, void-returning, and primitive-returning method-call hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_explicit_this_method_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Counter.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;\nclass Counter {\n    int helper() { return 1; }\n    int caller() { return this.helper(); }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Counter::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Counter::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Counter::caller"
    );
}

#[test]
fn traces_java_explicit_local_static_import_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport static com.example.Helper.utility;\nimport static com.example.Helper.instance;\nclass Main {\n    int caller() { return utility(1); }\n    int nonStatic() { return instance(1); }\n}\nclass Competing {\n    int utility(long value) { return (int) value; }\n    int caller() { return utility(1); }\n}\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example;\nclass Helper {\n    static int utility(int value) { return value; }\n    int instance(int value) { return value; }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    let instance_symbol = "com::example::Helper::instance";
    let live_instance = trace_symbol_graph(&dir, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(live_instance.callers.is_empty());
    let persisted_instance =
        trace_symbol_graph_from_index(&db_path, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted_instance.callers.is_empty());
}

#[test]
fn ignores_ambiguous_java_explicit_static_import_calls_in_live_and_persisted_indexes() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let first_path = source_dir.join("First.java");
    let second_path = source_dir.join("Second.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport static com.example.First.utility;\nimport static com.example.Second.utility;\nclass Main { int caller() { return utility(1); } }\n",
    )
    .unwrap();
    fs::write(
        &first_path,
        "package com.example; class First { static int utility(int value) { return value; } }\n",
    )
    .unwrap();
    fs::write(
        &second_path,
        "package com.example; class Second { static int utility(int value) { return value; } }\n",
    )
    .unwrap();

    let first_symbol = "com::example::First::utility";
    let live = trace_symbol_graph(&dir, first_symbol, TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, first_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_explicit_local_import_static_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;\nimport com.example.Helper;\nclass Main {\n    int caller() { return Helper.utility(1); }\n    int shadowed(Helper Helper) { return Helper.utility(1); }\n    int nonStatic() { return Helper.instance(1); }\n}\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example;\nclass Helper {\n    static int utility(int value) { return value; }\n    int instance(int value) { return value; }\n}\n",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    let instance_symbol = "com::example::Helper::instance";
    let live_instance = trace_symbol_graph(&dir, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(live_instance.callers.is_empty());
    let persisted_instance =
        trace_symbol_graph_from_index(&db_path, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted_instance.callers.is_empty());
}

#[test]
fn traces_java_same_file_static_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper {
    static int utility(int value) { return value; }
    static int flexible(int... values) { return values.length; }
    int instance(int value) { return value; }
}
class Main {
    int caller() { return Helper.utility(1); }
    int parameterShadowed(Helper Helper) { return Helper.utility(1); }
    int localTypeShadowed() { class Helper {} return Helper.utility(1); }
    int nonStatic() { return Helper.instance(1); }
    int varargs() { return Helper.flexible(1); }
}
class FieldShadowing {
    private Helper Helper;
    int fieldShadowed() { return Helper.utility(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    for target in [
        "com::example::Helper::instance",
        "com::example::Helper::flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn traces_java_same_file_static_type_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    let overlay = "class Helper { static int utility(int value) { return value; } }
class Main { int caller() { return Helper.utility(1); } }
";
    fs::write(
        &source_path,
        "class Stale {}
",
    )
    .unwrap();

    let helper_symbol = "Helper::utility";
    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Main::caller");
}

#[test]
fn traces_java_same_package_outer_static_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let outer_path = source_dir.join("Outer.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main { int caller() { return Outer.Helper.utility(1); } int shadowed(Outer Outer) { return Outer.Helper.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { static class Helper { static int utility(int value) { return value; } int instance(int value) { return value; } } }
",
    )
    .unwrap();

    let helper_symbol = "com::example::Outer::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    let instance_symbol = "com::example::Outer::Helper::instance";
    let live_instance = trace_symbol_graph(&dir, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(live_instance.callers.is_empty());
    let persisted_instance =
        trace_symbol_graph_from_index(&db_path, instance_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted_instance.callers.is_empty());
}

#[test]
fn traces_java_outer_static_type_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let outer_path = source_dir.join("Outer.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { static class Helper { static int utility(int value) { return value; } } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay =
        "package com.example; class Main { int caller() { return Outer.Helper.utility(1); } }
";
    let helper_symbol = "com::example::Outer::Helper::utility";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_explicit_imported_outer_static_type_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("com").join("base");
    let caller_dir = dir.join("src").join("com").join("child");
    let caller_path = caller_dir.join("Main.java");
    let outer_path = outer_dir.join("Outer.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.child; import com.base.Outer; class Main { int caller() { return Outer.Helper.utility(1); } int shadowed(Outer Outer) { return Outer.Helper.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package com.base; class Outer { static class Helper { static int utility(int value) { return value; } int instance(int value) { return value; } } }
",
    )
    .unwrap();

    let helper_symbol = "com::base::Outer::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, helper_symbol);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Main::caller");
}

#[test]
fn traces_java_same_package_static_interface_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let interface_path = source_dir.join("Tools.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main { int caller() { return Tools.utility(1); } int shadowed(Tools Tools) { return Tools.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package com.example; interface Tools { static int utility(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Tools::utility";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_static_interface_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Tools.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; interface Tools { static int utility(int value) { return value; } } class Main { int caller() { return Tools.utility(1); } }
";
    let target = "com::example::Tools::utility";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_same_package_default_interface_methods_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let interface_path = source_dir.join("Defaults.java");
    let abstract_interface_path = source_dir.join("Abstracts.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;
class Main implements Defaults { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } }
class AbstractMain implements Abstracts { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package com.example; interface Defaults { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &abstract_interface_path,
        "package com.example; interface Abstracts { int helper(int value); }
",
    )
    .unwrap();

    let default_target = "com::example::Defaults::helper";
    let abstract_target = "com::example::Abstracts::helper";
    let live = trace_symbol_graph(&dir, default_target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 2);
    assert_eq!(
        live.callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
    assert!(
        trace_symbol_graph(&dir, abstract_target, TraceDirection::Callers)
            .unwrap()
            .callers
            .is_empty()
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, default_target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 2);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
    assert!(
        trace_symbol_graph_from_index(&db_path, abstract_target, TraceDirection::Callers)
            .unwrap()
            .callers
            .is_empty()
    );
}

#[test]
fn traces_java_unambiguous_default_methods_across_multiple_direct_interfaces() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let primary_path = source_dir.join("Primary.java");
    let empty_path = source_dir.join("Empty.java");
    let abstract_path = source_dir.join("Abstracts.java");
    let secondary_path = source_dir.join("Secondary.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main implements Primary, Empty { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } } class AbstractBlocked implements Primary, Abstracts { int caller() { return helper(1); } } class DefaultBlocked implements Primary, Secondary { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &primary_path,
        "package com.example; interface Primary { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &empty_path,
        "package com.example; interface Empty {}
",
    )
    .unwrap();
    fs::write(
        &abstract_path,
        "package com.example; interface Abstracts { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &secondary_path,
        "package com.example; interface Secondary { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Primary::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 5);
    assert_eq!(
        live.callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 5);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
}

#[test]
fn traces_java_unique_default_interface_inheritance_chains_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let child_path = source_dir.join("Child.java");
    let root_path = source_dir.join("Root.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main implements Child { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.example; interface Child extends Root {}
",
    )
    .unwrap();
    fs::write(
        &root_path,
        "package com.example; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Root::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(
        live.callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
}

#[test]
fn traces_java_same_package_outer_default_interface_inheritance_chains() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let outer_path = source_dir.join("Outer.java");
    let root_path = source_dir.join("Root.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main implements Outer.Child { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &outer_path,
        "package com.example; class Outer { interface Child extends Root {} }
",
    )
    .unwrap();
    fs::write(
        &root_path,
        "package com.example; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Root::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_explicit_imported_outer_default_interface_inheritance_chains() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("com").join("base");
    let caller_dir = dir.join("src").join("com").join("child");
    let outer_path = base_dir.join("Outer.java");
    let root_path = base_dir.join("Root.java");
    let caller_path = caller_dir.join("Main.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package com.base; class Outer { interface Child extends Root {} }
",
    )
    .unwrap();
    fs::write(
        &root_path,
        "package com.base; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package com.child; import com.base.Outer; class Main implements Outer.Child { int caller() { return helper(1); } }
",
    )
    .unwrap();

    let target = "com::base::Root::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Main::caller");
}

#[test]
fn traces_java_explicit_imported_default_interface_inheritance_chains() {
    let dir = temporary_dir();
    let root_dir = dir.join("src").join("com").join("root");
    let middle_dir = dir.join("src").join("com").join("middle");
    let caller_dir = dir.join("src").join("com").join("child");
    let root_path = root_dir.join("Root.java");
    let child_path = middle_dir.join("Child.java");
    let caller_path = caller_dir.join("Main.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&root_dir).unwrap();
    fs::create_dir_all(&middle_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &root_path,
        "package com.root; interface Root { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package com.middle; import com.root.Root; interface Child extends Root {}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package com.child; import com.middle.Child; class Main implements Child { int caller() { return helper(1); } }
",
    )
    .unwrap();

    let target = "com::root::Root::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Main::caller");

    let caller_overlay = "package com.child; import com.middle.Child; class Main implements Child { int caller() { return this.helper(1); } }
";
    let live_overlay = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live_overlay.callers.len(), 1);
    assert_eq!(
        live_overlay.callers[0].symbol_id,
        "com::child::Main::caller"
    );

    let persisted_overlay = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_overlay.callers.len(), 1);
    assert_eq!(
        persisted_overlay.callers[0].symbol_id,
        "com::child::Main::caller"
    );
}

#[test]
fn traces_java_default_interface_methods_through_unique_empty_superclass_chains() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let base_path = source_dir.join("Base.java");
    let blocking_base_path = source_dir.join("BlockingBase.java");
    let interface_path = source_dir.join("Defaults.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Main extends Base implements Defaults { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } } class Blocked extends BlockingBase implements Defaults { int caller() { return helper(1); } int thisCaller() { return this.helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &base_path,
        "package com.example; class Base {}
",
    )
    .unwrap();
    fs::write(
        &blocking_base_path,
        "package com.example; class BlockingBase { int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package com.example; interface Defaults { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::example::Defaults::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 4);
    assert_eq!(
        live.callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 4);
    assert_eq!(
        persisted
            .callers
            .iter()
            .map(|caller| caller.symbol_id.as_str())
            .collect::<Vec<_>>(),
        [
            "com::example::Main::caller",
            "com::example::Main::thisCaller"
        ]
    );
}

#[test]
fn traces_java_default_interface_inheritance_chains_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Root.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; class Base {} interface Root { default int helper(int value) { return value; } } interface Child extends Root {} class Main extends Base implements Child { int caller() { return this.helper(1); } }
";
    let target = "com::example::Root::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_default_interface_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Defaults.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example; interface Defaults { default int helper(int value) { return value; } } interface Empty {} class Main implements Defaults, Empty { int caller() { return this.helper(1); } }
";
    let target = "com::example::Defaults::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        target,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_explicit_imported_default_interface_methods_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let interface_dir = dir.join("src").join("com").join("base");
    let caller_dir = dir.join("src").join("com").join("child");
    let caller_path = caller_dir.join("Main.java");
    let interface_path = interface_dir.join("Defaults.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&interface_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.child; import com.base.Defaults; class Main implements Defaults { int caller() { return helper(1); } }
",
    )
    .unwrap();
    fs::write(
        &interface_path,
        "package com.base; interface Defaults { default int helper(int value) { return value; } }
",
    )
    .unwrap();

    let target = "com::base::Defaults::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::child::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::child::Main::caller");
}

#[test]
fn traces_java_same_package_static_type_calls_across_files_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example;
class Main {
    int caller() { return Helper.utility(1); }
    int parameterShadowed(Helper Helper) { return Helper.utility(1); }
    int nonStatic() { return Helper.instance(1); }
    int varargs() { return Helper.flexible(1); }
}
",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example;
class Helper {
    static int utility(int value) { return value; }
    static int flexible(int... values) { return values.length; }
    int instance(int value) { return value; }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::utility";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");

    for target in [
        "com::example::Helper::instance",
        "com::example::Helper::flexible",
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty());
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty());
    }
}

#[test]
fn ignores_ambiguous_java_same_package_static_type_calls_across_files() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        source_dir.join("Main.java"),
        "package com.example; class Main { int caller() { return Helper.utility(1); } }
",
    )
    .unwrap();
    fs::write(
        source_dir.join("First.java"),
        "package com.example; class Helper { static int utility(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        source_dir.join("Second.java"),
        "package com.example; class Helper { static int utility(int value) { return value; } }
",
    )
    .unwrap();

    let live =
        trace_symbol_graph(&dir, "com::example::Main::caller", TraceDirection::Callees).unwrap();
    assert!(live.callees.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Main::caller",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(persisted.callees.is_empty());
}

#[test]
fn traces_java_same_package_static_type_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let caller_path = source_dir.join("Main.java");
    let helper_path = source_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &caller_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package com.example; class Helper { static int utility(int value) { return value; } }
",
    )
    .unwrap();
    let overlay = "package com.example; class Main { int caller() { return Helper.utility(1); } }
";
    let helper_symbol = "com::example::Helper::utility";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Main::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Main::caller");
}

#[test]
fn traces_java_typed_parameter_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.symbol.symbol_id, helper_symbol);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_typed_parameter_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_typed_local_and_field_receiver_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    private Helper fieldHelper = new Helper();
    int run() {
        Helper local = new Helper();
        return local.helper(1) + fieldHelper.helper(2);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_typed_receiver_calls_across_files_with_explicit_import() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.helper.Foo;
class Bar {
    int run(Foo foo) { return foo.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_typed_receiver_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper(int value) { return value; } }
class Base extends Grand {}
class Caller {
    int run(Base base) { return base.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_typed_receiver_calls_fail_closed_for_unknown_types_and_shadowed_static_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { static int run(int value) { return value; } }
class Caller {
    int unknownType(Unknown value) { return value.helper(1); }
    int memberChain(Helper helper) { return helper.inner.helper(1); }
    int lambdaShadowed() {
        java.util.function.IntFunction<Integer> function = Helper -> Helper.run(1);
        return function.apply(0);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "bound but unresolvable receivers must not fall through to static type calls"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_constructor_inferred_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() {
        var helper = new Helper();
        return helper.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_constructor_inferred_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() {
        var helper = new Helper();
        return helper.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_constructor_inferred_nested_receiver_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Outer {
    static class Inner { int helper(int value) { return value; } }
}
class Caller {
    int run() {
        var inner = new Outer.Inner();
        return inner.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_constructor_inferred_receiver_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper(int value) { return value; } }
class Base extends Grand {}
class Helper extends Base {}
class Caller {
    int run() {
        var helper = new Helper();
        return helper.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_var_constructor_receiver_calls_fail_closed_without_constructor_initializers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { static int run(int value) { return value; } }
class Caller {
    int run() {
        var factory = makeHelper();
        var missing = new Missing();
        return factory.run(1) + missing.run(2);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown-factory and missing-constructor var initializers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_constructor_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() { return new Helper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() { return new Helper().helper(1); }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_receiver_nested_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Outer {
    static class Inner { int helper(int value) { return value; } }
}
class Caller {
    int run() { return new Outer.Inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_receiver_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper(int value) { return value; } }
class Base extends Grand {}
class Helper extends Base {}
class Caller {
    int run() { return new Helper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_constructor_receiver_calls_fail_closed_for_unknown_and_anonymous_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int run(int value) { return value; } }
class Caller {
    int missing() { return new Missing().run(1); }
    int overridden() {
        return new Helper() { int run(int value) { return value + 1; } }.run(2);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown constructor types and anonymous bodies that declare the invoked member must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_anonymous_constructor_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int direct() { return new Helper() { }.helper(1); }
    int directWithBody() {
        return new Helper() { int other() { return 0; } }.helper(2);
    }
    int varInitializer() {
        var v = new Helper() { };
        return v.helper(3);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::direct",
            "com::example::Caller::directWithBody",
            "com::example::Caller::varInitializer"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::direct",
            "com::example::Caller::directWithBody",
            "com::example::Caller::varInitializer"
        ]
    );
}

#[test]
fn traces_java_anonymous_constructor_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run() {
        return new Helper() { }.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_anonymous_constructor_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int overridden() {
        return new Helper() { int helper(int value) { return value + 1; } }.helper(1);
    }
    int arityOverride() {
        return new Helper() { int helper() { return 0; } }.helper(1);
    }
    int missingType() {
        return new Missing() { }.helper(1);
    }
    int chained() {
        return new Helper() { }.inner().helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "anonymous receivers with overriding bodies, unknown constructed types, and chains with unknown hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_anonymous_constructor_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper inner = new Helper();
    Group inner2() { return this; }
    Group inner2(int value) { return this; }
}
class Outer { static class Inner { int helper(int value) { return value; } } }
class Caller {
    int fieldChain() { return new Group() { }.inner.helper(1); }
    int methodHopChain() { return new Group() { }.inner2().inner.helper(2); }
    int methodHopArgChain() { return new Group() { }.inner2(1).inner.helper(5); }
    int bodyUnrelated() {
        return new Group() { int other() { return 0; } }.inner.helper(3);
    }
    int nested() { return new Outer.Inner() { }.helper(4); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bodyUnrelated",
            "com::example::Caller::fieldChain",
            "com::example::Caller::methodHopArgChain",
            "com::example::Caller::methodHopChain"
        ]
    );

    let nested_symbol = "com::example::Outer::Inner::helper";
    let nested_live = trace_symbol_graph(&dir, nested_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(nested_live.callers.len(), 1);
    assert_eq!(
        nested_live.callers[0].symbol_id,
        "com::example::Caller::nested"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bodyUnrelated",
            "com::example::Caller::fieldChain",
            "com::example::Caller::methodHopArgChain",
            "com::example::Caller::methodHopChain"
        ]
    );
    let nested_persisted =
        trace_symbol_graph_from_index(&db_path, nested_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(nested_persisted.callers.len(), 1);
    assert_eq!(
        nested_persisted.callers[0].symbol_id,
        "com::example::Caller::nested"
    );
}

#[test]
fn traces_java_anonymous_constructor_chain_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Group { Helper inner = new Helper(); }
class Caller {
    int run() {
        return new Group() { }.inner.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_anonymous_constructor_chain_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Other { int helper(int value) { return value + 10; } }
class Group {
    Helper inner = new Helper();
    Group inner2() { return this; }
}
class Caller {
    int overrideFinal() {
        return new Group() { int helper(int value) { return value + 1; } }.inner.helper(1);
    }
    int overrideHop() {
        return new Group() { int inner2() { return this; } }.inner2().inner.helper(2);
    }
    int overrideArgHop() {
        return new Group() { Group inner2(int value) { return this; } }.inner2(1).inner.helper(2);
    }
    int arityMismatch() {
        return new Group() { }.inner2(1).inner.helper(2);
    }
    int fieldShadow() {
        return new Group() { Other inner = new Other(); }.inner.helper(2);
    }
    int missingType() {
        return new Missing() { }.inner.helper(1);
    }
    int unknownHop() {
        return new Group() { }.missing().inner.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "anonymous-rooted chains with overriding or field-shadowing bodies, arity-mismatched hops, unknown constructed types, and unknown hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_anonymous_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper entry = new Helper();
    Group entry2() { return this; }
    Group entry2(int value) { return this; }
    Holder holder = new Holder();
}
class Holder { Helper entry = new Helper(); }
class Outer { static class Inner { Helper entry = new Helper(); } }
class Caller {
    int varField() {
        var v = new Group() { }.entry;
        return v.helper(1);
    }
    int varFieldWithBody() {
        var v = new Group() { int other() { return 0; } }.entry;
        return v.helper(2);
    }
    int varFieldNested() {
        var v = new Outer.Inner() { }.entry;
        return v.helper(3);
    }
    int varFieldWithArgHop() {
        var v = new Group() { }.entry2(1).entry;
        return v.helper(6);
    }
    int varFieldWithHop() {
        var v = new Group() { }.entry2().entry;
        return v.helper(4);
    }
    int varFieldWithFieldHop() {
        var v = new Group() { }.holder.entry;
        return v.helper(5);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 6);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::varField",
            "com::example::Caller::varFieldNested",
            "com::example::Caller::varFieldWithArgHop",
            "com::example::Caller::varFieldWithBody",
            "com::example::Caller::varFieldWithFieldHop",
            "com::example::Caller::varFieldWithHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 6);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::varField",
            "com::example::Caller::varFieldNested",
            "com::example::Caller::varFieldWithArgHop",
            "com::example::Caller::varFieldWithBody",
            "com::example::Caller::varFieldWithFieldHop",
            "com::example::Caller::varFieldWithHop"
        ]
    );
}

#[test]
fn traces_java_var_anonymous_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper entry = new Helper();
    Group entry2() { return this; }
    Group entry2(int value) { return this; }
}
class Caller {
    int run() {
        var v = new Group() { }.entry;
        return v.helper(1);
    }
    int runHop() {
        var v = new Group() { }.entry2().entry;
        return v.helper(2);
    }
    int runArgHop() {
        var v = new Group() { }.entry2(1).entry;
        return v.helper(3);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::run",
            "com::example::Caller::runArgHop",
            "com::example::Caller::runHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::run",
            "com::example::Caller::runArgHop",
            "com::example::Caller::runHop"
        ]
    );
}

#[test]
fn java_var_anonymous_field_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper entry = new Helper();
    Group entry2() { return this; }
}
class Caller {
    int shadowedField() {
        var v = new Group() { Helper entry = new Helper(); }.entry;
        return v.helper(1);
    }
    int shadowedHop() {
        var v = new Group() { Group entry2() { return this; } }.entry2().entry;
        return v.helper(1);
    }
    int shadowedArgHop() {
        var v = new Group() { Group entry2(int value) { return this; } }.entry2(1).entry;
        return v.helper(1);
    }
    int missingType() {
        var v = new Missing() { }.entry;
        return v.helper(1);
    }
    int unknownChain() {
        var v = new Group() { }.missing.entry;
        return v.helper(1);
    }
    int argHop() {
        var v = new Group() { }.entry2(1).entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "anonymous var field-initializer chains with shadowing bodies, unknown constructed types, unknown chains, and arity-mismatched or shadowed argument hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_interface_typed_parameter_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { int helper(int value); }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_typed_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
interface Helper { int helper(int value); }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_typed_receiver_default_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
class Caller {
    int run(Helper helper) { return helper.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_typed_receiver_calls_across_files_with_explicit_import() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public interface Foo { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.helper.Foo;
public class Bar {
    public int run(Foo foo) { return foo.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn java_interface_typed_receiver_calls_fail_closed_when_interface_lacks_declaration() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper {}
class Impl implements Helper { int run(int value) { return value; } }
class Caller {
    int run(Helper helper) { return helper.run(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Impl::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "receivers typed as an interface must not guess implementation methods"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_interface_inherited_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { int helper(int value); }
interface Mid extends Base {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
interface Base { int helper(int value); }
interface Mid extends Base {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
";
    let helper_symbol = "com::example::Base::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_default_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { default int helper(int value) { return value; } }
interface Mid extends Base {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("pkg").join("base");
    let mid_dir = dir.join("src").join("pkg").join("mid");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let base_path = base_dir.join("Base.java");
    let mid_path = mid_dir.join("Mid.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&mid_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &base_path,
        "package pkg.base;
public interface Base { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &mid_path,
        "package pkg.mid;
import pkg.base.Base;
public interface Mid extends Base {}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.mid.Mid;
public class Bar {
    public int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::base::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_member_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { int helper(int value); }
interface Mid extends Base {}
class Group { Mid mid; }
class Caller {
    int run(Group group) { return group.mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_interface_inherited_receiver_calls_fail_closed_for_unresolvable_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { static int run(int value) { return value; } }
interface Other { static int run(int value) { return value; } }
interface Branching extends Base, Other {}
interface Missing extends Unknown {}
class Impl implements Base {}
class Caller {
    int branching(Branching branching) { return branching.run(1); }
    int missingParent(Missing missing) { return missing.run(1); }
    int classReceiver(Impl impl) { return impl.run(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Base::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "branching or unresolved interface chains, static interface members, and class receivers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_class_receiver_interface_default_methods_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
class Impl implements Helper {}
class Caller {
    int run(Impl impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_class_receiver_interface_default_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
interface Helper { default int helper(int value) { return value; } }
class Impl implements Helper {}
class Caller {
    int run(Impl impl) { return impl.helper(1); }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_class_receiver_interface_default_methods_across_files_with_explicit_import() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let impl_dir = dir.join("src").join("pkg").join("impl");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Helper.java");
    let impl_path = impl_dir.join("Impl.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&impl_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public interface Helper { default int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &impl_path,
        "package pkg.impl;
import pkg.helper.Helper;
public class Impl implements Helper {}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.impl.Impl;
public class Bar {
    public int run(Impl impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_class_receiver_interface_default_methods_through_shared_receiver_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
class Impl implements Helper {}
class Group { Impl impl; }
class Caller {
    int newCall() { return new Impl().helper(1); }
    int varCall() { var x = new Impl(); return x.helper(1); }
    int chainCall(Group group) { return group.impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::chainCall",
            "com::example::Caller::newCall",
            "com::example::Caller::varCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::chainCall",
            "com::example::Caller::newCall",
            "com::example::Caller::varCall"
        ]
    );
}

#[test]
fn java_class_receiver_interface_default_methods_fail_closed_for_nearer_class_declarations() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
class StaticImpl implements Helper { static int helper(int value) { return value; } }
class ArityImpl implements Helper { int helper() { return 0; } }
class Caller {
    int staticCall(StaticImpl impl) { return impl.helper(1); }
    int arityCall(ArityImpl impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "same-name methods nearer in the receiver class hierarchy must suppress interface default dispatch"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn java_class_receiver_interface_default_methods_fail_closed_for_competing_or_unresolved_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Helper { default int helper(int value) { return value; } }
interface Other { default int helper(int value) { return value; } }
class Competing implements Helper, Other {}
class Missing implements Helper, Unknown {}
interface StaticHelper { static int helper(int value) { return value; } }
class StaticInterface implements StaticHelper {}
class Caller {
    int competing(Competing impl) { return impl.helper(1); }
    int missing(Missing impl) { return impl.helper(1); }
    int staticInterface(StaticInterface impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "competing defaults, unresolved interfaces, and static-only interface members must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_branching_chains_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { int helper(int value); }
interface Marker {}
interface Mid extends Base, Marker {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_default_methods_through_branching_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { default int helper(int value) { return value; } }
interface Marker {}
interface Mid extends Base, Marker {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_branching_chains_from_dirty_vfs_overrides()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
interface Base { default int helper(int value) { return value; } }
interface Marker {}
interface Mid extends Base, Marker {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
";
    let helper_symbol = "com::example::Base::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_branching_chains_across_files_with_imports()
 {
    let dir = temporary_dir();
    let base_dir = dir.join("src").join("pkg").join("base");
    let marker_dir = dir.join("src").join("pkg").join("marker");
    let mid_dir = dir.join("src").join("pkg").join("mid");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let base_path = base_dir.join("Base.java");
    let marker_path = marker_dir.join("Marker.java");
    let mid_path = mid_dir.join("Mid.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&marker_dir).unwrap();
    fs::create_dir_all(&mid_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &base_path,
        "package pkg.base;
public interface Base { int helper(int value); }
",
    )
    .unwrap();
    fs::write(
        &marker_path,
        "package pkg.marker;
public interface Marker {}
",
    )
    .unwrap();
    fs::write(
        &mid_path,
        "package pkg.mid;
import pkg.base.Base;
import pkg.marker.Marker;
public interface Mid extends Base, Marker {}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.mid.Mid;
public class Bar {
    public int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::base::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_interface_inherited_receiver_calls_through_diamond_chains_resolve_once() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Root { int helper(int value); }
interface Left extends Root {}
interface Right extends Root {}
interface Mid extends Left, Right {}
class Caller {
    int run(Mid mid) { return mid.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Root::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_class_receiver_interface_default_methods_through_branching_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { default int helper(int value) { return value; } }
interface Marker {}
interface Helper extends Base, Marker {}
class Impl implements Helper {}
class Caller {
    int run(Impl impl) { return impl.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_interface_inherited_receiver_calls_fail_closed_for_competing_or_unresolvable_branches() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
interface Base { int helper(int value); }
interface Other { int helper(int value); }
interface Competing extends Base, Other {}
interface DefaultOther { default int helper(int value) { return value; } }
interface CompetingDefaults extends Base, DefaultOther {}
interface Missing extends Base, Unknown {}
interface StaticBranch { static int helper(int value) { return value; } }
interface StaticOnly extends Base, StaticBranch {}
interface Root { int helper(int value); }
interface CycleA extends CycleB {}
interface CycleB extends CycleA {}
interface Cyclic extends Root, CycleA {}
class Caller {
    int competing(Competing value) { return value.helper(1); }
    int competingDefaults(CompetingDefaults value) { return value.helper(1); }
    int missing(Missing value) { return value.helper(1); }
    int staticOnly(StaticOnly value) { return value.helper(1); }
    int cyclic(Cyclic value) { return value.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Base::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "competing, unresolved, static-only, and cyclic branches must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_generic_receiver_parameter_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Caller {
    int run(Box<String> box) { return box.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_generic_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Caller {
    int run(Box<String> box) { return box.helper(1); }
}
";
    let helper_symbol = "com::example::Box::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_generic_receiver_calls_through_member_chains_and_constructors() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Group { Box<String> box; }
class Caller {
    int newCall() { return new Box<String>().helper(1); }
    int varCall() { var box = new Box<String>(); return box.helper(1); }
    int chainCall(Group group) { return group.box.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::chainCall",
            "com::example::Caller::newCall",
            "com::example::Caller::varCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::chainCall",
            "com::example::Caller::newCall",
            "com::example::Caller::varCall"
        ]
    );
}

#[test]
fn traces_java_generic_receiver_calls_through_factory_initializers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Caller {
    Box<String> makeBox() { return new Box<String>(); }
    int run() { var box = makeBox(); return box.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_generic_receiver_calls_fail_closed_for_array_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Caller {
    int run(Box<String>[] boxes) { return boxes.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "array-typed receivers must fail closed instead of guessing a raw element type"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_member_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int run(Group group) { return group.inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_member_chain_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int run(Group group) { return group.inner.helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_member_chain_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.helper.Foo;
public class Group { public Foo inner = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
public class Bar {
    public int run(Group group) { return group.inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_member_chain_receiver_calls_through_var_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int varReceiver() {
        var group = new Group();
        return group.inner.helper(1);
    }
    int constructorReceiver() { return new Group().inner.helper(2); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorReceiver",
            "com::example::Caller::varReceiver"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorReceiver",
            "com::example::Caller::varReceiver"
        ]
    );
}

#[test]
fn traces_java_constructor_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int run() { return new Group().inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_chain_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    int run() { return new Group().inner.helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_constructor_chain_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let inner_path = inner_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.inner.Foo;
public class Group { public Foo inner = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
public class Bar {
    public int run() { return new Group().inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_constructor_chain_receiver_calls_through_deep_chains() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Holder { Helper helper = new Helper(); }
class Group { Holder holder = new Holder(); }
class Caller {
    int run() { return new Group().holder.helper.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_constructor_chain_receiver_calls_fail_closed_for_unresolvable_bases() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { static int run(int value) { return value; } }
class Group { Inner inner = new Inner(); }
class Caller {
    static Group makeGroup() { return new Group(); }
    int functionCallBase() { return makeGroup().inner.run(1); }
    int unknownHop() { return new Group().missing.run(1); }
    int staticMember() { return new Group().inner.run(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "function-call bases, unknown chain hops, and static final members must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn java_member_chain_receiver_calls_fail_closed_for_unknown_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { static int run(int value) { return value; } }
class Group {}
class Caller {
    int missingField(Group group) { return group.missing.run(1); }
    int unknownHopType(Group group) { return group.inner.run(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown chain hops and static final members must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_factory_inferred_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    static Helper makeHelper() { return new Helper(); }
    int run() {
        var factory = makeHelper();
        return factory.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_factory_inferred_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    static Helper makeHelper() { return new Helper(); }
    int run() {
        var factory = makeHelper();
        return factory.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_factory_inferred_receiver_calls_across_files_with_static_import() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let factory_path = factory_dir.join("Fact.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Foo;
public class Fact {
    public static Foo makeFoo() { return new Foo(); }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.factory.Fact.makeFoo;
public class Bar {
    public int run() {
        var foo = makeFoo();
        return foo.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_factory_inferred_nested_receiver_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Outer {
    static class Inner { int helper(int value) { return value; } }
}
class Caller {
    static Outer.Inner makeInner() { return new Outer.Inner(); }
    int run() {
        var inner = makeInner();
        return inner.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_factory_inferred_receiver_inherited_methods() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Grand { int helper(int value) { return value; } }
class Base extends Grand {}
class Caller {
    static Base makeBase() { return new Base(); }
    int run() {
        var base = makeBase();
        return base.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Grand::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_factory_inferred_receiver_calls_fail_closed_for_unresolvable_factories() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { static int run(int value) { return value; } }
class Util {
    static Helper makeHelper(int value) { return new Helper(); }
}
class Caller {
    static void makeVoid() {}
    static int makeInt() { return 1; }
    static Helper makeHelper() { return new Helper(); }
    static Helper makeHelper(int value) { return new Helper(); }
    static Helper makeHelper(String value) { return new Helper(); }
    int unknownFactory() {
        var factory = makeUnknown();
        return factory.run(1);
    }
    int qualifiedInitializer() {
        var factory = Util.makeHelper(1);
        return factory.run(1);
    }
    int voidFactory() {
        var factory = makeVoid();
        return factory.run(1);
    }
    int primitiveFactory() {
        var factory = makeInt();
        return factory.run(1);
    }
    int arityMismatch() {
        var factory = makeHelper(1, 2);
        return factory.run(1);
    }
    int ambiguousOverload() {
        var factory = makeHelper(1);
        return factory.run(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::run";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown, qualified, void/primitive-return, arity-mismatched, and ambiguous factory initializers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_method_hop_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner() { return new Inner(); } }
class Caller {
    int run(Group group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner inner() { return new Inner(); } }
class Caller {
    int run(Group group) { return group.inner().helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Deep { int helper(int value) { return value; } }
class Inner { Deep deeper() { return new Deep(); } }
class Group { Inner inner() { return new Inner(); } }
class Holder { Group group; }
class Caller {
    int newCall() { return new Group().inner().deeper().helper(1); }
    int varCall() { var group = new Group(); return group.inner().deeper().helper(1); }
    int paramCall(Group group) { return group.inner().deeper().helper(1); }
    int fieldHopCall(Holder holder) { return holder.group.inner().deeper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Deep::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldHopCall",
            "com::example::Caller::newCall",
            "com::example::Caller::paramCall",
            "com::example::Caller::varCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldHopCall",
            "com::example::Caller::newCall",
            "com::example::Caller::paramCall",
            "com::example::Caller::varCall"
        ]
    );
}

#[test]
fn traces_java_method_hop_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let inner_path = inner_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.inner.Foo;
public class Group { public Foo inner() { return new Foo(); } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
public class Bar {
    public int run(Group group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_interface_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IGroup { Inner inner(); }
class Caller {
    int run(IGroup group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_generic_return_types() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Group { Box<String> inner() { return new Box<String>(); } }
class Caller {
    int run(Group group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Box::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_method_hop_receiver_calls_fail_closed_for_unsupported_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group {
    Inner inner() { return new Inner(); }
    Inner inner(int value) { return new Inner(); }
    int tag() { return 1; }
    void reset() {}
    static Inner make() { return new Inner(); }
}
class Caller {
    int argMismatchHop(Group group) { return group.inner(1, 2).helper(1); }
    int unknownHop(Group group) { return group.unknown().helper(1); }
    int primitiveHop(Group group) { return group.tag().helper(1); }
    int voidHop(Group group) { return group.reset().helper(1); }
    int staticHop(Group group) { return group.make().helper(1); }
    int unboundHop() { return unknown.inner().helper(1); }
    int unknownThisHop() { return this.inner().helper(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "arity-mismatched, unknown, primitive/void-return, static, unbound, and unknown `this`-rooted method-call hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_this_rooted_member_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Caller {
    Inner inner = new Inner();
    int run() { return this.inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_this_rooted_method_hop_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Caller {
    Inner inner() { return new Inner(); }
    int run() { return this.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_this_rooted_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Caller {
    Inner inner = new Inner();
    Inner makeInner() { return new Inner(); }
    int fieldCall() { return this.inner.helper(1); }
    int hopCall() { return this.makeInner().helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldCall",
            "com::example::Caller::hopCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldCall",
            "com::example::Caller::hopCall"
        ]
    );
}

#[test]
fn traces_java_this_rooted_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Deep { int helper(int value) { return value; } }
class Inner { Deep deeper() { return new Deep(); } }
class Holder { Inner inner() { return new Inner(); } }
class Caller {
    Holder holder = new Holder();
    Inner inner() { return new Inner(); }
    int thisHop() { return this.inner().deeper().helper(1); }
    int thisFieldHop() { return this.holder.inner().deeper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Deep::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::thisFieldHop",
            "com::example::Caller::thisHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::thisFieldHop",
            "com::example::Caller::thisHop"
        ]
    );
}

#[test]
fn traces_java_this_rooted_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let inner_path = inner_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.inner.Foo;
public class Group { public Foo foo = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
import pkg.inner.Foo;
public class Bar {
    Group group = new Group();
    Foo makeFoo() { return new Foo(); }
    public int fieldCall() { return this.group.foo.helper(1); }
    public int hopCall() { return this.makeFoo().helper(1); }
}
",
    )
    .unwrap();

    let foo_helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, foo_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        ["pkg::caller::Bar::fieldCall", "pkg::caller::Bar::hopCall"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, foo_helper_symbol, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        ["pkg::caller::Bar::fieldCall", "pkg::caller::Bar::hopCall"]
    );
}

#[test]
fn java_this_rooted_receiver_calls_fail_closed_for_unsupported_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Caller {
    int tag() { return 1; }
    void reset() {}
    static Inner make() { return new Inner(); }
    int unknownHop() { return this.unknown().helper(1); }
    int unknownFieldHop() { return this.missing.helper(1); }
    int primitiveHop() { return this.tag().helper(1); }
    int voidHop() { return this.reset().helper(1); }
    int staticHop() { return this.make().helper(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unknown, missing-field, primitive/void-return, and static `this`-rooted chain hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_super_rooted_method_hop_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Base { Inner inner() { return new Inner(); } }
class Child extends Base {
    int run() { return super.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Child::run");
}

#[test]
fn traces_java_super_rooted_member_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Base { Inner member = new Inner(); }
class Child extends Base {
    int run() { return super.member.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Child::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Child::run");
}

#[test]
fn traces_java_super_rooted_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Base { Inner inner() { return new Inner(); } Inner member = new Inner(); }
class Child extends Base {
    int hopCall() { return super.inner().helper(1); }
    int fieldCall() { return super.member.helper(1); }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Child::fieldCall",
            "com::example::Child::hopCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Child::fieldCall",
            "com::example::Child::hopCall"
        ]
    );
}

#[test]
fn traces_java_super_rooted_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Deep { int helper(int value) { return value; } }
class Inner { Deep deeper() { return new Deep(); } }
class Base { Inner inner() { return new Inner(); } }
class Mid extends Base { Inner member = new Inner(); }
class Child extends Mid {
    int hopCall() { return super.inner().deeper().helper(1); }
    int fieldHopCall() { return super.member.deeper().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Deep::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Child::fieldHopCall",
            "com::example::Child::hopCall"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Child::fieldHopCall",
            "com::example::Child::hopCall"
        ]
    );
}

#[test]
fn traces_java_super_rooted_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let base_dir = dir.join("src").join("pkg").join("base");
    let child_dir = dir.join("src").join("pkg").join("child");
    let inner_path = inner_dir.join("Foo.java");
    let base_path = base_dir.join("Base.java");
    let child_path = child_dir.join("Child.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&base_dir).unwrap();
    fs::create_dir_all(&child_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &base_path,
        "package pkg.base;
import pkg.inner.Foo;
public class Base { public Foo inner() { return new Foo(); } }
",
    )
    .unwrap();
    fs::write(
        &child_path,
        "package pkg.child;
import pkg.base.Base;
public class Child extends Base {
    public int run() { return super.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::child::Child::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::child::Child::run");
}

#[test]
fn java_super_rooted_receiver_calls_fail_closed_for_unsupported_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Base {
    int tag() { return 1; }
    void reset() {}
    static Inner make() { return new Inner(); }
}
class Child extends Base {
    int argHop() { return super.inner(1).helper(1); }
    int unknownHop() { return super.unknown().helper(1); }
    int unknownFieldHop() { return super.missing.helper(1); }
    int primitiveHop() { return super.tag().helper(1); }
    int voidHop() { return super.reset().helper(1); }
    int staticHop() { return super.make().helper(1); }
}
class Solo {
    int noSuperHop() { return super.helper(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "argument-taking, unknown, missing-field, primitive/void-return, static, and no-superclass `super`-rooted chain hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner makeInner() { return new Inner(); } }
class Caller {
    int run(Group g) {
        var v = g.makeInner();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner makeInner() { return new Inner(); } }
class Caller {
    int run(Group g) {
        var v = g.makeInner();
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group {
    Inner makeInner() { return new Inner(); }
    Group inner() { return new Group(); }
}
class Base { Inner makeInner() { return new Inner(); } }
class Child extends Base {
    int thisInitializer() {
        var v = this.makeInner();
        return v.helper(1);
    }
    int superInitializer() {
        var v = super.makeInner();
        return v.helper(1);
    }
}
class Caller {
    Group holder = new Group();
    int constructorInitializer() {
        var v = new Group().makeInner();
        return v.helper(1);
    }
    int fieldInitializer() {
        var v = holder.makeInner();
        return v.helper(1);
    }
    int hopInitializer(Group g) {
        var v = g.inner().makeInner();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 5);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorInitializer",
            "com::example::Caller::fieldInitializer",
            "com::example::Caller::hopInitializer",
            "com::example::Child::superInitializer",
            "com::example::Child::thisInitializer"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 5);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorInitializer",
            "com::example::Caller::fieldInitializer",
            "com::example::Caller::hopInitializer",
            "com::example::Child::superInitializer",
            "com::example::Child::thisInitializer"
        ]
    );
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let inner_dir = dir.join("src").join("pkg").join("inner");
    let group_dir = dir.join("src").join("pkg").join("group");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let inner_path = inner_dir.join("Foo.java");
    let group_path = group_dir.join("Group.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&inner_dir).unwrap();
    fs::create_dir_all(&group_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &inner_path,
        "package pkg.inner;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &group_path,
        "package pkg.group;
import pkg.inner.Foo;
public class Group { public Foo makeFoo() { return new Foo(); } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.group.Group;
public class Bar {
    public int run(Group g) {
        var v = g.makeFoo();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let foo_helper_symbol = "pkg::inner::Foo::helper";
    let live = trace_symbol_graph(&dir, foo_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, foo_helper_symbol, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_qualified_initializer_receiver_calls_fail_closed_for_unsupported_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
class Group { Inner makeInner(int value) { return new Inner(); } }
class Util { static Inner make() { return new Inner(); } }
class Caller {
    Group group = new Group();
    int unboundReceiver() {
        var v = unknown.makeInner();
        return v.helper(1);
    }
    int factoryInferredHop() {
        var a = makeA();
        var b = a.make();
        return b.helper(1);
    }
    int arityMismatch() {
        var v = group.makeInner();
        return v.helper(1);
    }
    int unknownThisCallee() {
        var v = this.missing();
        return v.helper(1);
    }
    int unknownSuperCallee() {
        var v = super.missing();
        return v.helper(1);
    }
    Inner makeA() { return new Inner(); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "unbound, factory-inferred, arity-mismatched, and unknown `this`/`super` qualified initializer receivers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_method_hop_receiver_calls_through_interface_inheritance() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IFactory { Inner inner(); }
interface IGroup extends IFactory {}
class Caller {
    int run(IGroup group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_interface_defaults() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IGroup {
    default Inner inner() { return new Inner(); }
}
class Caller {
    int run(IGroup group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_inherited_interface_defaults() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IFactory {
    default Inner inner() { return new Inner(); }
}
interface IGroup extends IFactory {}
class Caller {
    int run(IGroup group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_method_hop_receiver_calls_through_class_receiver_interface_defaults() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IGroup {
    default Inner inner() { return new Inner(); }
}
class Group implements IGroup {}
class Caller {
    int run(Group group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_method_hop_receiver_calls_fail_closed_for_ambiguous_or_static_interface_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Inner { int helper(int value) { return value; } }
interface IGroupA { Inner inner(); }
interface IGroupB { Inner inner(); }
interface IGroup extends IGroupA, IGroupB {}
interface IStatic {
    static Inner inner() { return new Inner(); }
}
class Caller {
    int ambiguousHop(IGroup group) { return group.inner().helper(1); }
    int staticHop(IStatic group) { return group.inner().helper(1); }
}
",
    )
    .unwrap();

    let target = "com::example::Inner::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "branching and static interface method-call hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_receiver_calls_with_nested_type_imports_across_files() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("pkg").join("outer");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let outer_path = outer_dir.join("Outer.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package pkg.outer;
public class Outer {
    public static class Inner { public int helper(int value) { return value; } }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.outer.Outer.Inner;
public class Bar {
    public int run(Inner inner) { return inner.helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::outer::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_bare_calls_with_nested_static_member_imports_across_files() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("pkg").join("outer");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let outer_path = outer_dir.join("Outer.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package pkg.outer;
public class Outer {
    public static class Inner {
        public static int helper(int value) { return value; }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.outer.Outer.Inner.helper;
public class Bar {
    public int run() { return helper(1); }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::outer::Outer::Inner::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_receiver_calls_with_nested_type_imports_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("pkg").join("outer");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let outer_path = outer_dir.join("Outer.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package pkg.outer;
public class Outer {
    public static class Inner { public int helper(int value) { return value; } }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller; class Stale {}
",
    )
    .unwrap();
    let overlay = "package pkg.caller;
import pkg.outer.Outer.Inner;
public class Bar {
    public int run(Inner inner) { return inner.helper(1); }
}
";
    let helper_symbol = "pkg::outer::Outer::Inner::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn java_nested_type_imports_fail_closed_for_missing_nested_targets() {
    let dir = temporary_dir();
    let outer_dir = dir.join("src").join("pkg").join("outer");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let outer_path = outer_dir.join("Outer.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&outer_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &outer_path,
        "package pkg.outer;
public class Outer { public static class Other { public int helper(int value) { return value; } } }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.outer.Outer.Missing;
public class Bar {
    public int run(Missing inner) { return inner.helper(1); }
}
",
    )
    .unwrap();

    let target = "pkg::outer::Outer::Other::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "nested type imports naming a missing nested target must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper helper = new Helper();
    int thisField() {
        var v = this.helper;
        return v.helper(1);
    }
    int bareField() {
        var v = helper;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bareField",
            "com::example::Caller::thisField"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bareField",
            "com::example::Caller::thisField"
        ]
    );
}

#[test]
fn traces_java_var_static_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util { static Helper STATIC_HELPER = new Helper(); }
class Caller {
    int run() {
        var v = Util.STATIC_HELPER;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_factory_root_member_chain_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let entry_path = helper_dir.join("Entry.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo {
    public Entry entry = new Entry();
    public int helper(int value) { return value; }
}
",
    )
    .unwrap();
    fs::write(
        &entry_path,
        "package pkg.helper;
public class Entry { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util {
    public static Foo MakeHelper() { return new Foo(); }
    public static Foo MakeHelper(int value) { return new Foo(); }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.helper.Foo;
import static pkg.util.Util.MakeHelper;
public class Caller {
    public Foo makeFoo() { return new Foo(); }
    public int sameType() { return makeFoo().helper(1); }
    public int sameTypeChained() { return makeFoo().entry.helper(1); }
    public int imported() { return MakeHelper().helper(1); }
    public int importedChained() { return MakeHelper().entry.helper(1); }
    public int importedArity() { return MakeHelper(1).helper(1); }
}
",
    )
    .unwrap();

    // A bare factory-call root resolves through the same factory rules as a
    // `var` initializer (a unique same-type method or explicit static-method
    // import with matching arity) and dispatches the trailing member chain on
    // the factory's declared type, including intermediate field hops and
    // arity-matched overloads.
    for (target, expected) in [
        (
            "pkg::helper::Foo::helper",
            vec![
                "pkg::caller::Caller::imported",
                "pkg::caller::Caller::importedArity",
                "pkg::caller::Caller::sameType",
            ],
        ),
        (
            "pkg::helper::Entry::helper",
            vec![
                "pkg::caller::Caller::importedChained",
                "pkg::caller::Caller::sameTypeChained",
            ],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        let mut callers = live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} live");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        let mut callers = persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} persisted");
    }
}

#[test]
fn traces_java_factory_root_member_chain_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Foo { int helper(int value) { return value; } }
class Caller { Foo makeFoo() { return new Foo(); } int run() { return 0; } }
",
    )
    .unwrap();
    let overlay = "package com.example;
class Foo { int helper(int value) { return value; } }
class Caller {
    Foo makeFoo() { return new Foo(); }
    int run() { return makeFoo().helper(1); }
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        "com::example::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        "com::example::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_factory_root_member_chain_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Foo {
    Entry entry = new Entry();
    int helper(int value) { return value; }
}
class Caller {
    Foo makeFoo() { return new Foo(); }
    int methodAsValue() { return makeFoo.entry.helper(1); }
    int missingHop() { return makeFoo().missing.helper(1); }
    int arityMismatch() { return makeFoo(1).helper(1); }
    int unknownFactory() { return MissingHelper().helper(1); }
    int control() { return makeFoo().helper(1); }
}
",
    )
    .unwrap();

    // A factory root used as a field (without call parens), chains with
    // missing hops, arity-mismatched or unknown factories still fail closed
    // (the inner factory call itself may still be a legitimate callee), while
    // a resolvable same-type factory root keeps tracing the final member.
    for target in ["com::example::Foo::helper", "com::example::Entry::helper"] {
        for caller in [
            "com::example::Caller::methodAsValue",
            "com::example::Caller::missingHop",
            "com::example::Caller::arityMismatch",
            "com::example::Caller::unknownFactory",
        ] {
            let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
            assert!(
                !live.callers.iter().any(|symbol| symbol.symbol_id == caller),
                "{caller} should not call {target} live"
            );
            rebuild_symbol_index(&dir, &db_path).unwrap();
            let persisted =
                trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
            assert!(
                !persisted
                    .callers
                    .iter()
                    .any(|symbol| symbol.symbol_id == caller),
                "{caller} should not call {target} persisted"
            );
        }
    }
    let live =
        trace_symbol_graph(&dir, "com::example::Foo::helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::control");
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Caller::control"
    );
}

#[test]
fn traces_java_parenthesized_member_chain_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let entry_path = helper_dir.join("Entry.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo {
    public Entry entry = new Entry();
    public int helper(int value) { return value; }
    public Foo inner() { return this; }
}
",
    )
    .unwrap();
    fs::write(
        &entry_path,
        "package pkg.helper;
public class Entry { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util {
    public static Foo MakeHelper() { return new Foo(); }
    public static Foo MakeHelper(int value) { return new Foo(); }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.helper.Foo;
import pkg.util.Util;
import static pkg.util.Util.MakeHelper;
public class Caller {
    public Foo makeFoo() { return new Foo(); }
    public int parenBound(Foo group) { return (group).helper(1); }
    public int parenBoundHop(Foo group) { return (group).inner().entry.helper(1); }
    public int parenFactory() { return (makeFoo()).entry.helper(1); }
    public int parenFactoryDirect() { return (makeFoo()).helper(1); }
    public int parenImportedFactory() { return (MakeHelper()).entry.helper(1); }
    public int parenImportedFactoryDirect() { return (MakeHelper()).helper(1); }
    public int parenConstructed() { return (new Foo()).entry.helper(1); }
    public int parenThis() { return (this).makeFoo().entry.helper(1); }
    public int parenQualifiedFactory() { return (Util.MakeHelper()).entry.helper(1); }
}
",
    )
    .unwrap();

    // A parenthesized receiver in a member chain such as
    // `(group).helper(1)`, `(group).inner().entry.helper(1)`,
    // `(makeFoo()).entry.helper(1)`, `(MakeHelper()).entry.helper(1)`,
    // `(new Foo()).entry.helper(1)`, `(this).makeFoo().entry.helper(1)`, or
    // `(Util.MakeHelper()).entry.helper(1)` unwraps the parentheses and keeps
    // the same chain spelling as the unparenthesized form, so bound,
    // same-type factory, static-imported factory, constructor-, `this`-,
    // and type-qualified factory roots all dispatch the final member on the
    // canonical declared type.
    for (target, expected) in [
        (
            "pkg::helper::Foo::helper",
            vec![
                "pkg::caller::Caller::parenBound",
                "pkg::caller::Caller::parenFactoryDirect",
                "pkg::caller::Caller::parenImportedFactoryDirect",
            ],
        ),
        (
            "pkg::helper::Entry::helper",
            vec![
                "pkg::caller::Caller::parenBoundHop",
                "pkg::caller::Caller::parenConstructed",
                "pkg::caller::Caller::parenFactory",
                "pkg::caller::Caller::parenImportedFactory",
                "pkg::caller::Caller::parenQualifiedFactory",
                "pkg::caller::Caller::parenThis",
            ],
        ),
        (
            "pkg::helper::Foo::inner",
            vec!["pkg::caller::Caller::parenBoundHop"],
        ),
        (
            "pkg::caller::Caller::makeFoo",
            vec![
                "pkg::caller::Caller::parenFactory",
                "pkg::caller::Caller::parenFactoryDirect",
                "pkg::caller::Caller::parenThis",
            ],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        let mut callers = live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} live");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        let mut callers = persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} persisted");
    }
}

#[test]
fn traces_java_parenthesized_member_chain_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Foo { int helper(int value) { return value; } }
class Entry { int helper(int value) { return value; } }
class Caller { Foo makeFoo() { return new Foo(); } int run() { return 0; } }
",
    )
    .unwrap();
    let overlay = "package com.example;
class Foo { int helper(int value) { return value; } }
class Entry { int helper(int value) { return value; } }
class Caller {
    Foo makeFoo() { return new Foo(); }
    int run() { return (makeFoo()).helper(1); }
    int runConstructed() { return (new Entry()).helper(1); }
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        "com::example::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        "com::example::Entry::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(
        live.callers[0].symbol_id,
        "com::example::Caller::runConstructed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        "com::example::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");

    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        "com::example::Entry::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Caller::runConstructed"
    );
}

#[test]
fn java_parenthesized_member_chain_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Foo {
    Entry entry = new Entry();
    int helper(int value) { return value; }
}
class Caller {
    Foo makeFoo() { return new Foo(); }
    int makeCount() { return 1; }
    int missingHop() { return (makeFoo()).missing.helper(1); }
    int arityMismatch() { return (makeFoo(1)).entry.helper(1); }
    int unknownFactory() { return (MissingHelper()).helper(1); }
    int primitiveRoot() { return (makeCount()).entry.helper(1); }
    int unboundRoot() { return (group).entry.helper(1); }
    int control() { return (makeFoo()).entry.helper(1); }
}
",
    )
    .unwrap();

    // A parenthesized chain root fails closed exactly like its
    // unparenthesized form: missing hops, arity-mismatched or unknown
    // factories, primitive return types, and unbound receivers never dispatch
    // a chain member, while the resolvable same-type factory root keeps
    // tracing the final member.
    for target in ["com::example::Foo::helper", "com::example::Entry::helper"] {
        for caller in [
            "com::example::Caller::missingHop",
            "com::example::Caller::arityMismatch",
            "com::example::Caller::unknownFactory",
            "com::example::Caller::primitiveRoot",
            "com::example::Caller::unboundRoot",
        ] {
            let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
            assert!(
                !live.callers.iter().any(|symbol| symbol.symbol_id == caller),
                "{caller} should not call {target} live"
            );
            rebuild_symbol_index(&dir, &db_path).unwrap();
            let persisted =
                trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
            assert!(
                !persisted
                    .callers
                    .iter()
                    .any(|symbol| symbol.symbol_id == caller),
                "{caller} should not call {target} persisted"
            );
        }
    }
    let live =
        trace_symbol_graph(&dir, "com::example::Entry::helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::control");
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index(
        &db_path,
        "com::example::Entry::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Caller::control"
    );
}

#[test]
fn traces_java_parenthesized_var_initializer_receiver_calls_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper makeHelper() { return new Helper(); }
    int run() {
        var constructed = (new Helper());
        var factory = (makeHelper());
        return constructed.helper(1) + factory.helper(2);
    }
}
",
    )
    .unwrap();

    // A parenthesized `var` initializer such as `(new Helper())` or
    // `(makeHelper())` unwraps to the same receiver binding as the
    // unparenthesized form, so the local dispatches the final member on the
    // constructed or factory-returned declared type.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_parenthesized_var_initializer_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper makeHelper() { return new Helper(); }
    int run() {
        var constructed = (new Helper());
        return constructed.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_parenthesized_var_initializer_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper makeHelper() { return new Helper(); }
    int makeCount() { return 1; }
    int run() {
        var arity = (makeHelper(1));
        var primitive = (makeCount());
        var array = (new int[3]);
        var unknown = (MissingHelper());
        return arity.helper(1) + primitive.helper(1) + array.helper(1) + unknown.helper(1);
    }
    int control() {
        var ok = (makeHelper());
        return ok.helper(1);
    }
}
",
    )
    .unwrap();

    // A parenthesized `var` initializer fails closed exactly like its
    // unparenthesized form: arity-mismatched factories, primitive return
    // types, array creations, and unknown factories never bind a usable
    // receiver type, while the resolvable factory initializer keeps tracing.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::control");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Caller::control"
    );
}

#[test]
fn traces_java_array_access_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    Helper item = new Helper();
    int helper(int value) { return value; }
    Group inner() { return this; }
}
class Caller {
    private Helper[] fieldItems = new Helper[2];
    int run(Helper[] items, Group[] groups) {
        Helper[] local = new Helper[3];
        return items[0].helper(1)
            + local[1].helper(2)
            + fieldItems[0].helper(3)
            + this.fieldItems[1].helper(4)
            + groups[0].item.helper(5)
            + groups[0].inner().helper(6);
    }
}
",
    )
    .unwrap();

    // Element-access receivers on array-typed parameters, locals, and fields
    // dispatch on the element component type; `this.`-rooted element chains
    // and member chains after an element hop resolve the same way.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");

    let group_helper_symbol = "com::example::Group::helper";
    let group_live =
        trace_symbol_graph(&dir, group_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(group_live.callers.len(), 1);
    assert_eq!(group_live.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_array_access_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    private Helper[] fieldItems = new Helper[2];
    int run(Helper[] items) {
        Helper[] local = new Helper[3];
        return items[0].helper(1) + local[1].helper(2) + this.fieldItems[0].helper(3);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_array_access_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper[] makeItems() { return new Helper[2]; }
    int[] makeCounts() { return new int[2]; }
    Helper[][] makeMatrix() { return new Helper[2][2]; }
    int run(Helper[] items, int[] counts, Helper[][] matrix) {
        var countsVar = makeCounts();
        var matrixVar = makeMatrix();
        var unknownVar = unknownFactory();
        var qualifiedUnknownVar = Util.unknownFactory();
        var qualifiedCountsVar = Util.makeCounts();
        return items.helper(1)
            + counts[0].helper(2)
            + matrix[0][0].helper(3)
            + makeCounts()[0].helper(4)
            + makeMatrix()[0].helper(5)
            + makeItems()[0][0].helper(6)
            + countsVar[0].helper(7)
            + matrixVar[0].helper(8)
            + unknownVar[0].helper(9)
            + qualifiedUnknownVar[0].helper(10)
            + qualifiedCountsVar[0].helper(11);
    }
    int control() {
        Helper[] items = new Helper[2];
        return items[0].helper(1);
    }
}
class Util {
    static int[] makeCounts() { return new int[2]; }
}
",
    )
    .unwrap();

    // A direct member call on an array, a primitive-component array, a
    // multi-dimensional array, a primitive- or multi-dimensional-returning
    // factory array, multi-dimensional element access on a factory-returned
    // array, and `var` locals bound from primitive-, multi-dimensional-,
    // unknown-, or non-array-returning qualified factories all fail closed;
    // only the resolvable element-access receiver in `control` traces.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::control");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Caller::control"
    );
}

#[test]
fn traces_java_factory_returned_array_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    int helper(int value) { return value; }
    Group inner() { return this; }
}
class Caller {
    Helper[] makeItems() { return new Helper[2]; }
    Group[] makeGroups() { return new Group[1]; }
    int run() {
        return makeItems()[0].helper(1)
            + makeGroups()[0].inner().helper(2);
    }
}
",
    )
    .unwrap();

    // A factory-returned array receiver such as `makeItems()[0].helper(...)`
    // dispatches on the array's element component type, with member chains
    // after the element hop resolving through the same chain rules.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");

    let group_helper_symbol = "com::example::Group::helper";
    let group_live =
        trace_symbol_graph(&dir, group_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(group_live.callers.len(), 1);
    assert_eq!(group_live.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_factory_returned_array_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper[] makeItems() { return new Helper[2]; }
    int run() {
        return makeItems()[0].helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_static_imported_array_factory_receiver_calls_in_live_workspace_and_persisted_index()
{
    let dir = temporary_dir();
    let example_dir = dir.join("src").join("com").join("example");
    let factories_path = example_dir.join("Factories.java");
    let source_path = example_dir.join("Caller.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&example_dir).unwrap();
    fs::write(
        &factories_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
public class Factories {
    public static Helper[] makeItems() { return new Helper[2]; }
}
",
    )
    .unwrap();
    fs::write(
        &source_path,
        "package com.example;
import static com.example.Factories.makeItems;
class Caller {
    int run() {
        return makeItems()[0].helper(1);
    }
}
",
    )
    .unwrap();

    // A static-imported array factory root dispatches on the element
    // component type through the same factory rules as a `var` initializer.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_element_access_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    private Helper[] fieldItems = new Helper[2];
    int run(Helper[] items) {
        Helper[] local = new Helper[3];
        var first = items[0];
        var second = local[1];
        var third = fieldItems[0];
        return first.helper(1)
            + second.helper(2)
            + third.helper(3);
    }
}
",
    )
    .unwrap();

    // A `var` local bound from an element access such as `var first = items[0]`
    // dispatches on the base array's element component type for parameters,
    // locals, and enclosing-class fields.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_element_access_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    int run(Helper[] items) {
        var first = items[0];
        return first.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_qualified_element_access_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Holder { Helper[] fieldItems = new Helper[1]; }
class Group { Holder holder = new Holder(); Helper[] fieldItems = new Helper[1]; }
class Base { Helper[] inheritedItems = new Helper[1]; }
class Util { static Helper[] fieldItems = new Helper[1]; }
class Caller extends Base {
    private Helper[] fieldItems = new Helper[2];
    int run(Group group) {
        var first = this.fieldItems[0];
        var second = group.fieldItems[0];
        var third = group.holder.fieldItems[0];
        var sixth = super.inheritedItems[0];
        var seventh = Util.fieldItems[0];
        return first.helper(1)
            + second.helper(2)
            + third.helper(3)
            + sixth.helper(4)
            + seventh.helper(5);
    }
}
",
    )
    .unwrap();

    // A `var` local bound from an element access with a qualified base such as
    // `var first = this.fieldItems[0]`, `var second = group.fieldItems[0]`,
    // a multi-hop field chain `var third = group.holder.fieldItems[0]`, a
    // `super`-rooted field `var sixth = super.inheritedItems[0]`, or a static
    // type field `var seventh = Util.fieldItems[0]` dispatches on the terminal
    // array field's element component type.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_qualified_element_access_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Util { static Helper[] fieldItems = new Helper[2]; }
class Caller {
    int run() {
        var first = Util.fieldItems[0];
        return first.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_var_qualified_element_access_receiver_calls_fail_closed_for_unsupported_bases() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util {
    static Helper[] fieldItems = new Helper[1];
    Helper[] instanceItems = new Helper[1];
}
class Caller {
    private Helper[] fieldItems;
    Helper[] makeItems() { return new Helper[2]; }
    int[] makeCounts() { return new int[2]; }
    Helper[][] makeMatrix() { return new Helper[2][2]; }
    int run(int[] counts, Helper[][] matrix) {
        var superBase = super.fieldItems[0];
        var unbound = counts[0];
        var matrixAccess = matrix[0][0];
        var unknownFactory = makeUnknown()[0];
        var primitiveFactory = makeCounts()[0];
        var multiFactory = makeMatrix()[0];
        var arityFactory = makeItems(1)[0];
        var staticInstanceField = Util.instanceItems[0];
        return superBase.helper(1)
            + unbound.helper(2)
            + matrixAccess.helper(3)
            + unknownFactory.helper(4)
            + primitiveFactory.helper(5)
            + multiFactory.helper(6)
            + arityFactory.helper(7)
            + staticInstanceField.helper(8);
    }
    int control() {
        var ok = this.fieldItems[0];
        return ok.helper(1);
    }
}
",
    )
    .unwrap();

    // `var` locals bound from element accesses with an unresolvable `super`
    // base, primitive-array or multi-dimensional bases, unknown/primitive-/
    // multi-dimensional-returning/arity-mismatched factory-call bases, and
    // non-static fields on a static type receiver all fail closed; only the
    // `this`-rooted element access in `control` traces.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::control");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(
        persisted.callers[0].symbol_id,
        "com::example::Caller::control"
    );
}

#[test]
fn traces_java_var_factory_call_element_access_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    int helper(int value) { return value; }
    Group inner() { return this; }
}
class Util {
    static Helper[] makeItems() { return new Helper[2]; }
    static Group[] makeGroups() { return new Group[1]; }
}
class Caller {
    Helper[] makeItems() { return new Helper[2]; }
    int run() {
        var first = makeItems()[0];
        var second = Util.makeItems()[0];
        var third = Util.makeGroups()[0];
        return first.helper(1)
            + second.helper(2)
            + third.helper(3);
    }
}
",
    )
    .unwrap();

    // A `var` local bound from an element access on a factory call such as
    // `var first = makeItems()[0]` or `var second = Util.makeItems()[0]`
    // resolves the factory through the same rules as other `var` initializers
    // and dispatches on the array's element component type.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");

    let group_helper_symbol = "com::example::Group::helper";
    let group_live =
        trace_symbol_graph(&dir, group_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(group_live.callers.len(), 1);
    assert_eq!(group_live.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_factory_call_element_access_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper[] makeItems() { return new Helper[2]; }
    int run() {
        var first = makeItems()[0];
        return first.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_factory_returned_array_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    int helper(int value) { return value; }
    Group inner() { return this; }
}
class Caller {
    Helper[] makeItems() { return new Helper[2]; }
    Group[] makeGroups() { return new Group[1]; }
    int run() {
        var items = makeItems();
        var groups = makeGroups();
        return items[0].helper(1)
            + groups[0].inner().helper(2);
    }
}
",
    )
    .unwrap();

    // A `var` local initialized from a bare factory call whose declared return
    // type is a single-level array dispatches an element access on the array's
    // element component type, with member chains after the element hop
    // resolving through the same chain rules.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");

    let group_helper_symbol = "com::example::Group::helper";
    let group_live =
        trace_symbol_graph(&dir, group_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(group_live.callers.len(), 1);
    assert_eq!(group_live.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_factory_returned_array_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper[] makeItems() { return new Helper[2]; }
    int run() {
        var items = makeItems();
        return items[0].helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_qualified_factory_returned_array_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Group {
    int helper(int value) { return value; }
    Group inner() { return this; }
}
class Util {
    static Helper[] makeItems() { return new Helper[2]; }
    static Group[] makeGroups() { return new Group[1]; }
}
class Caller {
    Helper[] makeLocal() { return new Helper[2]; }
    int run(Group group) {
        var items = Util.makeItems();
        var groups = Util.makeGroups();
        var local = this.makeLocal();
        var bound = group.makeGroups();
        return items[0].helper(1)
            + groups[0].inner().helper(2)
            + local[0].helper(3)
            + bound[0].helper(4);
    }
}
",
    )
    .unwrap();

    // A `var` local initialized from a qualified factory call whose declared
    // return type is a single-level array dispatches an element access on the
    // array's element component type through the qualified initializer rules:
    // static type receivers (`Util.makeItems`), `this`-rooted callees
    // (`this.makeLocal`), and bound-receiver chains (`group.makeGroups`),
    // with member chains after the element hop resolving through the same
    // chain rules.
    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");

    let group_helper_symbol = "com::example::Group::helper";
    let group_live =
        trace_symbol_graph(&dir, group_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(group_live.callers.len(), 1);
    assert_eq!(group_live.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_qualified_factory_returned_array_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Util {
    static Helper[] makeItems() { return new Helper[2]; }
}
class Caller {
    int run() {
        var items = Util.makeItems();
        return items[0].helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_generic_static_root_member_chain_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Box<T> {
    Entry entry = new Entry();
    int helper(int value) { return value; }
}
class Util {
    static Box<String> STATIC_HELPER = new Box<String>();
    static Box<String> MakeBox() { return new Box<String>(); }
}
class Caller {
    int run() { return Util.STATIC_HELPER.helper(1); }
    int chained() { return Util.STATIC_HELPER.entry.helper(1); }
    int factory() { return Util.MakeBox().helper(1); }
}
",
    )
    .unwrap();

    // A type-qualified static root whose declared or factory-return type is
    // generic normalizes to the raw base type, so the trailing member chain
    // dispatches on the canonical raw class (direct final call, intermediate
    // field hops, or a static factory-call root).
    for (target, expected) in [
        (
            "com::example::Box::helper",
            vec!["com::example::Caller::factory", "com::example::Caller::run"],
        ),
        (
            "com::example::Entry::helper",
            vec!["com::example::Caller::chained"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        let mut callers = live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} live");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        let mut callers = persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} persisted");
    }
}

#[test]
fn traces_java_generic_static_root_member_chain_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Util { static Box<String> STATIC_HELPER = new Box<String>(); }
class Caller { int run() { return 0; } }
",
    )
    .unwrap();
    let overlay = "package com.example;
class Box<T> { int helper(int value) { return value; } }
class Util { static Box<String> STATIC_HELPER = new Box<String>(); }
class Caller {
    int run() { return Util.STATIC_HELPER.helper(1); }
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        "com::example::Box::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        "com::example::Box::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_generic_static_root_member_chain_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Box<T> {
    Entry entry = new Entry();
    int helper(int value) { return value; }
}
class Util {
    static Box<String> STATIC_HELPER = new Box<String>();
    static Box<String> MakeBox() { return new Box<String>(); }
}
class Caller {
    int missingField() { return Util.MISSING.helper(1); }
    int methodAsValue() { return Util.MakeBox.entry.helper(1); }
    int missingHop() { return Util.STATIC_HELPER.missing.helper(1); }
    int genericTypePrefix() { return Box<Integer>.STATIC_HELPER.helper(1); }
    int control() { return Util.STATIC_HELPER.helper(1); }
}
",
    )
    .unwrap();

    // A type-qualified generic-static root that is not a declared static
    // field (a missing member or a method used as a value), chains with
    // missing hops, and type-argument-prefix spellings that the Java grammar
    // does not parse as method calls (and therefore produce no fact) still
    // fail closed, while a resolvable generic static field root keeps tracing.
    for (caller, expected) in [
        ("com::example::Caller::missingField", Vec::<&str>::new()),
        ("com::example::Caller::methodAsValue", Vec::<&str>::new()),
        ("com::example::Caller::missingHop", Vec::<&str>::new()),
        (
            "com::example::Caller::genericTypePrefix",
            Vec::<&str>::new(),
        ),
        (
            "com::example::Caller::control",
            vec!["com::example::Box::helper"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        let mut callees = live
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callees.sort();
        assert_eq!(callees, expected, "{caller} live");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        let mut callees = persisted
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callees.sort();
        assert_eq!(callees, expected, "{caller} persisted");
    }
}

#[test]
fn traces_java_direct_type_qualified_static_root_member_chain_calls_across_files() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let entry_path = helper_dir.join("Entry.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo {
    public Entry entry = new Entry();
    public int helper(int value) { return value; }
}
",
    )
    .unwrap();
    fs::write(
        &entry_path,
        "package pkg.helper;
public class Entry { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util {
    public static Foo STATIC_HELPER = new Foo();
    public static Foo MakeHelper(int value) { return new Foo(); }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.util.Util;
public class Caller {
    public int run() { return Util.STATIC_HELPER.helper(1); }
    public int chained() { return Util.STATIC_HELPER.entry.helper(1); }
    public int factory() { return Util.MakeHelper(1).helper(1); }
}
",
    )
    .unwrap();

    // A type-qualified static root pins the root's declared type and
    // dispatches the trailing member chain (direct final call, intermediate
    // field hops, or a static factory-call root) on that canonical type, so a
    // caller in another package dispatches the final member independently of
    // its own package.
    for (target, expected) in [
        (
            "pkg::helper::Foo::helper",
            vec!["pkg::caller::Caller::factory", "pkg::caller::Caller::run"],
        ),
        (
            "pkg::helper::Entry::helper",
            vec!["pkg::caller::Caller::chained"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        let mut callers = live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} live");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        let mut callers = persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} persisted");
    }
}

#[test]
fn traces_java_direct_type_qualified_static_root_member_chain_calls_in_same_package() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("pkg");
    let entry_path = source_dir.join("Entry.java");
    let base_path = source_dir.join("Base.java");
    let helper_path = source_dir.join("Foo.java");
    let util_path = source_dir.join("Util.java");
    let caller_path = source_dir.join("Caller.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &entry_path,
        "package pkg;
public class Entry { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &base_path,
        "package pkg;
public class Base { public static Foo INHERITED_STATIC = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package pkg;
public class Foo {
    public Entry entry = new Entry();
    public int helper(int value) { return value; }
}
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg;
public class Util extends Base {
    public static Foo STATIC_HELPER = new Foo();
    public static Foo MakeHelper() { return new Foo(); }
    public static class Nested { public static Foo INSTANCE = new Foo(); }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg;
public class Caller {
    public int run() { return Util.STATIC_HELPER.helper(1); }
    public int chained() { return Util.STATIC_HELPER.entry.helper(1); }
    public int factory() { return Util.MakeHelper().helper(1); }
    public int nested() { return Util.Nested.INSTANCE.helper(1); }
    public int inherited() { return Util.INHERITED_STATIC.helper(1); }
}
",
    )
    .unwrap();

    // Same-package type prefixes, nested-type prefixes, and static fields
    // inherited from a direct superclass all pin the root's declared type and
    // dispatch the trailing member chain on that canonical type.
    for (target, expected) in [
        (
            "pkg::Foo::helper",
            vec![
                "pkg::Caller::factory",
                "pkg::Caller::inherited",
                "pkg::Caller::nested",
                "pkg::Caller::run",
            ],
        ),
        ("pkg::Entry::helper", vec!["pkg::Caller::chained"]),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        let mut callers = live
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} live");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        let mut callers = persisted
            .callers
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callers.sort();
        assert_eq!(callers, expected, "{target} persisted");
    }
}

#[test]
fn traces_java_direct_type_qualified_static_root_member_chain_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util { public static Foo STATIC_HELPER = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
public class Caller { public int run() { return 0; } }
",
    )
    .unwrap();
    let overlay = "package pkg.caller;
import pkg.util.Util;
public class Caller {
    public int run() { return Util.STATIC_HELPER.helper(1); }
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "pkg::helper::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "pkg::helper::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Caller::run");
}

#[test]
fn java_direct_type_qualified_static_root_member_chain_calls_fail_closed_for_unsupported_references()
 {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("pkg");
    let entry_path = source_dir.join("Entry.java");
    let helper_path = source_dir.join("Foo.java");
    let util_path = source_dir.join("Util.java");
    let caller_path = source_dir.join("Caller.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &entry_path,
        "package pkg;
public class Entry { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package pkg;
public class Foo {
    public Entry entry = new Entry();
    public int helper(int value) { return value; }
}
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg;
public class Util {
    public static Foo STATIC_HELPER = new Foo();
    public static Foo MakeHelper() { return new Foo(); }
    public Foo nonStatic = new Foo();
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg;
public class Caller {
    public int missingField() { return Util.MISSING.helper(1); }
    public int methodAsValue() { return Util.MakeHelper.entry.helper(1); }
    public int nonStaticRoot() { return Util.nonStatic.helper(1); }
    public int missingHop() { return Util.STATIC_HELPER.missing.helper(1); }
    public int arityMismatch() { return Util.STATIC_HELPER.helper(); }
    public int control() { return Util.STATIC_HELPER.helper(1); }
}
",
    )
    .unwrap();

    // A type-qualified root that is not a declared static field (a missing
    // member or a method used as a value), a non-static root, chains with
    // missing hops, and arity-mismatched final calls still fail closed, while
    // a resolvable static field root keeps tracing.
    for (caller, expected) in [
        ("pkg::Caller::missingField", Vec::<&str>::new()),
        ("pkg::Caller::methodAsValue", Vec::<&str>::new()),
        ("pkg::Caller::nonStaticRoot", Vec::<&str>::new()),
        ("pkg::Caller::missingHop", Vec::<&str>::new()),
        ("pkg::Caller::arityMismatch", Vec::<&str>::new()),
        ("pkg::Caller::control", vec!["pkg::Foo::helper"]),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        let mut callees = live
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callees.sort();
        assert_eq!(callees, expected, "{caller} live");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        let mut callees = persisted
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callees.sort();
        assert_eq!(callees, expected, "{caller} persisted");
    }
}

#[test]
fn traces_java_direct_static_imported_field_member_chain_calls_across_files() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let entry_path = helper_dir.join("Entry.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo {
    public Entry entry = new Entry();
    public int helper(int value) { return value; }
}
",
    )
    .unwrap();
    fs::write(
        &entry_path,
        "package pkg.helper;
public class Entry { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util { public static Foo STATIC_HELPER = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.util.Util.STATIC_HELPER;
public class Bar {
    public int run() { return STATIC_HELPER.helper(1); }
    public int chained() { return STATIC_HELPER.entry.helper(1); }
}
",
    )
    .unwrap();

    // A statically imported field root pins the field's declared type and
    // dispatches the trailing member chain (direct final call or intermediate
    // field hops) on that canonical type, so a caller in another package
    // dispatches the final member independently of its own package.
    for (target, expected) in [
        ("pkg::helper::Foo::helper", vec!["pkg::caller::Bar::run"]),
        (
            "pkg::helper::Entry::helper",
            vec!["pkg::caller::Bar::chained"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            live.callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{target} live"
        );
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert_eq!(
            persisted
                .callers
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            expected,
            "{target} persisted"
        );
    }
}

#[test]
fn traces_java_direct_static_imported_field_member_chain_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util { public static Foo STATIC_HELPER = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
public class Bar { public int run() { return 0; } }
",
    )
    .unwrap();
    let overlay = "package pkg.caller;
import static pkg.util.Util.STATIC_HELPER;
public class Bar {
    public int run() { return STATIC_HELPER.helper(1); }
}
";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "pkg::helper::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "pkg::helper::Foo::helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn java_direct_static_imported_field_member_chain_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo {
    public Entry entry = new Entry();
    public int helper(int value) { return value; }
}
",
    )
    .unwrap();
    fs::write(
        helper_dir.join("Entry.java"),
        "package pkg.helper;
public class Entry { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util {
    public static Foo STATIC_HELPER = new Foo();
    public static Foo MakeHelper() { return new Foo(); }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.util.Util.STATIC_HELPER;
import static pkg.util.Util.MakeHelper;
import static pkg.util.Util.MISSING;
public class Bar {
    public int missingField() { return MISSING.helper(1); }
    public int importedMethodAsValue() { return MakeHelper.entry.helper(1); }
    public int missingHop() { return STATIC_HELPER.missing.helper(1); }
    public int control() { return STATIC_HELPER.entry.helper(1); }
}
",
    )
    .unwrap();

    // A leading static-imported segment that is not a declared static field
    // (a missing member or a method used as a value) and chains with missing
    // hops still fail closed, while a resolvable static-imported field chain
    // keeps tracing.
    for (caller, expected) in [
        ("pkg::caller::Bar::missingField", Vec::<&str>::new()),
        (
            "pkg::caller::Bar::importedMethodAsValue",
            Vec::<&str>::new(),
        ),
        ("pkg::caller::Bar::missingHop", Vec::<&str>::new()),
        (
            "pkg::caller::Bar::control",
            vec!["pkg::helper::Entry::helper"],
        ),
    ] {
        let live = trace_symbol_graph(&dir, caller, TraceDirection::Callees).unwrap();
        let mut callees = live
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callees.sort();
        assert_eq!(callees, expected, "{caller} live");
        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, caller, TraceDirection::Callees).unwrap();
        let mut callees = persisted
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>();
        callees.sort();
        assert_eq!(callees, expected, "{caller} persisted");
    }
}

#[test]
fn traces_java_var_static_imported_field_receiver_calls_across_files() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
public class Util { public static Foo STATIC_HELPER = new Foo(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.util.Util.STATIC_HELPER;
public class Bar {
    public int run() {
        var v = STATIC_HELPER;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let foo_helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, foo_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, foo_helper_symbol, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_var_field_receiver_calls_through_shared_paths() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Holder { Entry entry = new Entry(); }
class Base { Holder holder = new Holder(); }
class Child extends Base {
    int thisChain() {
        var v = this.holder.entry;
        return v.helper(1);
    }
    int superField() {
        var v = super.holder.entry;
        return v.helper(1);
    }
    int bareChain() {
        var v = holder.entry;
        return v.helper(1);
    }
    int bareField() {
        var v = holder;
        return v.entry.helper(1);
    }
}
class Util {
    static Holder REGISTRY = new Holder();
    static class Inner { static Entry STATIC_ENTRY = new Entry(); }
}
class Caller {
    int staticChain() {
        var v = Util.REGISTRY.entry;
        return v.helper(1);
    }
    int nestedStatic() {
        var v = Util.Inner.STATIC_ENTRY;
        return v.helper(1);
    }
    int typedLocal() {
        Holder local = new Holder();
        var v = local;
        return v.entry.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 7);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::nestedStatic",
            "com::example::Caller::staticChain",
            "com::example::Caller::typedLocal",
            "com::example::Child::bareChain",
            "com::example::Child::bareField",
            "com::example::Child::superField",
            "com::example::Child::thisChain"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 7);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::nestedStatic",
            "com::example::Caller::staticChain",
            "com::example::Caller::typedLocal",
            "com::example::Child::bareChain",
            "com::example::Child::bareField",
            "com::example::Child::superField",
            "com::example::Child::thisChain"
        ]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Caller {
    Helper helper = new Helper();
    int run() {
        var v = this.helper;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn java_var_field_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util { Helper INSTANCE_HELPER = new Helper(); }
class Caller {
    Helper helper = new Helper();
    int factoryInferredCopy() {
        var local = makeHelper();
        var v = local;
        return v.helper(1);
    }
    int nonStaticTypeField() {
        var v = Util.INSTANCE_HELPER;
        return v.helper(1);
    }
    int unknownField() {
        var v = missing;
        return v.helper(1);
    }
    int unknownTypeField() {
        var v = Missing.STATIC;
        return v.helper(1);
    }
    int unknownThisField() {
        var v = this.missing;
        return v.helper(1);
    }
    int unknownSuperField() {
        var v = super.missing;
        return v.helper(1);
    }
    int boundReceiverField() {
        var v = helper.other;
        return v.helper(1);
    }
    Helper makeHelper() { return new Helper(); }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "factory-inferred copies, non-static type fields, unknown fields, unknown types, unknown `this`/`super` fields, and bound-receiver field chains with unknown fields must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_field_receiver_calls_through_method_hops() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
}
class Holder {
    Group group = new Group();
    Group inner() { return group; }
}
class Base { Holder holder = new Holder(); }
class Child extends Base {
    Group inner() { return holder.group; }
    int thisHop() {
        var v = this.holder.inner().entry;
        return v.helper(1);
    }
    int thisDirectHop() {
        var v = this.inner().entry;
        return v.helper(1);
    }
    int superHop() {
        var v = super.holder.inner().entry;
        return v.helper(1);
    }
    int bareHop() {
        var v = holder.inner().entry;
        return v.helper(1);
    }
    int bareFieldHop() {
        var v = holder.group.inner().entry;
        return v.helper(1);
    }
}
class Util { static Holder REGISTRY = new Holder(); }
class Caller {
    int staticHop() {
        var v = Util.REGISTRY.inner().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 6);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::staticHop",
            "com::example::Child::bareFieldHop",
            "com::example::Child::bareHop",
            "com::example::Child::superHop",
            "com::example::Child::thisDirectHop",
            "com::example::Child::thisHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 6);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::staticHop",
            "com::example::Child::bareFieldHop",
            "com::example::Child::bareHop",
            "com::example::Child::superHop",
            "com::example::Child::thisDirectHop",
            "com::example::Child::thisHop"
        ]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_through_method_hops_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); Group inner() { return this; } }
class Base { Group group = new Group(); }
class Caller extends Base {
    int run() {
        var v = this.group.inner().entry;
        return v.helper(1);
    }
    int runBare() {
        var v = group.inner().entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        ["com::example::Caller::run", "com::example::Caller::runBare"]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        ["com::example::Caller::run", "com::example::Caller::runBare"]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_through_bound_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
}
class Holder {
    Group group = new Group();
    Group inner() { return group; }
}
class Caller {
    Holder holder = new Holder();
    int fieldChain() {
        var v = holder.group.entry;
        return v.helper(1);
    }
    int fieldHop() {
        var v = holder.inner().entry;
        return v.helper(1);
    }
    int paramChain(Holder local) {
        var v = local.group.entry;
        return v.helper(1);
    }
    int paramHop(Holder local) {
        var v = local.inner().entry;
        return v.helper(1);
    }
    int declaredLocal() {
        Holder local = new Holder();
        var v = local.group.entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 5);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::declaredLocal",
            "com::example::Caller::fieldChain",
            "com::example::Caller::fieldHop",
            "com::example::Caller::paramChain",
            "com::example::Caller::paramHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 5);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::declaredLocal",
            "com::example::Caller::fieldChain",
            "com::example::Caller::fieldHop",
            "com::example::Caller::paramChain",
            "com::example::Caller::paramHop"
        ]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_through_bound_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); Group inner() { return this; } }
class Caller {
    Group group = new Group();
    int fieldChain() {
        var v = group.entry;
        return v.helper(1);
    }
    int paramChain(Group g) {
        var v = g.entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldChain",
            "com::example::Caller::paramChain"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::fieldChain",
            "com::example::Caller::paramChain"
        ]
    );
}

#[test]
fn traces_java_var_static_imported_field_chain_receiver_calls_across_files() {
    let dir = temporary_dir();
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let util_dir = dir.join("src").join("pkg").join("util");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_path = helper_dir.join("Foo.java");
    let util_path = util_dir.join("Util.java");
    let caller_path = caller_dir.join("Bar.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&helper_dir).unwrap();
    fs::create_dir_all(&util_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Foo { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &util_path,
        "package pkg.util;
import pkg.helper.Foo;
class Holder { public Foo foo = new Foo(); }
public class Util { public static Holder HOLDER = new Holder(); }
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.util.Util.HOLDER;
public class Bar {
    public int run() {
        var v = HOLDER.foo;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let foo_helper_symbol = "pkg::helper::Foo::helper";
    let live = trace_symbol_graph(&dir, foo_helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "pkg::caller::Bar::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, foo_helper_symbol, TraceDirection::Callers)
            .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "pkg::caller::Bar::run");
}

#[test]
fn traces_java_var_field_receiver_calls_through_constructor_roots() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
}
class Holder {
    Group group = new Group();
    Group inner() { return group; }
}
class Caller {
    int constructorChain() {
        var v = new Holder().group.entry;
        return v.helper(1);
    }
    int constructorHop() {
        var v = new Holder().inner().entry;
        return v.helper(1);
    }
    int constructorDirect() {
        var v = new Group().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorChain",
            "com::example::Caller::constructorDirect",
            "com::example::Caller::constructorHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::constructorChain",
            "com::example::Caller::constructorDirect",
            "com::example::Caller::constructorHop"
        ]
    );
}

#[test]
fn traces_java_var_field_receiver_calls_through_constructor_roots_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); Group inner() { return this; } }
class Holder { Group group = new Group(); }
class Caller {
    int run() {
        var v = new Holder().group.entry;
        return v.helper(1);
    }
    int runDirect() {
        var v = new Group().entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::run",
            "com::example::Caller::runDirect"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::run",
            "com::example::Caller::runDirect"
        ]
    );
}

#[test]
fn traces_java_var_static_type_factory_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util {
    static Helper make() { return new Helper(); }
    static Helper make(int value) { return new Helper(); }
    static class Nested {
        static Helper nestedMake() { return new Helper(); }
    }
}
interface Factory {
    static Helper make() { return new Helper(); }
}
class Caller {
    int simpleFactory() {
        var v = Util.make();
        return v.helper(1);
    }
    int arityFactory() {
        var v = Util.make(2);
        return v.helper(1);
    }
    int nestedFactory() {
        var v = Util.Nested.nestedMake();
        return v.helper(1);
    }
    int interfaceFactory() {
        var v = Factory.make();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::arityFactory",
            "com::example::Caller::interfaceFactory",
            "com::example::Caller::nestedFactory",
            "com::example::Caller::simpleFactory"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::arityFactory",
            "com::example::Caller::interfaceFactory",
            "com::example::Caller::nestedFactory",
            "com::example::Caller::simpleFactory"
        ]
    );
}

#[test]
fn traces_java_var_static_type_factory_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Helper { int helper(int value) { return value; } }
class Util { static Helper make() { return new Helper(); } }
class Caller {
    int run() {
        var v = Util.make();
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Helper::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_static_type_factory_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_path = factory_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let helper_path = helper_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Helper { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Helper;
public class Util {
    public static Helper make() { return new Helper(); }
    public static class Nested {
        public static Helper nestedMake() { return new Helper(); }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.factory.Util;
public class Caller {
    public int importedFactory() {
        var v = Util.make();
        return v.helper(1);
    }
    public int importedNestedFactory() {
        var v = Util.Nested.nestedMake();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactory",
            "pkg::caller::Caller::importedNestedFactory"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactory",
            "pkg::caller::Caller::importedNestedFactory"
        ]
    );
}

#[test]
fn java_var_static_type_factory_receiver_calls_fail_closed_for_unsupported_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Helper { int helper(int value) { return value; } }
class Util {
    static Helper make(int value) { return new Helper(); }
    static Helper varargs(int... values) { return new Helper(); }
}
class Caller {
    int arityMismatch() {
        var v = Util.make();
        return v.helper(1);
    }
    int varargsFactory() {
        var v = Util.varargs(1);
        return v.helper(1);
    }
    int unknownType() {
        var v = Missing.make();
        return v.helper(1);
    }
}
class ShadowingCaller {
    Helper Util;
    int shadowedByField() {
        var v = Util.make();
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Helper::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "arity-mismatched static factories, varargs factories, unknown types, and bound-name shadowing of static type receivers must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_static_factory_method_hop_field_receiver_calls_in_live_workspace_and_persisted_index()
 {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
}
class Util {
    static Group factory() { return new Group(); }
    static class Nested {
        static Group nestedFactory() { return new Group(); }
    }
}
class Caller {
    int staticFactoryHop() {
        var v = Util.factory().entry;
        return v.helper(1);
    }
    int nestedFactoryHop() {
        var v = Util.Nested.nestedFactory().entry;
        return v.helper(1);
    }
    int staticFactoryInstanceHop() {
        var v = Util.factory().inner().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 3);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::nestedFactoryHop",
            "com::example::Caller::staticFactoryHop",
            "com::example::Caller::staticFactoryInstanceHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 3);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::nestedFactoryHop",
            "com::example::Caller::staticFactoryHop",
            "com::example::Caller::staticFactoryInstanceHop"
        ]
    );
}

#[test]
fn traces_java_var_static_factory_method_hop_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Util { static Group factory() { return new Group(); } }
class Caller {
    int run() {
        var v = Util.factory().entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_static_factory_method_hop_field_receiver_calls_across_files_with_imports() {
    let dir = temporary_dir();
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_path = factory_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let helper_path = helper_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Helper { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Helper;
public class Util {
    public static Holder factory() { return new Holder(); }
    public static class Holder {
        public Helper entry = new Helper();
        public static Holder nestedFactory() { return new Holder(); }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import pkg.factory.Util;
public class Caller {
    public int importedFactoryHop() {
        var v = Util.factory().entry;
        return v.helper(1);
    }
    public int importedNestedFactoryHop() {
        var v = Util.Holder.nestedFactory().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactoryHop",
            "pkg::caller::Caller::importedNestedFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactoryHop",
            "pkg::caller::Caller::importedNestedFactoryHop"
        ]
    );
}

#[test]
fn java_var_static_factory_method_hop_field_receiver_calls_fail_closed_for_unsupported_references()
{
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
}
class Util {
    static Group factory(int value) { return new Group(); }
    static Group varargs(int... values) { return new Group(); }
    Group instanceFactory() { return new Group(); }
    static int primitive() { return 0; }
    static Missing missingReturn() { return null; }
}
class Caller {
    int instanceFactoryHop() {
        var v = Util.instanceFactory().entry;
        return v.helper(1);
    }
    int arityMismatch() {
        var v = Util.factory().entry;
        return v.helper(1);
    }
    int varargsFactory() {
        var v = Util.varargs(1).entry;
        return v.helper(1);
    }
    int unknownFactory() {
        var v = Util.missing().entry;
        return v.helper(1);
    }
    int primitiveReturn() {
        var v = Util.primitive().entry;
        return v.helper(1);
    }
    int unknownReturnType() {
        var v = Util.missingReturn().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "non-static factories, arity-mismatched factories, varargs factories, unknown factories, and primitive or unknown factory return types must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_java_var_factory_method_hop_field_receiver_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group {
    Entry entry = new Entry();
    Group inner() { return this; }
    Group makeFoo() { return new Group(); }
}
class Util {
    static Group make() { return new Group(); }
}
class Caller {
    Group group = new Group();
    Group makeFoo() { return new Group(); }
    int bareFactoryHop() {
        var v = makeFoo().entry;
        return v.helper(1);
    }
    int bareFactoryInstanceHop() {
        var v = makeFoo().inner().entry;
        return v.helper(1);
    }
    int staticTypeFactoryHop() {
        var v = Util.make().entry;
        return v.helper(1);
    }
    int boundFactoryHop() {
        var v = group.makeFoo().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bareFactoryHop",
            "com::example::Caller::bareFactoryInstanceHop",
            "com::example::Caller::boundFactoryHop",
            "com::example::Caller::staticTypeFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "com::example::Caller::bareFactoryHop",
            "com::example::Caller::bareFactoryInstanceHop",
            "com::example::Caller::boundFactoryHop",
            "com::example::Caller::staticTypeFactoryHop"
        ]
    );
}

#[test]
fn traces_java_var_factory_method_hop_field_receiver_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example; class Stale {}
",
    )
    .unwrap();
    let overlay = "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Caller {
    Group makeFoo() { return new Group(); }
    int run() {
        var v = makeFoo().entry;
        return v.helper(1);
    }
}
";
    let helper_symbol = "com::example::Entry::helper";

    let live = trace_symbol_graph_with_source(
        &dir,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "com::example::Caller::run");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        overlay,
        helper_symbol,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "com::example::Caller::run");
}

#[test]
fn traces_java_var_factory_method_hop_field_receiver_calls_across_files_with_static_import() {
    let dir = temporary_dir();
    let factory_dir = dir.join("src").join("pkg").join("factory");
    let caller_dir = dir.join("src").join("pkg").join("caller");
    let helper_dir = dir.join("src").join("pkg").join("helper");
    let factory_path = factory_dir.join("Util.java");
    let caller_path = caller_dir.join("Caller.java");
    let helper_path = helper_dir.join("Helper.java");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(&factory_dir).unwrap();
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(
        &helper_path,
        "package pkg.helper;
public class Helper { public int helper(int value) { return value; } }
",
    )
    .unwrap();
    fs::write(
        &factory_path,
        "package pkg.factory;
import pkg.helper.Helper;
public class Util {
    public static Holder make() { return new Holder(); }
    public static class Holder {
        public Helper entry = new Helper();
        public static Holder nestedMake() { return new Holder(); }
    }
}
",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "package pkg.caller;
import static pkg.factory.Util.make;
import static pkg.factory.Util.Holder.nestedMake;
public class Caller {
    public int importedFactoryHop() {
        var v = make().entry;
        return v.helper(1);
    }
    public int importedNestedFactoryHop() {
        var v = nestedMake().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let helper_symbol = "pkg::helper::Helper::helper";
    let live = trace_symbol_graph(&dir, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 2);
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactoryHop",
            "pkg::caller::Caller::importedNestedFactoryHop"
        ]
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, helper_symbol, TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 2);
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort();
    assert_eq!(
        callers,
        [
            "pkg::caller::Caller::importedFactoryHop",
            "pkg::caller::Caller::importedNestedFactoryHop"
        ]
    );
}

#[test]
fn java_var_factory_method_hop_field_receiver_calls_fail_closed_for_unsupported_references() {
    let dir = temporary_dir();
    let source_path = dir.join("Types.java");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package com.example;
class Entry { int helper(int value) { return value; } }
class Group { Entry entry = new Entry(); }
class Util {
    static Group make(int value) { return new Group(); }
}
class Caller {
    Group make(int value) { return new Group(); }
    void makeVoid() { }
    int primitive() { return 0; }
    int arityMismatch() {
        var v = make().entry;
        return v.helper(1);
    }
    int staticArityMismatch() {
        var v = Util.make().entry;
        return v.helper(1);
    }
    int unknownFactory() {
        var v = missing().entry;
        return v.helper(1);
    }
    int voidFactory() {
        var v = makeVoid().entry;
        return v.helper(1);
    }
    int primitiveFactory() {
        var v = primitive().entry;
        return v.helper(1);
    }
}
",
    )
    .unwrap();

    let target = "com::example::Entry::helper";
    let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
    assert!(
        live.callers.is_empty(),
        "arity-mismatched, unknown, void-returning, and primitive-returning factory method-call hops must fail closed"
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}
