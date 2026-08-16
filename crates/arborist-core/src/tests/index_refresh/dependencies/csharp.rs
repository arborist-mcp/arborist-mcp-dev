use std::collections::BTreeMap;

use super::*;
use crate::language::normalize_path;
use crate::symbol_index_state::load_symbol_index_with_overrides;

#[test]
fn refreshes_csharp_generic_nested_static_callers_when_the_target_becomes_instance_only() {
    let dir = temporary_dir();
    let outer = dir.join("Outer.cs");
    let caller = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &outer,
        "namespace Demo; class Outer<T> { class Helper<U> { public static void Ping() {} } }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo; class Caller { void Call() { Outer<int>.Helper<string>.Ping(); } }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Outer::Helper::Ping"]
    );

    fs::write(
        &outer,
        "namespace Demo; class Outer<T> { class Helper<U> { public void Ping() {} } }\n",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &outer).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let refreshed =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(refreshed.callees.is_empty());
}

#[test]
fn refreshes_csharp_global_namespace_imported_nested_type_callers_from_directive_only_files() {
    let dir = temporary_dir();
    let outer = dir.join("Outer.cs");
    let caller = dir.join("Caller.cs");
    let global_usings = dir.join("GlobalUsings.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &outer,
        "namespace Demo.Utility; class Outer { class Helper { public static void Ping() {} } }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo.App; class Caller { void Call() { Outer.Helper.Ping(); } }\n",
    )
    .unwrap();
    fs::write(&global_usings, "global using Demo.Utility;\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Utility::Outer::Helper::Ping"]
    );

    fs::write(&global_usings, "// no global namespace imports\n").unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &global_usings).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let refreshed =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(refreshed.callees.is_empty());
}

#[test]
fn refreshes_csharp_global_nested_type_alias_callers_from_directive_only_files() {
    let dir = temporary_dir();
    let outer = dir.join("Outer.cs");
    let caller = dir.join("Caller.cs");
    let global_usings = dir.join("GlobalUsings.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &outer,
        "namespace Demo; class Outer { class Helper { public static void Ping() {} } }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo.App; class Caller { void Call() { GlobalOuter.Helper.Ping(); } }\n",
    )
    .unwrap();
    fs::write(&global_usings, "global using GlobalOuter = Demo.Outer;\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Outer::Helper::Ping"]
    );

    fs::write(&global_usings, "// no global aliases\n").unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &global_usings).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let refreshed =
        trace_symbol_graph_from_index(&db_path, "Demo::App::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(refreshed.callees.is_empty());
}

#[test]
fn refreshes_csharp_nested_type_static_callers_when_a_nearer_outer_type_appears() {
    let dir = temporary_dir();
    let outer = dir.join("Outer.cs");
    let caller = dir.join("Caller.cs");
    let nearer = dir.join("Nearer.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &outer,
        "namespace Demo; class Outer { class Helper { public static void Ping() {} } }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo.App.Tools; class Caller { void Call() { Outer.Helper.Ping(); } }\n",
    )
    .unwrap();
    fs::write(&nearer, "namespace Demo.App; class Other {}\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial = trace_symbol_graph_from_index(
        &db_path,
        "Demo::App::Tools::Caller::Call",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Outer::Helper::Ping"]
    );

    fs::write(
        &nearer,
        "namespace Demo.App; class Outer { class Helper { public int Ping() => 0; } }\n",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &nearer).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let refreshed = trace_symbol_graph_from_index(
        &db_path,
        "Demo::App::Tools::Caller::Call",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(refreshed.callees.is_empty());
}

#[test]
fn refreshes_csharp_inherited_bare_callers_when_a_base_method_becomes_static() {
    let dir = temporary_dir();
    let base = dir.join("Base.cs");
    let derived = dir.join("Derived.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &base,
        "namespace Demo; class Base { public int Ping(int value) => value; }\n",
    )
    .unwrap();
    fs::write(
        &derived,
        "namespace Demo; class Derived : Base { int Call(int value) => Ping(value); }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial =
        trace_symbol_graph_from_index(&db_path, "Demo::Derived::Call", TraceDirection::Callees)
            .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Base::Ping"]
    );

    fs::write(
        &base,
        "namespace Demo; class Base { public static int Ping(int value) => value; }\n",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &base).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    // A static base method remains callable by simple name from a derived
    // instance method, so the refreshed bare call still resolves to the now
    // static `Base::Ping`; the refresh still rebuilds the changed base and
    // its inherited bare dependents together.
    let refreshed =
        trace_symbol_graph_from_index(&db_path, "Demo::Derived::Call", TraceDirection::Callees)
            .unwrap();
    assert_eq!(
        refreshed
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Base::Ping"]
    );
}

#[test]
fn refreshes_csharp_enclosing_namespace_static_dependents_when_a_nearer_type_appears() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.cs");
    let caller = dir.join("Caller.cs");
    let nearer = dir.join("Nearer.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "namespace Demo; class Helper { public static void Ping() {} }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo.App.Tools; class Caller { void Call() { Helper.Ping(); } }\n",
    )
    .unwrap();
    fs::write(&nearer, "namespace Demo.App; class Other {}\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial = trace_symbol_graph_from_index(
        &db_path,
        "Demo::App::Tools::Caller::Call",
        TraceDirection::Callees,
    )
    .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Helper::Ping"]
    );

    fs::write(
        &nearer,
        "namespace Demo.App; class Helper { public int Ping() => 0; }\n",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &nearer).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let refreshed = trace_symbol_graph_from_index(
        &db_path,
        "Demo::App::Tools::Caller::Call",
        TraceDirection::Callees,
    )
    .unwrap();
    assert!(refreshed.callees.is_empty());
}

