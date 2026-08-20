use super::*;

#[test]
fn refreshes_python_local_import_dependents() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let unrelated = dir.join("unrelated.py");
    let db_path = dir.join("symbols.db");

    fs::write(&helper, "def helper(value):\n    return value + 1\n").unwrap();
    fs::write(
        &caller,
        "from helper import helper\n\ndef caller(value):\n    return helper(value)\n",
    )
    .unwrap();
    fs::write(&unrelated, "def unrelated():\n    return 0\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&helper, "def helper(value):\n    return value + 2\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_python_wildcard_package_dependents() {
    let dir = temporary_dir();
    let package = dir.join("pkg/__init__.py");
    let caller = dir.join("caller.py");
    let unrelated = dir.join("unrelated.py");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(package.parent().unwrap()).unwrap();
    fs::write(&package, "def exported(value):\n    return value + 1\n").unwrap();
    fs::write(
        &caller,
        "from pkg import *\n\ndef caller(value):\n    return exported(value)\n",
    )
    .unwrap();
    fs::write(&unrelated, "def unrelated():\n    return 0\n").unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&package, "def exported(value):\n    return value + 2\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &package).unwrap();
    assert_eq!(stats.rebuilt_files, 2);
    assert_eq!(stats.reused_files, 1);
}
