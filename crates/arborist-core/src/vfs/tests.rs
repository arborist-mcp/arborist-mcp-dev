use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::VirtualFileSystem;
use crate::language::{point_for_offset, position_from};
use crate::{Position, PositionEdit, TraceDirection, trace_symbol_graph_from_index};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[test]
fn applies_incremental_edit_and_commits() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert_eq!(snapshot.version, 0);
    let digit_offset = snapshot.source.rfind('1').unwrap();

    let result = vfs
        .apply_edit(&file, digit_offset, digit_offset + 1, "2")
        .unwrap();
    assert!(result.incremental_parse);
    assert!(result.dirty);
    assert_eq!(result.version, 1);
    assert!(result.source.contains("return 2"));

    let committed = vfs.commit_file(&file).unwrap();
    assert!(!committed.dirty);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
}

#[test]
fn path_aliases_share_one_virtual_entry() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let alias_dir = file.parent().unwrap().join("child");
    fs::create_dir_all(&alias_dir).unwrap();
    let alias = alias_dir.join("..").join("buffer.py");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&alias).unwrap();
    assert!(!snapshot.file.contains("/../"));
    let digit_offset = snapshot.source.rfind('1').unwrap();

    vfs.apply_edit(&file, digit_offset, digit_offset + 1, "2")
        .unwrap();

    let statuses = vfs.virtual_file_statuses(false).unwrap();
    assert_eq!(statuses.len(), 1);
    assert!(statuses[0].dirty);

    let aliased_snapshot = vfs.read_file(&alias).unwrap();
    assert!(aliased_snapshot.source.contains("return 2"));

    let committed = vfs.commit_file(&alias).unwrap();
    assert!(!committed.dirty);
    assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
}

#[test]
fn discards_virtual_changes() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    let digit_offset = snapshot.source.rfind('1').unwrap();
    vfs.apply_edit(&file, digit_offset, digit_offset + 1, "9")
        .unwrap();
    let discarded = vfs.discard_file(&file).unwrap();

    assert!(!discarded.dirty);
    assert!(discarded.source.contains("return 1"));
}

#[test]
fn discarding_unchanged_file_is_idempotent() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let first = vfs.discard_file(&file).unwrap();
    let second = vfs.discard_file(&file).unwrap();

    assert_eq!(first, initial);
    assert_eq!(second, initial);
}

#[test]
fn generated_position_edits_preserve_order_and_vfs_idempotence() {
    for (left, right, updated_left, updated_right) in generated_edit_cases() {
        let source = format!("def value() -> str:\n    return \"{left}:{right}\"\n");
        let file = temp_file(&source);
        let mut vfs = VirtualFileSystem::new();

        let left_start = source.find(left).unwrap();
        let after_left = source.replacen(left, updated_left, 1);
        let right_start = after_left.rfind(right).unwrap();
        let expected = after_left.replacen(right, updated_right, 1);
        let edits = [
            PositionEdit {
                start: position_at(&source, left_start),
                end: position_at(&source, left_start + left.len()),
                new_text: updated_left.to_string(),
            },
            PositionEdit {
                start: position_at(&after_left, right_start),
                end: position_at(&after_left, right_start + right.len()),
                new_text: updated_right.to_string(),
            },
        ];

        let edited = vfs.apply_position_edits(&file, &edits).unwrap();
        assert_eq!(edited.source, expected, "edit order failed for {source:?}");
        assert!(edited.dirty);

        let committed = vfs.commit_file(&file).unwrap();
        let repeated_commit = vfs.commit_file(&file).unwrap();
        assert_eq!(committed, repeated_commit, "commit must be idempotent");
        assert_eq!(fs::read_to_string(&file).unwrap(), expected);

        let revert_start = expected.find(updated_left).unwrap();
        vfs.apply_edit(&file, revert_start, revert_start + updated_left.len(), left)
            .unwrap();
        let discarded = vfs.discard_file(&file).unwrap();
        let repeated_discard = vfs.discard_file(&file).unwrap();
        assert_eq!(discarded, repeated_discard, "discard must be idempotent");
        assert_eq!(discarded.source, expected);
        assert!(!discarded.dirty);
    }
}

