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
