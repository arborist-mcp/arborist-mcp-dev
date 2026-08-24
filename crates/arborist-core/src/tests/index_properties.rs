use std::sync::LazyLock;

use proptest::prelude::*;

use crate::symbol_index_model::symbol_base_name_ref;

/// Characters valid in identifier-like segments (no `::` or `.` separators,
/// no whitespace or control characters) so generated semantic paths exercise
/// real-world nesting without producing ambiguous separators.
static SEGMENT_CHARACTERS: LazyLock<Vec<char>> =
    LazyLock::new(|| vec!['a', 'z', '0', '9', '_', 'A', 'Z']);

fn segment_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(&*SEGMENT_CHARACTERS), 1..=12)
        .prop_map(String::from_iter)
}

fn path_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(segment_strategy(), 1..=5).prop_map(|segments| segments.join("::"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn symbol_base_name_extracts_the_last_segment(
        prefix_segments in prop::collection::vec(segment_strategy(), 0..=3),
        base_name in segment_strategy(),
    ) {
        let mut segments = prefix_segments;
        let base = base_name.clone();
        segments.push(base_name.clone());

        let semantic_path = segments.join("::");
        prop_assert_eq!(symbol_base_name_ref(&semantic_path), base.as_str());
    }

    #[test]
    fn symbol_base_name_is_idempotent(
        semantic_path in path_strategy(),
    ) {
        let base = symbol_base_name_ref(&semantic_path);
        prop_assert_eq!(symbol_base_name_ref(base), base);
    }

    #[test]
    fn symbol_base_name_is_always_a_nonempty_substring_of_input(
        semantic_path in path_strategy(),
    ) {
        let base = symbol_base_name_ref(&semantic_path);
        prop_assert!(!base.is_empty(), "base name must not be empty for {semantic_path:?}");
        prop_assert!(
            semantic_path.ends_with(base),
            "base name {base:?} must be a suffix of the input {semantic_path:?}"
        );
    }

    #[test]
    fn symbol_base_name_strips_method_suffix_after_dot(
        namespace_segments in prop::collection::vec(segment_strategy(), 1..=3),
        type_name in segment_strategy(),
        method_name in segment_strategy(),
    ) {
        let mut parts = namespace_segments;
        parts.push(type_name.clone());
        let joined = parts.join("::");
        let semantic_path = format!("{joined}.{}", method_name);

        // The dot-suffix form extracts the method name, not the type.
        prop_assert_eq!(symbol_base_name_ref(&semantic_path), method_name.as_str());
    }
}

#[cfg(test)]
mod symbol_kind_rank_properties {
    use crate::symbol_index_model::symbol_kind_rank;

    /// Known node kinds and their expected rank tiers.
    const RANK_3_KINDS: &[&str] = &[
        "function_definition",
        "function_item",
        "function_signature_item",
        "class_definition",
    ];
    const RANK_2_KINDS: &[&str] = &[
        "alias_declaration",
        "class_specifier",
        "concept_definition",
        "enum_specifier",
        "enumerator",
        "namespace_alias_definition",
        "struct_specifier",
        "template_instantiation",
        "type_definition",
        "union_specifier",
        "using_declaration",
        "const_item",
        "enum_item",
        "mod_item",
        "static_item",
        "struct_item",
        "trait_item",
        "type_item",
    ];
    const RANK_1_KINDS: &[&str] = &["declaration", "field_declaration"];

    #[test]
    fn known_kinds_have_the_expected_rank_tiers() {
        for kind in RANK_3_KINDS {
            assert_eq!(symbol_kind_rank(kind), 3, "{kind} should have rank 3");
        }
        for kind in RANK_2_KINDS {
            assert_eq!(symbol_kind_rank(kind), 2, "{kind} should have rank 2");
        }
        for kind in RANK_1_KINDS {
            assert_eq!(symbol_kind_rank(kind), 1, "{kind} should have rank 1");
        }
    }

    #[test]
    fn unknown_kinds_have_rank_zero() {
        for kind in ["identifier", "expression_statement", "", "unknown_node"] {
            assert_eq!(
                symbol_kind_rank(kind),
                0,
                "{kind:?} should default to rank 0"
            );
        }
    }

    #[test]
    fn function_and_class_kinds_outrank_type_and_field_kinds() {
        // The core ordering invariant used by query disambiguation.
        for func_or_class in RANK_3_KINDS {
            for type_kind in RANK_2_KINDS {
                assert!(
                    symbol_kind_rank(func_or_class) > symbol_kind_rank(type_kind),
                    "{func_or_class} should outrank {type_kind}"
                );
            }
            for field_kind in RANK_1_KINDS {
                assert!(
                    symbol_kind_rank(func_or_class) > symbol_kind_rank(field_kind),
                    "{func_or_class} should outrank {field_kind}"
                );
            }
        }
        for type_kind in RANK_2_KINDS {
            for field_kind in RANK_1_KINDS {
                assert!(
                    symbol_kind_rank(type_kind) > symbol_kind_rank(field_kind),
                    "{type_kind} should outrank {field_kind}"
                );
            }
        }
    }

    #[test]
    fn ranks_are_always_bounded() {
        let all_known = RANK_3_KINDS
            .iter()
            .chain(RANK_2_KINDS)
            .chain(RANK_1_KINDS)
            .chain(["identifier"].iter());
        for kind in all_known {
            let rank = symbol_kind_rank(kind);
            assert!(rank <= 3, "{kind} rank {rank} exceeds maximum of 3");
        }
    }
}