#[test]
fn rejects_byte_edit_inside_utf8_character() {
    let file = temp_file("def value() -> str:\n    return 'é'\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    let character_start = snapshot.source.find('é').unwrap();
    let interior_byte = character_start + 1;
    let error = vfs
        .apply_edit(&file, interior_byte, interior_byte, "x")
        .expect_err("byte edits must not split UTF-8 characters");

    assert!(
        error
            .to_string()
            .contains("edit range must align to UTF-8 character boundaries")
    );
    let unchanged = vfs.read_file(&file).unwrap();
    assert!(!unchanged.dirty);
    assert_eq!(unchanged.source, snapshot.source);
}

#[test]
fn discard_refreshes_from_current_disk_source() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    vfs.open_file(&file, Some("def value() -> int:\n    return 9\n"))
        .unwrap();
    fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
    let discarded = vfs.discard_file(&file).unwrap();

    assert!(!discarded.dirty);
    assert!(discarded.source.contains("return 2"));
    assert_eq!(discarded.disk_source, discarded.source);
}

#[test]
fn refreshes_clean_file_deleted_on_disk_as_empty() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert_eq!(snapshot.version, 0);

    fs::remove_file(&file).unwrap();
    let refreshed = vfs.read_file(&file).unwrap();

    assert!(!refreshed.dirty);
    assert_eq!(refreshed.source, "");
    assert_eq!(refreshed.disk_source, "");
    assert_eq!(refreshed.version, 1);
}

#[test]
fn commit_refreshes_clean_file_changed_on_disk() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert_eq!(snapshot.version, 0);

    fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
    let committed = vfs.commit_file(&file).unwrap();

    assert!(!committed.dirty);
    assert!(committed.source.contains("return 2"));
    assert_eq!(committed.disk_source, committed.source);
    assert_eq!(committed.version, 1);
}

#[test]
fn patches_virtual_symbol_without_immediate_commit() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .patch_node(&file, "value", "def value() -> int:\n    return 3\n", None)
        .unwrap();

    assert!(result.applied);
    let snapshot = vfs.read_file(&file).unwrap();
    assert!(snapshot.dirty);
    assert!(snapshot.source.contains("return 3"));
    assert!(fs::read_to_string(&file).unwrap().contains("return 1"));
}