#[test]
fn refreshes_csharp_dependents_when_a_type_becomes_ambiguous() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.cs");
    let caller = dir.join("Caller.cs");
    let competing = dir.join("Competing.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "namespace Demo; public static class Helper { public static void Ping() {} }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "using static Demo.Helper;\nnamespace Demo; class Caller { void Call() { Ping(); } }\n",
    )
    .unwrap();
    fs::write(&competing, "namespace Demo; class Other {}\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Helper::Ping"]
    );

    fs::write(&competing, "namespace Demo; class Helper {}\n").unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &competing).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let ambiguous =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(ambiguous.callees.is_empty());
}

#[test]
fn refreshes_csharp_global_using_dependents_from_directive_only_files() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.cs");
    let caller = dir.join("Caller.cs");
    let global_usings = dir.join("GlobalUsings.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "namespace Demo; public static class Helper { public static void Ping() {} }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo; class Caller { void Call() { Ping(); } }\n",
    )
    .unwrap();
    fs::write(&global_usings, "global using static Demo.Helper;\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Helper::Ping"]
    );

    fs::write(&global_usings, "// no global imports\n").unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &global_usings).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 2);

    let refreshed =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(refreshed.callees.is_empty());
}

#[test]
fn refreshes_csharp_global_using_dependents_from_persisted_index_overrides() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.cs");
    let caller = dir.join("Caller.cs");
    let global_usings = dir.join("GlobalUsings.cs");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "namespace Demo; public static class Helper { public static void Ping() {} }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "namespace Demo; class Caller { void Call() { Ping(); } }\n",
    )
    .unwrap();
    fs::write(&global_usings, "global using static Demo.Helper;\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let overrides = BTreeMap::from([(
        normalize_path(&global_usings),
        "// no global imports\n".to_string(),
    )]);
    let (symbols, indexed_files) = load_symbol_index_with_overrides(&db_path, &overrides).unwrap();

    assert_eq!(indexed_files, 3);
    let caller = symbols
        .iter()
        .find(|symbol| symbol.symbol_id == "Demo::Caller::Call")
        .unwrap();
    assert!(caller.dependencies.is_empty());
}

#[test]
fn refreshes_csharp_generic_static_import_callers_when_the_target_becomes_instance_only() {
    let dir = temporary_dir();
    let helper = dir.join("Helper.cs");
    let caller = dir.join("Caller.cs");
    let db_path = dir.join("symbols.db");
    fs::write(
        &helper,
        "namespace Demo; class Helper<T> { public static int Ping(int value) => value; }
",
    )
    .unwrap();
    fs::write(
        &caller,
        "using static Demo.Helper<int>;
namespace Demo; class Caller { int Call(int value) => Ping(value); }
",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let initial =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert_eq!(
        initial
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        ["Demo::Helper::Ping"]
    );

    fs::write(
        &helper,
        "namespace Demo; class Helper<T> { public int Ping(int value) => value; }
",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let refreshed =
        trace_symbol_graph_from_index(&db_path, "Demo::Caller::Call", TraceDirection::Callees)
            .unwrap();
    assert!(refreshed.callees.is_empty());
}
