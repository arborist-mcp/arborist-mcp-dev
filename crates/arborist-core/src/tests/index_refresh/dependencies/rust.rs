use super::*;

#[test]
fn refreshes_rust_out_of_line_module_dependents() {
    let dir = temporary_dir();
    let root = dir.join("lib.rs");
    let api = dir.join("api.rs");
    let helper_dir = dir.join("api");
    let helper = helper_dir.join("helper.rs");
    let unrelated = dir.join("unrelated.rs");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&helper_dir).unwrap();
    fs::write(&root, "mod api;\n").unwrap();
    fs::write(
        &api,
        "mod helper;\npub fn api() -> i32 { helper::value() }\n",
    )
    .unwrap();
    fs::write(&helper, "pub fn value() -> i32 { 1 }\n").unwrap();
    fs::write(&unrelated, "pub fn unrelated() -> i32 { 0 }\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&helper, "pub fn value() -> i32 { 2 }\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 4);
    assert_eq!(stats.rebuilt_files, 3);
    assert_eq!(stats.reused_files, 1);
}