#[test]
fn patches_virtual_symbol_at_position_without_immediate_commit() {
    let file = temp_file(
        "def decorator(func):\n    return func\n\n@decorator\ndef value() -> int:\n    return 1\n",
    );
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .patch_node_at_position(
            &file,
            &Position { row: 3, column: 1 },
            "def value() -> int:\n    return 3\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(result.resolved_path, "value");
    assert!(
        result
            .validation
            .syntax_errors
            .iter()
            .any(|issue| issue.kind == "decorator_guard")
    );
    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert!(snapshot.source.contains("@decorator"));
    assert!(snapshot.source.contains("return 1"));
    assert!(fs::read_to_string(&file).unwrap().contains("@decorator"));
}

#[test]
fn rejects_blank_virtual_patch_without_dirtying_buffer() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .patch_node(&file, "value", " \t", None)
        .expect_err("blank virtual patch replacements should be rejected");

    assert!(error.to_string().contains("new_code"));
    assert!(error.to_string().contains("blank"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}

#[test]
fn rejects_blank_virtual_patch_bypass_without_dirtying_buffer() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .patch_node(
            &file,
            "value",
            "def value() -> int:\n    return 2\n",
            Some(" \t"),
        )
        .expect_err("blank virtual patch bypass reasons should be rejected");

    assert!(error.to_string().contains("bypass_reason"));
    assert!(error.to_string().contains("blank"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}

#[test]
fn rolls_back_invalid_virtual_patch() {
    let file = temp_file(
        "def helper(value: int) -> int:\n    return value + 1\n\ndef value() -> int:\n    return helper(1)\n",
    );
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .patch_node(
            &file,
            "value",
            "def value() -> int:\n    return missing_helper(1)\n",
            None,
        )
        .unwrap();

    assert!(!result.applied);
    assert_eq!(
        result.validation.unresolved_identifiers,
        vec!["missing_helper"]
    );

    let snapshot = vfs.read_file(&file).unwrap();
    assert!(!snapshot.dirty);
    assert!(snapshot.source.contains("return helper(1)"));
}

#[test]
fn rolls_back_virtual_patch_when_validation_errors() {
    let workspace = temp_workspace();
    let file = workspace.join("sample.c");
    let bad_include = workspace.join("bad.txt");
    fs::write(&bad_include, "int helper(void);\n").unwrap();
    fs::write(
        &file,
        "#include \"bad.txt\"\n\nint value(void) {\n    return 1;\n}\n",
    )
    .unwrap();
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .patch_node(
            &file,
            "value",
            "int value(void) {\n    return helper();\n}\n",
            None,
        )
        .expect_err("validation errors should reject the virtual patch");

    assert!(
        error
            .to_string()
            .contains("failed to validate virtual patch")
    );
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}

#[test]
fn opens_with_virtual_source_and_lists_dirty_files() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let snapshot = vfs
        .open_file(&file, Some("def value() -> int:\n    return 7\n"))
        .unwrap();
    assert!(snapshot.dirty);
    assert!(snapshot.source.contains("return 7"));
    assert!(snapshot.disk_source.contains("return 1"));

    let dirty_files = vfs.virtual_file_statuses(true).unwrap();
    assert_eq!(dirty_files.len(), 1);
    assert_eq!(dirty_files[0].file, snapshot.file);
    assert!(dirty_files[0].dirty);
}

#[test]
fn open_with_source_refreshes_disk_baseline() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    let initial = vfs.read_file(&file).unwrap();
    assert!(!initial.dirty);

    fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
    let reopened = vfs
        .open_file(&file, Some("def value() -> int:\n    return 2\n"))
        .unwrap();

    assert!(!reopened.dirty);
    assert!(reopened.source.contains("return 2"));
    assert_eq!(reopened.disk_source, reopened.source);
}

#[test]
fn list_virtual_files_refreshes_clean_disk_changes() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    vfs.read_file(&file).unwrap();
    fs::write(&file, "def value(\n").unwrap();
    let statuses = vfs.virtual_file_statuses(false).unwrap();

    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].version, 1);
    assert!(statuses[0].syntax_error_count > 0);
    assert!(!statuses[0].dirty);
}

#[test]
fn applies_position_edits_in_sequence() {
    let file = temp_file("def value() -> int:\n    return 10\n");
    let mut vfs = VirtualFileSystem::new();

    let result = vfs
        .apply_position_edits(
            &file,
            &[
                PositionEdit {
                    start: Position { row: 1, column: 11 },
                    end: Position { row: 1, column: 13 },
                    new_text: "20".to_string(),
                },
                PositionEdit {
                    start: Position { row: 1, column: 0 },
                    end: Position { row: 1, column: 0 },
                    new_text: "# staged\n".to_string(),
                },
            ],
        )
        .unwrap();

    assert!(result.source.contains("return 20"));
    assert!(result.source.contains("# staged"));
    assert!(result.dirty);
}

#[test]
fn empty_position_edits_report_current_syntax_errors() {
    let file = temp_file("def value(\n");
    let mut vfs = VirtualFileSystem::new();

    let result = vfs.apply_position_edits(&file, &[]).unwrap();

    assert!(result.incremental_parse);
    assert!(!result.validation.syntax_errors.is_empty());
    assert!(result.validation.resolved_identifiers.is_empty());
    assert_eq!(result.validation.commit_gate.status, "not_evaluated");
}

