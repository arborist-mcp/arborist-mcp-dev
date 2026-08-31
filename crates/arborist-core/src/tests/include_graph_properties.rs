use std::fs;

use super::support::temporary_dir;
use crate::symbol_index_workspace::{
    expanded_refresh_file_paths, transitive_local_file_dependents,
};
use crate::workspace_scan::{WorkspaceScanDeadline, WorkspaceScanLimits};

fn scan_deadline() -> WorkspaceScanDeadline {
    WorkspaceScanDeadline::new(WorkspaceScanLimits::default()).unwrap()
}

fn write_c_source(path: &std::path::Path, includes: &[&str]) {
    let mut source = String::new();
    for include in includes {
        source.push_str(&format!("#include \"{include}\"\n"));
    }
    source.push_str("\nint value(void) {\n    return 1;\n}\n");
    fs::write(path, source).unwrap();
}

#[test]
fn transitive_dependents_close_over_multi_hop_chains() {
    let dir = temporary_dir();
    // base.h <- mid.h <- top.c: refreshing base.h must reach the full chain.
    write_c_source(&dir.join("base.h"), &[]);
    write_c_source(&dir.join("mid.h"), &["base.h"]);
    write_c_source(&dir.join("top.c"), &["mid.h"]);

    let dependents = transitive_local_file_dependents(&dir, &dir.join("base.h")).unwrap();

    let names: Vec<String> = dependents
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["mid.h".to_string(), "top.c".to_string()]);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn transitive_dependents_deduplicate_diamond_reachability() {
    let dir = temporary_dir();
    // base.h is included by both mid_a and mid_b, which are both included by
    // top.c; the diamond must report each dependent exactly once.
    write_c_source(&dir.join("base.h"), &[]);
    write_c_source(&dir.join("mid_a.c"), &["base.h"]);
    write_c_source(&dir.join("mid_b.c"), &["base.h"]);
    write_c_source(&dir.join("top.h"), &["mid_a.c", "mid_b.c"]);

    let dependents = transitive_local_file_dependents(&dir, &dir.join("base.h")).unwrap();

    let names: Vec<String> = dependents
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        names,
        vec![
            "mid_a.c".to_string(),
            "mid_b.c".to_string(),
            "top.h".to_string(),
        ]
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn transitive_dependents_exclude_unrelated_files_and_self() {
    let dir = temporary_dir();
    write_c_source(&dir.join("base.h"), &[]);
    write_c_source(&dir.join("user.c"), &["base.h"]);
    write_c_source(&dir.join("other.py"), &[]);

    let dependents = transitive_local_file_dependents(&dir, &dir.join("base.h")).unwrap();

    let names: Vec<String> = dependents
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["user.c".to_string()]);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn transitive_dependents_are_deterministic_across_runs() {
    let dir = temporary_dir();
    // header0.h is included directly by user0_0.c and user0_1.c; other
    // headers form an unrelated cluster.
    write_c_source(&dir.join("header0.h"), &[]);
    write_c_source(&dir.join("user0_0.c"), &["header0.h"]);
    write_c_source(&dir.join("user0_1.c"), &["header0.h"]);
    for index in 1..4 {
        let header_name = format!("header{index}.h");
        write_c_source(&dir.join(&header_name), &[]);
        write_c_source(
            &dir.join(format!("user{index}.c")),
            &[&format!("header{index}.h")],
        );
    }

    let first = transitive_local_file_dependents(&dir, &dir.join("header0.h")).unwrap();
    let second = transitive_local_file_dependents(&dir, &dir.join("header0.h")).unwrap();

    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
    assert!(first.contains(&dir.join("user0_0.c")));
    assert!(first.contains(&dir.join("user0_1.c")));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn expanded_refresh_paths_include_self_even_without_dependents() {
    let dir = temporary_dir();
    write_c_source(&dir.join("base.h"), &[]);

    let deadline = scan_deadline();
    let refresh_paths = expanded_refresh_file_paths(
        &dir,
        &dir.join("base.h"),
        WorkspaceScanLimits::default(),
        &deadline,
    )
    .unwrap();

    assert_eq!(refresh_paths, vec![dir.join("base.h")]);
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn expanded_refresh_paths_union_self_with_transitive_dependents() {
    let dir = temporary_dir();
    // base.h <- mid.h <- top.c: the expanded set is {self} ∪ dependents.
    write_c_source(&dir.join("base.h"), &[]);
    write_c_source(&dir.join("mid.h"), &["base.h"]);
    write_c_source(&dir.join("top.c"), &["mid.h"]);

    let deadline = scan_deadline();
    let refresh_paths = expanded_refresh_file_paths(
        &dir,
        &dir.join("base.h"),
        WorkspaceScanLimits::default(),
        &deadline,
    )
    .unwrap();

    let names: Vec<String> = refresh_paths
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(
        names,
        vec![
            "base.h".to_string(),
            "mid.h".to_string(),
            "top.c".to_string(),
        ]
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn expanded_refresh_paths_honor_workspace_file_size_limit_during_reverse_scan() {
    let dir = temporary_dir();
    write_c_source(&dir.join("base.h"), &[]);
    fs::write(
        dir.join("oversized.c"),
        "#include \"base.h\"\n".to_string() + &"x".repeat(256),
    )
    .unwrap();

    let limits = WorkspaceScanLimits::with_max_file_bytes(128);
    let deadline = WorkspaceScanDeadline::new(limits).unwrap();
    let error = expanded_refresh_file_paths(&dir, &dir.join("base.h"), limits, &deadline)
        .expect_err("reverse dependency scans must enforce workspace file-size limits");

    println!("reverse scan error: {error:#}");
    assert!(
        error
            .to_string()
            .contains("workspace scan source file too large")
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn expanded_refresh_paths_gate_expansion_by_language_adapter() {
    let dir = temporary_dir();
    // An unsupported extension fails language detection outright...
    write_c_source(&dir.join("base.h"), &[]);

    let deadline = scan_deadline();
    let error = expanded_refresh_file_paths(
        &dir,
        &dir.join("missing.xyz"),
        WorkspaceScanLimits::default(),
        &deadline,
    )
    .expect_err("unsupported extensions should fail language detection");
    assert!(error.to_string().contains("unsupported file extension"));

    // ...while a supported adapter without incremental file dependencies
    // (Python) refreshes exactly the requested file.
    fs::write(dir.join("helper.py"), "def helper():\n    return 1\n").unwrap();
    let python_paths = expanded_refresh_file_paths(
        &dir,
        &dir.join("helper.py"),
        WorkspaceScanLimits::default(),
        &deadline,
    )
    .unwrap();
    assert_eq!(python_paths, vec![dir.join("helper.py")]);
    fs::remove_dir_all(dir).unwrap();
}
