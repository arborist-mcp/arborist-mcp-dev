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