#[test]
fn rolls_back_position_edits_when_later_edit_fails() {
    let file = temp_file("def value() -> int:\n    return 10\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .apply_position_edits(
            &file,
            &[
                PositionEdit {
                    start: Position { row: 1, column: 11 },
                    end: Position { row: 1, column: 13 },
                    new_text: "20".to_string(),
                },
                PositionEdit {
                    start: Position { row: 99, column: 0 },
                    end: Position { row: 99, column: 0 },
                    new_text: "# bad\n".to_string(),
                },
            ],
        )
        .expect_err("later edit failure should reject the whole batch");

    assert!(
        error
            .to_string()
            .contains("failed to apply position edit at index 1")
    );
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}

#[test]
fn rolls_back_position_edits_when_later_edit_splits_utf8_character() {
    let file = temp_file("def value() -> str:\n    return 'é'\n");
    let mut vfs = VirtualFileSystem::new();
    let initial = vfs.read_file(&file).unwrap();

    let error = vfs
        .apply_position_edits(
            &file,
            &[
                PositionEdit {
                    start: Position { row: 0, column: 0 },
                    end: Position { row: 0, column: 0 },
                    new_text: "# staged\n".to_string(),
                },
                PositionEdit {
                    start: Position { row: 2, column: 13 },
                    end: Position { row: 2, column: 13 },
                    new_text: "x".to_string(),
                },
            ],
        )
        .expect_err("position edits must not split UTF-8 characters");

    assert!(
        error
            .to_string()
            .contains("failed to apply position edit at index 1")
    );
    let error_chain = format!("{error:#}");
    assert!(error_chain.contains("does not align to a UTF-8 character boundary"));
    let snapshot = vfs.read_file(&file).unwrap();
    assert_eq!(snapshot.source, initial.source);
    assert_eq!(snapshot.version, initial.version);
    assert_eq!(snapshot.dirty, initial.dirty);
}

#[test]
fn closes_virtual_file_without_persisting_changes() {
    let file = temp_file("def value() -> int:\n    return 1\n");
    let mut vfs = VirtualFileSystem::new();

    vfs.open_file(&file, Some("def value() -> int:\n    return 8\n"))
        .unwrap();
    let snapshot = vfs.close_file(&file, false).unwrap();

    assert!(!snapshot.dirty);
    assert!(snapshot.source.contains("return 1"));
    assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
    assert!(fs::read_to_string(&file).unwrap().contains("return 1"));
}

#[test]
fn traces_symbol_graph_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.patch_node(
        &helper_path,
        "helper",
        "def helper(value: int) -> int:\n    return branch(value)\n",
        None,
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
        .unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "branch")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "leaf")
    );
    assert!(
        fs::read_to_string(&helper_path)
            .unwrap()
            .contains("return leaf")
    );
}

#[test]
fn traces_cpp_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(
        &source,
        "namespace api {\nclass Counter {\npublic:\n    Counter(int value) {}\n};\nCounter caller(int value) { return Counter{}; }\n}\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace lib { class Counter { public: Counter(int value) {} }; }\nnamespace api { using namespace lib; Counter caller(int value) { return Counter{value}; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["lib::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_new_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nint caller(int value) { auto counter = new api::Counter(value); return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_default_new_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller() { return 0; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter() {} }; }\nint caller() { auto counter = new api::Counter; return 0; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter()"]
    );
}

#[test]
fn traces_cpp_braced_initializer_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nint caller(int value) { api::Counter counter{value}; return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("alias.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using First = api::Counter; using Alias = First; int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_typedef_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("alias.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { typedef api::Counter Alias; int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_cv_qualified_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("alias.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = const volatile api::Counter; int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_this_member_template_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: template <typename T> T adjust(T value) { return value; } int caller(int value) { return this->template adjust<int>(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(T)"]
    );
}

#[test]
fn traces_cpp_this_member_template_specializations_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: template <typename T> T adjust(T value) { return value; } int caller(int value) { return this->template adjust< int >(value); } }; template <> int Counter::adjust<int>(int value) { return value + 1; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust<int>(int)"]
    );
}

