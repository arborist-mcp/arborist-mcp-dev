use super::*;

#[test]
fn refreshes_go_local_package_import_dependents() {
    let dir = temporary_dir();
    let command_dir = dir.join("cmd");
    let package_dir = dir.join("internal").join("service");
    let command = command_dir.join("main.go");
    let first = package_dir.join("first.go");
    let second = package_dir.join("second.go");
    let unrelated = dir.join("unrelated.go");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&command_dir).unwrap();
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &command,
        "package main\n\nimport \"example.com/project/internal/service\"\n\nfunc main() { service.Value() }\n",
    )
    .unwrap();
    fs::write(&first, "package service\nfunc Value() int { return 1 }\n").unwrap();
    fs::write(&second, "package service\nfunc Other() int { return 2 }\n").unwrap();
    fs::write(
        &unrelated,
        "package project\nfunc Unrelated() int { return 0 }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&second, "package service\nfunc Other() int { return 3 }\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &second).unwrap();
    assert_eq!(stats.indexed_files, 4);
    assert_eq!(stats.rebuilt_files, 3);
    assert_eq!(stats.reused_files, 1);
}

#[test]
fn refreshes_go_same_package_direct_call_dependents() {
    let dir = temporary_dir();
    let caller = dir.join("caller.go");
    let helper = dir.join("helper.go");
    let unrelated = dir.join("unrelated.go");
    let db_path = dir.join("symbols.db");

    fs::write(
        &caller,
        "package metrics\nfunc Caller() int { return Helper() }\n",
    )
    .unwrap();
    fs::write(&helper, "package metrics\nfunc Helper() int { return 1 }\n").unwrap();
    fs::write(
        &unrelated,
        "package metrics\nfunc Unrelated() int { return 0 }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(&helper, "package metrics\nfunc Helper() int { return 2 }\n").unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &helper).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 3);
    assert_eq!(stats.reused_files, 0);
}

#[test]
fn refreshes_go_embedded_interface_dependents_across_same_package_files() {
    let dir = temporary_dir();
    let base = dir.join("base.go");
    let worker = dir.join("worker.go");
    let unrelated = dir.join("unrelated.go");
    let db_path = dir.join("symbols.db");

    fs::write(
        &base,
        "package metrics\ntype Base interface { Run() error }\n",
    )
    .unwrap();
    fs::write(
        &worker,
        "package metrics\ntype Worker interface { Base }\nfunc Caller(worker Worker) error { return worker.Run() }\n",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package metrics\nfunc Unrelated() int { return 0 }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &base,
        "package metrics\ntype Base interface { Run() error; Stop() error }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &base).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 3);
    assert_eq!(stats.reused_files, 0);
}

#[test]
fn refreshes_go_cross_file_interface_factory_dependents() {
    let dir = temporary_dir();
    let caller = dir.join("caller.go");
    let factory = dir.join("factory.go");
    let unrelated = dir.join("unrelated.go");
    let db_path = dir.join("symbols.db");

    fs::write(
        &caller,
        "package metrics\nfunc Caller() error { return NewWorker().Run(1) }\n",
    )
    .unwrap();
    fs::write(
        &factory,
        "package metrics\ntype Worker interface { Run(value int) error }\nfunc NewWorker() Worker { return nil }\n",
    )
    .unwrap();
    fs::write(
        &unrelated,
        "package metrics\nfunc Unrelated() int { return 0 }\n",
    )
    .unwrap();

    rebuild_symbol_index(&dir, &db_path).unwrap();
    fs::write(
        &factory,
        "package metrics\ntype Worker interface { Run(value int) error; Stop() error }\nfunc NewWorker() Worker { return nil }\n",
    )
    .unwrap();

    let stats = refresh_symbol_index_for_file(&dir, &db_path, &factory).unwrap();
    assert_eq!(stats.indexed_files, 3);
    assert_eq!(stats.rebuilt_files, 3);
    assert_eq!(stats.reused_files, 0);
}
