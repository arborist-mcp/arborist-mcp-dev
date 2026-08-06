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