#[test]
fn traces_cpp_this_member_lvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) && { return value + 1; } int caller(int value) { return this->adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) &"]
    );
}

#[test]
fn traces_cpp_temporary_member_rvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) && { return value + 1; } }; int caller(int value) { return Counter{}.adjust(value); } int moved_caller(int value) { return std::move(Counter{}).adjust(value); } }\n",
        ),
    )
    .unwrap();

    for caller in ["api::caller", "api::moved_caller"] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
            .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec!["api::Counter::adjust(int) &&"],
            "{caller}",
        );
    }
}

#[test]
fn traces_cpp_moved_this_member_rvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) && { return value + 1; } int adjust(int value) & { return value; } int caller(int value) && { return std::move(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) &&"]
    );
}

#[test]
fn traces_cpp_nested_this_member_receivers_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) && { return value + 1; } int adjust(int value) const & { return value + 2; } int adjust(int value) const && { return value + 3; } int parenthesized_caller(int value) { return (((*this))).adjust(value); } int moved_caller(int value) { return (std::move(static_cast<Counter &>(*this))).adjust(value); } int const_moved_caller(int value) { return std::move(std::as_const(((*this)))).adjust(value); } int forwarded_caller(int value) { return ((std::forward<Counter const &>(((*this))))).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        (
            "api::Counter::parenthesized_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::Counter::moved_caller", "api::Counter::adjust(int) &&"),
        (
            "api::Counter::const_moved_caller",
            "api::Counter::adjust(int) const &&",
        ),
        (
            "api::Counter::forwarded_caller",
            "api::Counter::adjust(int) const &",
        ),
    ] {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
            .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
            "{caller}",
        );
    }
}

#[test]
fn traces_cpp_cast_this_member_rvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) && { return value + 1; } int adjust(int value) & { return value; } int caller(int value) && { return static_cast< Counter && >(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) &&"]
    );
}

#[test]
fn traces_cpp_const_cast_this_member_rvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) const && { return value + 1; } int adjust(int value) && { return value; } int caller(int value) && { return static_cast<const Counter&&>(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &&"]
    );
}

#[test]
fn traces_cpp_const_cast_this_member_lvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) const & { return value + 1; } int adjust(int value) & { return value; } int caller(int value) { return static_cast<Counter const &>(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &"]
    );
}

#[test]
fn traces_cpp_as_const_this_member_lvalue_ref_overloads_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) const & { return value + 1; } int adjust(int value) & { return value; } int caller(int value) { return std::as_const(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "api::Counter::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::adjust(int) const &"]
    );
}

#[test]
fn traces_cpp_forward_this_member_calls_with_value_categories_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) const & { return value + 3; } int adjust(int value) & { return value + 2; } int adjust(int value) const && { return value + 1; } int adjust(int value) && { return value; } int rvalue_caller(int value) { return std::forward<Counter>(*this).adjust(value); } int const_lvalue_caller(int value) { return std::forward<Counter const &>(*this).adjust(value); } }; }\n",
        ),
    )
    .unwrap();

    let expected_callees = [
        (
            "api::Counter::rvalue_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::Counter::const_lvalue_caller",
            "api::Counter::adjust(int) const &",
        ),
    ];
    for (caller, expected_callee) in expected_callees {
        let trace = vfs
            .trace_symbol_graph(&workspace, caller, TraceDirection::Both)
            .unwrap();
        assert_eq!(
            trace
                .callees
                .iter()
                .map(|symbol| symbol.symbol_id.as_str())
                .collect::<Vec<_>>(),
            vec![expected_callee],
        );
    }
}

