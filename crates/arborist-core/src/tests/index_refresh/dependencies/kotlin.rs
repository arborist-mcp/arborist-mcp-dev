use super::*;
use crate::list_symbols_from_index;

#[test]
fn refreshes_kotlin_declarations_without_rebuilding_unrelated_sources() {
    let dir = temporary_dir();
    let source = dir.join("Counter.kt");
    let unrelated = dir.join("Unrelated.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "package demo\n\nclass Counter {\n    fun increment(value: Int) = value\n}\n",
    )
    .unwrap();
    fs::write(&unrelated, "package demo\n\nclass Unrelated\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &source,
        "package demo\n\nclass RenamedCounter {\n    fun increment(value: Int) = value\n}\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &source).unwrap();
    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let listed = list_symbols_from_index(&db_path, 10).unwrap();
    assert_eq!(
        listed
            .symbols
            .iter()
            .map(|symbol| symbol.semantic_path.as_str())
            .collect::<Vec<_>>(),
        vec![
            "demo::RenamedCounter",
            "demo::RenamedCounter::increment",
            "demo::Unrelated",
        ]
    );
}

#[test]
fn refreshes_kotlin_unique_explicit_import_dependents() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let importer = source_dir.join("Main.kt");
    let helper = source_dir.join("Helper.kt");
    let unrelated = source_dir.join("Unrelated.kt");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::write(
        &importer,
        "package com.example\n\nimport com.example.Helper\n\nclass Main\n",
    )
    .unwrap();
    fs::write(&helper, "package com.example\n\nclass Helper\n").unwrap();
    fs::write(&unrelated, "package com.example\n\nclass Unrelated\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &helper,
        "package com.example\n\nclass Helper {\n    val value = 1\n}\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_kotlin_wildcard_package_dependents() {
    let dir = temporary_dir();
    let source_dir = dir.join("src").join("com").join("example");
    let importer = dir.join("src").join("com").join("app").join("Main.kt");
    let helper = source_dir.join("Helper.kt");
    let other = source_dir.join("Other.kt");
    let unrelated = dir
        .join("src")
        .join("com")
        .join("other")
        .join("Unrelated.kt");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(importer.parent().unwrap()).unwrap();
    fs::create_dir_all(unrelated.parent().unwrap()).unwrap();
    fs::write(
        &importer,
        "package com.app\n\nimport com.example.*\n\nclass Main\n",
    )
    .unwrap();
    fs::write(&helper, "package com.example\n\nclass Helper\n").unwrap();
    fs::write(&other, "package com.example\n\nclass Other\n").unwrap();
    fs::write(&unrelated, "package com.other\n\nclass Unrelated\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &helper,
        "package com.example\n\nclass Helper {\n    val value = 1\n}\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 4);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 2);
}

#[test]
fn refreshes_kotlin_cross_package_import_dependents() {
    let dir = temporary_dir();
    let child_dir = dir.join("src").join("com").join("child");
    let base_dir = dir.join("src").join("com").join("base");
    let importer = child_dir.join("Main.kt");
    let base = base_dir.join("Base.kt");
    let unrelated = child_dir.join("Unrelated.kt");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&child_dir).unwrap();
    fs::create_dir_all(&base_dir).unwrap();
    fs::write(
        &importer,
        "package com.child\n\nimport com.base.Base\n\nclass Main\n",
    )
    .unwrap();
    fs::write(&base, "package com.base\n\nclass Base\n").unwrap();
    fs::write(&unrelated, "package com.child\n\nclass Unrelated\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &base,
        "package com.base\n\nclass Base {\n    val value = 1\n}\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &base).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_kotlin_extension_function_dependents() {
    let dir = temporary_dir();
    let caller = dir.join("Caller.kt");
    let extensions = dir.join("Extensions.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller,
        "package com.example\n\nclass Other\n\nclass Holder {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n",
    )
    .unwrap();
    fs::write(
        &extensions,
        "package com.example\n\nfun Other.helper(value: Int): Int = value\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let before =
        trace_symbol_graph_from_index(&db_path, "com::example::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(before.callers.len(), 1);
    assert_eq!(before.callers[0].symbol_id, "com::example::Holder::run");

    // Editing the caller must re-resolve the unchanged extension target (loaded from the
    // persisted index) instead of dropping the edge.
    fs::write(
        &caller,
        "package com.example\n\nclass Other\n\nclass Holder {\n    fun run(): Int {\n        val other = Other()\n        return other.helper(2)\n    }\n    fun second(): Int {\n        val other = Other()\n        return other.helper(1)\n    }\n}\n",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &caller).unwrap();
    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.rebuilt_files, 1);

    let after =
        trace_symbol_graph_from_index(&db_path, "com::example::helper", TraceDirection::Callers)
            .unwrap();
    let mut caller_ids = after
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    caller_ids.sort_unstable();
    assert_eq!(
        caller_ids,
        vec!["com::example::Holder::run", "com::example::Holder::second"]
    );
}

#[test]
fn refreshes_kotlin_same_package_top_level_function_dependents() {
    let dir = temporary_dir();
    let caller = dir.join("Caller.kt");
    let callee = dir.join("Helper.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller,
        "package com.example\n\nfun caller(): Int = helper(1)\n",
    )
    .unwrap();
    fs::write(
        &callee,
        "package com.example\n\nfun helper(value: Int): Int = value\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let before =
        trace_symbol_graph_from_index(&db_path, "com::example::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(before.callers.len(), 1);
    assert_eq!(before.callers[0].symbol_id, "com::example::caller");

    // Editing the callee must re-resolve the unchanged same-package caller
    // (loaded from the persisted index) instead of dropping the edge.
    fs::write(
        &callee,
        "package com.example\n\nfun helper(value: Int): Int = value + 1\n",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &callee).unwrap();
    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let after =
        trace_symbol_graph_from_index(&db_path, "com::example::helper", TraceDirection::Callers)
            .unwrap();
    assert_eq!(after.callers.len(), 1);
    assert_eq!(after.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn refreshes_kotlin_same_package_companion_receiver_dependents() {
    let dir = temporary_dir();
    let caller = dir.join("Caller.kt");
    let callee = dir.join("Config.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller,
        "package com.example\n\nfun caller(): Int = Config.helper(1)\n",
    )
    .unwrap();
    fs::write(
        &callee,
        "package com.example\n\nclass Config {\n    companion object {\n        fun helper(value: Int): Int = value\n    }\n}\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let helper_path = "com::example::Config::Companion::helper";
    let before =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(before.callers.len(), 1);
    assert_eq!(before.callers[0].symbol_id, "com::example::caller");

    // Editing the callee must re-resolve the unchanged qualified-receiver caller
    // instead of dropping the companion-member edge.
    fs::write(
        &callee,
        "package com.example\n\nclass Config {\n    companion object {\n        fun helper(value: Int): Int = value + 1\n    }\n}\n",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &callee).unwrap();
    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let after =
        trace_symbol_graph_from_index(&db_path, helper_path, TraceDirection::Callers).unwrap();
    assert_eq!(after.callers.len(), 1);
    assert_eq!(after.callers[0].symbol_id, "com::example::caller");
}

#[test]
fn refreshes_kotlin_same_package_object_chained_receiver_dependents() {
    let dir = temporary_dir();
    let caller = dir.join("Caller.kt");
    let callee = dir.join("Registry.kt");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller,
        "package com.example\n\nclass Holder {\n    fun run(value: Int): Int = value\n}\n\nfun caller(): Int = Registry.holder.run(1)\n",
    )
    .unwrap();
    fs::write(
        &callee,
        "package com.example\n\nobject Registry {\n    val holder = Holder()\n}\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let run_path = "com::example::Holder::run";
    let before =
        trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(before.callers.len(), 1);
    assert_eq!(before.callers[0].symbol_id, "com::example::caller");

    // Editing the callee must re-resolve the unchanged object-chained-receiver
    // caller instead of dropping the property-chain edge.
    fs::write(
        &callee,
        "package com.example\n\nobject Registry {\n    val holder = Holder()\n    val extra = 1\n}\n",
    )
    .unwrap();
    let stats = refresh_symbol_index_for_file(&dir, &db_path, &callee).unwrap();
    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.rebuilt_files, 1);
    assert_eq!(stats.reused_files, 1);

    let after = trace_symbol_graph_from_index(&db_path, run_path, TraceDirection::Callers).unwrap();
    assert_eq!(after.callers.len(), 1);
    assert_eq!(after.callers[0].symbol_id, "com::example::caller");
}
