use std::path::{Component, PathBuf};

use tree_sitter::Point;

use crate::language::{
    normalize_absolute_path, offset_for_position, point_for_offset, position_from,
};

fn generated_utf8_sources() -> Vec<String> {
    let prefixes = ["", "a", "é", "茅", "🙂", "line\n", "多字节\né"];
    let middles = ["", "b", "\n", "xy", "🙂é", "\n\n", "尾\n"];
    let suffixes = ["", "end", "\nend", "z", "完成🙂", "\n茅"];

    let mut sources = Vec::new();
    for prefix in prefixes {
        for middle in middles {
            for suffix in suffixes {
                sources.push(format!("{prefix}{middle}{suffix}"));
            }
        }
    }
    sources.sort();
    sources.dedup();
    sources
}

fn generated_relative_paths() -> Vec<PathBuf> {
    let heads = ["alpha", "beta", "gamma"];
    let tails = ["file.py", "nested", "delta.txt"];

    let mut paths = Vec::new();
    for head in heads {
        paths.push(PathBuf::from(head));
        paths.push(PathBuf::from(".").join(head));
        paths.push(PathBuf::from(head).join("."));

        for tail in tails {
            paths.push(PathBuf::from(head).join(tail));
            paths.push(PathBuf::from(".").join(head).join(tail));
            paths.push(PathBuf::from(head).join("..").join(head).join(tail));
            paths.push(
                PathBuf::from(head)
                    .join(tail)
                    .join("..")
                    .join("normalized")
                    .join("leaf.py"),
            );
        }
    }

    paths.sort();
    paths.dedup();
    paths
}

fn expected_point(source: &str, byte_offset: usize) -> Point {
    let prefix = &source.as_bytes()[..byte_offset];
    let row = prefix.iter().filter(|byte| **byte == b'\n').count();
    let column = prefix
        .iter()
        .rposition(|byte| *byte == b'\n')
        .map_or(prefix.len(), |newline| prefix.len() - newline - 1);

    Point { row, column }
}

#[test]
fn normalize_absolute_path_is_idempotent_for_generated_relative_paths() {
    for path in generated_relative_paths() {
        let normalized = normalize_absolute_path(&path).unwrap();

        assert!(
            normalized.is_absolute(),
            "expected absolute path for {path:?}"
        );
        assert_eq!(
            normalize_absolute_path(&std::env::current_dir().unwrap().join(&path)).unwrap(),
            normalized
        );
        assert_eq!(normalize_absolute_path(&normalized).unwrap(), normalized);
        assert!(
            !normalized
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        );
    }
}

#[test]
fn byte_position_helpers_round_trip_generated_utf8_boundaries() {
    for source in generated_utf8_sources() {
        for offset in source
            .char_indices()
            .map(|(index, _)| index)
            .chain(std::iter::once(source.len()))
        {
            let point = point_for_offset(&source, offset).unwrap();
            let position = position_from(point);

            assert_eq!(point, expected_point(&source, offset));
            assert_eq!(
                offset_for_position(&source, &position).unwrap(),
                offset,
                "expected round-trip for source {source:?} at offset {offset}"
            );
        }
    }
}

#[test]
fn byte_position_helpers_reject_generated_non_boundary_offsets() {
    for source in generated_utf8_sources() {
        for offset in 0..source.len() {
            if source.is_char_boundary(offset) {
                continue;
            }

            let point_error = point_for_offset(&source, offset)
                .expect_err("non-boundary offsets should be rejected");
            assert!(
                point_error
                    .to_string()
                    .contains("does not align to a UTF-8 character boundary"),
                "unexpected point error for source {source:?} at offset {offset}: {point_error}"
            );

            let position_error =
                offset_for_position(&source, &position_from(expected_point(&source, offset)))
                    .expect_err("positions inside a UTF-8 character should be rejected");
            assert!(
                position_error
                    .to_string()
                    .contains("does not align to a UTF-8 character boundary"),
                "unexpected position error for source {source:?} at offset {offset}: {position_error}"
            );
        }
    }
}