#[test]
fn traces_cpp_header_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn does_not_trace_cpp_header_type_aliases_moved_after_the_caller_in_virtual_source() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &header,
        "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &caller,
        Some(
            "namespace app { int caller(int value) { Alias counter{value}; return value; } }\n#include \"aliases.hpp\"\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn traces_cpp_type_aliases_from_virtual_local_headers() {
    let workspace = temp_workspace();
    let header = workspace.join("aliases.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &caller,
        "#include \"aliases.hpp\"\nnamespace app { int caller(int value) { Alias counter{value}; return value; } }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &header,
        Some(
            "namespace api { class Counter { public: Counter(int value) {} }; }\nnamespace app { using Alias = api::Counter; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Counter::Counter(int)"]
    );
}

#[test]
fn traces_cpp_qualified_namespace_aliases_from_virtual_local_headers_in_order() {
    let workspace = temp_workspace();
    let header = workspace.join("imports.hpp");
    let caller = workspace.join("caller.cpp");
    fs::write(
        &caller,
        "#include \"imports.hpp\"\nint caller() { return detail::convert(1); }\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &header,
        Some(
            "namespace implementation { int convert(int value) { return value; } }\nnamespace detail = implementation;\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["implementation::convert(int)"]
    );

    vfs.open_file(
        &caller,
        Some("int caller() { return detail::convert(1); }\n#include \"imports.hpp\"\n"),
    )
    .unwrap();
    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert!(trace.callees.is_empty());
}

#[test]
fn traces_cpp_template_type_alias_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("alias.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\nnamespace app { template <typename T> using Alias = api::Box<T>; int caller(int value) { Alias<int> box{value}; return value; } }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "app::caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );
}

#[test]
fn traces_cpp_template_braced_initializer_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("box.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\nint caller(int value) { api::Box<int> box{value}; return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box<int>::Box(int)"]
    );
}

#[test]
fn traces_cpp_template_new_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("box.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api {\ntemplate <typename T> class Box { public: Box(T value) {} };\ntemplate <> class Box<int> { public: Box(int value) {} };\n}\nint caller(int value) { auto box = new api::Box<int>(value); return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box<int>::Box(int)"]
    );
}

#[test]
fn traces_cpp_template_constructor_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("box.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { template <typename T> class Box { public: Box(T value) {} }; }\nint caller(int value) { auto box = api::Box<int>{value}; return value; }\n",
        ),
    )
    .unwrap();

    let trace = vfs
        .trace_symbol_graph(&workspace, "caller", TraceDirection::Both)
        .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Box::Box(T)"]
    );
}

#[test]
fn trace_patch_context_uses_unsaved_workspace_overrides() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let consumer_path = workspace.join("consumer.py");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &consumer_path,
        "def consume(value: int) -> int:\n    return value\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &consumer_path,
        Some(
            "from caller import orchestrate\n\n\ndef consume(value: int) -> int:\n    return orchestrate(value)\n",
        ),
    )
    .unwrap();

    let result = vfs
        .validate_patch_with_trace_context(
            &workspace,
            &caller_path,
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(result.patch.applied);
    assert_eq!(result.trace_target, result.patch.resolved_symbol_id);
    assert!(result.trace_error.is_none());
    assert_eq!(
        result
            .trace_validation
            .as_ref()
            .map(|validation| validation.allowed),
        Some(true)
    );

    let trace = result.trace.expect("trace result should be present");
    assert!(
        trace
            .callees
            .iter()
            .find(|symbol| symbol.semantic_path == "helper")
            .is_some()
    );
    assert!(
        trace
            .callers
            .iter()
            .find(|symbol| symbol.semantic_path == "consume")
            .is_some()
    );

    let consumer_snapshot = vfs.read_file(&consumer_path).unwrap();
    assert!(consumer_snapshot.dirty);
    assert!(
        consumer_snapshot
            .source
            .contains("return orchestrate(value)")
    );
    let consumer_disk = fs::read_to_string(&consumer_path).unwrap();
    assert!(consumer_disk.contains("return value"));
    assert!(!consumer_disk.contains("return orchestrate(value)"));
}

