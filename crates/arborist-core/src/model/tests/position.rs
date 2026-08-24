use super::*;

#[test]
fn position_rejects_unknown_fields() {
    let error = serde_json::from_str::<Position>(r#"{"row":0,"column":0,"character":0}"#)
        .expect_err("positions should reject unknown fields");

    assert!(error.to_string().contains("unknown field `character`"));
}

#[test]
fn position_edit_rejects_unknown_fields() {
    let error = serde_json::from_str::<PositionEdit>(
        r#"{"start":{"row":0,"column":0},"end":{"row":0,"column":0},"new_text":"x","newText":"x"}"#,
    )
    .expect_err("position edits should reject unknown fields");

    assert!(error.to_string().contains("unknown field `newText`"));
}

#[test]
fn workspace_edit_preview_rejects_duplicate_files() {
    let result = WorkspaceEditPreviewResult {
        changed: false,
        files: vec![
            WorkspaceEditPreviewFile {
                file: "sample.py".to_string(),
                source: "value = 1\n".to_string(),
                unified_diff: String::new(),
                changed: false,
                validation: PatchValidationReport::default(),
            },
            WorkspaceEditPreviewFile {
                file: "sample.py".to_string(),
                source: "value = 1\n".to_string(),
                unified_diff: String::new(),
                changed: false,
                validation: PatchValidationReport::default(),
            },
        ],
    };

    let error = result
        .validate_public_output()
        .expect_err("workspace previews must not repeat files");

    assert!(error.to_string().contains("duplicate preview files"));
}

#[cfg(test)]
mod point_is_after_properties {
    use super::*;
    use proptest::prelude::*;

    fn position_strategy() -> impl Strategy<Value = Position> {
        (0usize..8, 0usize..32).prop_map(|(row, column)| Position { row, column })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        #[test]
        fn point_is_after_is_exactly_lexicographic_ordering(
            a in position_strategy(),
            b in position_strategy(),
        ) {
            let expected = a.row > b.row || (a.row == b.row && a.column > b.column);
            prop_assert_eq!(point_is_after(&a, &b), expected);
        }

        #[test]
        fn point_is_after_is_antisymmetric(
            a in position_strategy(),
            b in position_strategy(),
        ) {
            // At most one of is_after(a,b) and is_after(b,a) can be true;
            // both false means equal positions.
            prop_assert!(
                !(point_is_after(&a, &b) && point_is_after(&b, &a)),
                "positions cannot be after each other in both directions"
            );
        }

        #[test]
        fn point_is_after_transitivity(
            base_row in 0usize..4,
            base_col in 0usize..16,
            d1_row in 0usize..3,
            d1_col in 0usize..16,
            d2_row in 0usize..3,
            d2_col in 0usize..16,
        ) {
            // Construct a strictly increasing chain: a < b <= c.
            let a = Position { row: base_row, column: base_col };
            let b = Position { row: base_row + d1_row, column: base_col + d1_col };
            let c = Position { row: b.row + d2_row, column: b.column + d2_col };

            prop_assume!(point_is_after(&b, &a));
            prop_assume!(point_is_after(&c, &b));

            prop_assert!(
                point_is_after(&c, &a),
                "if b>a and c>b then c>a must hold for {a:?} < {b:?} < {c:?}"
            );
        }

        #[test]
        fn point_is_after_agrees_with_derived_total_order(
            a in position_strategy(),
            b in position_strategy(),
            c in position_strategy(),
        ) {
            // Verify pairwise ordering matches lexicographic comparison on
            // (row, column) tuples for all three pairs.
            let to_tuple = |p: &Position| (p.row, p.column);

            let points = [&a, &b, &c];
            for i in 0..points.len() {
                for j in 0..points.len() {
                    let expected = to_tuple(points[i]) > to_tuple(points[j]);
                    prop_assert_eq!(
                        point_is_after(points[i], points[j]),
                        expected,
                        "point_is_after mismatch between {:?} and {:?}",
                        points[i], points[j]
                    );
                }
            }
        }
    }
}