#[test]
fn trace_patch_context_rejects_unresolved_crlf_patch_bindings() {
    let workspace = temp_workspace();
    let caller_path = workspace.join("caller.py");
    let original_source = "def orchestrate(value: int) -> int:\r\n    return value + 1\r\n";

    fs::write(&caller_path, original_source).unwrap();

    let mut vfs = VirtualFileSystem::new();
    let result = vfs
        .validate_patch_with_trace_context(
            &workspace,
            &caller_path,
            "orchestrate",
            "def orchestrate(value: int) -> int:\n    return missing_helper(value)\n",
            None,
            TraceDirection::Both,
        )
        .unwrap();

    assert!(!result.patch.applied);
    assert_eq!(result.patch.validation.commit_gate.status, "rejected");
    assert_eq!(
        result.patch.validation.unresolved_identifiers,
        vec!["missing_helper"]
    );
    assert!(result.trace.is_none());
    assert!(result.trace_validation.is_none());
    assert_eq!(
        result.trace_error.as_deref(),
        Some("trace skipped because patch validation rejected the patch")
    );

    let snapshot = vfs.read_file(&caller_path).unwrap();
    assert_eq!(snapshot.source, original_source);
    assert!(!snapshot.dirty);
}

#[test]
fn trace_symbol_graph_ignores_virtual_files_in_skipped_dirs() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let venv_path = workspace.join("VENV").join("installed.py");

    fs::create_dir_all(venv_path.parent().unwrap()).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&venv_path, Some("def installed() -> int:\n    return 2\n"))
        .unwrap();

    assert!(
        vfs.trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
            .is_ok()
    );
    assert!(
        vfs.trace_symbol_graph(&workspace, "installed", TraceDirection::Both)
            .is_err()
    );
}

#[test]
fn trace_symbol_graph_ignores_virtual_files_in_sibling_workspace_prefix() {
    let dir = temp_workspace();
    let workspace = dir.join("project");
    let sibling = dir.join("project-extra");
    let helper_path = workspace.join("helper.py");
    let sibling_path = sibling.join("installed.py");

    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&sibling).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &sibling_path,
        Some("def installed() -> int:\n    return 2\n"),
    )
    .unwrap();

    assert!(
        vfs.trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
            .is_ok()
    );
    assert!(
        vfs.trace_symbol_graph(&workspace, "installed", TraceDirection::Both)
            .is_err()
    );
}

#[test]
fn virtual_workspace_overrides_skip_symlink_file_escape() {
    let dir = temp_workspace();
    let workspace = dir.join("workspace");
    let outside = dir.join("outside");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("linked.py"), "def leaked():\n    return 1\n").unwrap();

    let linked_path = workspace.join("linked.py");
    if !try_symlink_file(&outside.join("linked.py"), &linked_path) {
        let _ = fs::remove_dir_all(dir);
        return;
    }

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(&linked_path, Some("def leaked():\n    return 2\n"))
        .unwrap();

    let overrides = vfs.virtual_overrides_for_workspace(&workspace).unwrap();

    assert!(overrides.is_empty());
}

#[test]
fn commits_refresh_registered_symbol_index() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let db_path = workspace.join("symbols.db");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 2);
    assert_eq!(stats.reused_files, 0);
    assert_eq!(vfs.registered_symbol_indexes().len(), 1);

    vfs.patch_node(
        &helper_path,
        "helper",
        "def helper(value: int) -> int:\n    return branch(value)\n",
        None,
    )
    .unwrap();
    vfs.commit_file(&helper_path).unwrap();

    let trace = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "branch")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "leaf")
    );

    assert!(vfs.unregister_symbol_index(&workspace).unwrap());
    assert!(vfs.registered_symbol_indexes().is_empty());
}

#[test]
fn commits_new_file_refresh_registered_symbol_index() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let db_path = workspace.join("symbols.db");

    fs::write(
        &caller_path,
        "def orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 1);

    let initial_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(initial_trace.callees.is_empty());

    vfs.open_file(
        &helper_path,
        Some("def helper(value: int) -> int:\n    return value + 1\n"),
    )
    .unwrap();
    vfs.commit_file(&helper_path).unwrap();

    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(
        updated_trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "helper")
    );
}

#[test]
fn refreshes_registered_symbol_index_after_external_disk_change() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let db_path = workspace.join("symbols.db");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.register_symbol_index(&workspace, &db_path).unwrap();

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return branch(value)\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();

    let stats = vfs
        .refresh_registered_symbol_indexes(20_000, None, None)
        .unwrap();
    assert_eq!(stats.len(), 1);
    assert_eq!(stats[0].indexed_files, 2);
    assert_eq!(stats[0].rebuilt_files, 1);
    assert_eq!(stats[0].reused_files, 1);

    let trace = trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).unwrap();
    assert!(
        trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "branch")
    );
    assert!(
        !trace
            .callees
            .iter()
            .any(|symbol| symbol.semantic_path == "leaf")
    );
}

#[test]
fn commits_clean_deleted_file_refresh_registered_symbol_index() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let caller_path = workspace.join("caller.py");
    let db_path = workspace.join("symbols.db");

    fs::write(
        &helper_path,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
        &caller_path,
        "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
    )
    .unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 2);
    vfs.read_file(&helper_path).unwrap();

    fs::remove_file(&helper_path).unwrap();
    let committed = vfs.commit_file(&helper_path).unwrap();

    assert_eq!(committed.source, "");
    assert!(!committed.dirty);
    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_err());
    let updated_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert!(updated_trace.callees.is_empty());
}

#[test]
fn commits_skip_registered_index_refresh_for_ignored_dirs() {
    let workspace = temp_workspace();
    let helper_path = workspace.join("helper.py");
    let venv_path = workspace.join("VENV").join("installed.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(venv_path.parent().unwrap()).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 1);

    vfs.open_file(&venv_path, Some("def installed() -> int:\n    return 2\n"))
        .unwrap();
    vfs.commit_file(&venv_path).unwrap();

    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
    assert!(trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err());
}

#[test]
fn commit_skips_registered_index_refresh_for_sibling_workspace_prefix() {
    let dir = temp_workspace();
    let workspace = dir.join("project");
    let sibling = dir.join("project-extra");
    let helper_path = workspace.join("helper.py");
    let sibling_path = sibling.join("installed.py");
    let db_path = workspace.join("symbols.db");

    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&sibling).unwrap();
    fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
    assert_eq!(stats.indexed_files, 1);

    vfs.open_file(
        &sibling_path,
        Some("def installed() -> int:\n    return 2\n"),
    )
    .unwrap();
    vfs.commit_file(&sibling_path).unwrap();

    assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
    assert!(trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err());
}

fn temp_file(contents: &str) -> std::path::PathBuf {
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("arborist-vfs-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join(Path::new("buffer.py"));
    fs::write(&file, contents).unwrap();
    file
}

fn generated_edit_cases() -> [(&'static str, &'static str, &'static str, &'static str); 3] {
    [
        ("alpha", "beta", "first", "second"),
        ("é", "茅", "ß", "文"),
        ("🙂", "尾", "星", "末"),
    ]
}

fn position_at(source: &str, byte_offset: usize) -> Position {
    position_from(point_for_offset(source, byte_offset).unwrap())
}

fn temp_workspace() -> std::path::PathBuf {
    let suffix = format!(
        "{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let dir = std::env::temp_dir().join(format!("arborist-vfs-workspace-{suffix}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[cfg(unix)]
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::unix::fs::symlink(target, link).is_ok()
}

#[cfg(windows)]
fn try_symlink_file(target: &Path, link: &Path) -> bool {
    std::os::windows::fs::symlink_file(target, link).is_ok()
}
