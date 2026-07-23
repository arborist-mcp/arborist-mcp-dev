mod bindings;

mod call_arities;
mod member_call_names;
mod name_collection;
mod receivers;
mod std_get;
mod type_qualifiers;
mod types;

pub(super) use bindings::collect_cpp_local_bindings;
pub(crate) use call_arities::{collect_c_call_arities, collect_cpp_call_arities};
pub(super) use name_collection::collect_c_local_definitions;
pub(crate) use name_collection::{collect_c_graph_references, collect_c_references};
use receivers::*;
pub(super) use receivers::{
    cpp_local_member_receiver_type, cpp_standard_sequence_at_receiver, cpp_subscript_receiver,
    cpp_temporary_type_from_expression, cpp_this_receiver_from_expression,
    cpp_visible_local_binding,
};

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::Path;

    use crate::language::parse_document;

    use super::super::cpp_types::cpp_type_is_top_level_const;
    use super::{
        collect_c_graph_references, collect_cpp_call_arities, cpp_this_receiver_from_expression,
    };
    use crate::symbol_index_model::{
        CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
        CPP_TEMPORARY_MEMBER_CALL_SEPARATOR,
    };

    #[test]
    fn collects_only_object_braced_initializers() {
        let source = "namespace api { class Counter { public: Counter(int value) {} }; }\nint caller(api::Counter* existing, api::Counter& current) { api::Counter counter{1}; api::Counter* pointer{existing}; api::Counter& reference{current}; return 0; }\n";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([("api::Counter".to_string(), BTreeSet::from([1]))])
        );
    }

    #[test]
    fn collects_this_and_typed_pointer_member_call_arities() {
        let source = "class Counter { int adjust(int value) { return value; } int caller(Counter* other) { return this->adjust(1) + (*this).adjust(1, 2) + other->adjust(1, 2, 3) + (*other).adjust(1, 2, 3, 4); } };";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([
                ("adjust".to_string(), BTreeSet::from([1, 2])),
                (
                    format!(
                        "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
                    ),
                    BTreeSet::from([3, 4]),
                ),
            ])
        );
    }

    #[test]
    fn ignores_parameters_of_local_function_prototypes() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller(int value) { int declared(Counter current); return current.adjust(value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert!(!arities.keys().any(|name| {
            name.contains("Counter::adjust")
                && name.starts_with(CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX)
        }));
    }

    #[test]
    fn collects_catch_parameter_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller(int value) { try { throw value; } catch (Counter current) { return current.adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn collects_this_member_template_call_arities() {
        let source = "class Counter { template <typename T> T adjust(T value) { return value; } int caller(int value) { return this->template adjust<int>(value); } };";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([("adjust<int>".to_string(), BTreeSet::from([1]))])
        );
    }

    #[test]
    fn collects_temporary_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) && { return value; } }; int caller(int value) { return Counter{}.adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities,
            BTreeMap::from([
                ("Counter".to_string(), BTreeSet::from([0])),
                (
                    format!(
                        "{CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
                    ),
                    BTreeSet::from([1]),
                ),
            ])
        );
    }

    #[test]
    fn collects_local_variable_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } int adjust(int value) && { return value; } }; int caller(int value) { Counter current{}; const Counter locked{}; return current.adjust(value) + locked.adjust(value) + std::move(current).adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn collects_wrapped_pointer_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(Counter* current, int value) { return std::as_const(current)->adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert!(!arities.contains_key(&format!(
            "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
        )));
    }

    #[test]
    fn collects_auto_reference_factory_member_call_arities() {
        let source = "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; auto mutable_ref = std::ref(target); auto parenthesized_ref = (std::ref(target)); auto const_ref = std::cref(target); auto as_const_ref = std::ref(std::as_const(target)); return mutable_ref.get().adjust(value) + parenthesized_ref.get().adjust(value) + const_ref.get().adjust(value) + as_const_ref.get().adjust(value) + (std::cref(target)).get().adjust(value) + std::ref(std::move(target)).get().adjust(value); } }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn scopes_auto_reference_factories_to_visible_bindings() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; { const Counter target{}; auto current = std::ref(target); current.get().adjust(value); } auto current = std::ref(target); return current.get().adjust(value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn collects_auto_addressof_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; const Counter locked{}; auto mutable_pointer = std::addressof(target); auto const_pointer = std::addressof(locked); auto native_pointer = &target; auto native_const_pointer = &locked; return mutable_pointer->adjust(value) + const_pointer->adjust(value) + native_pointer->adjust(value, value) + native_const_pointer->adjust(value, value, value) + std::addressof(std::move(target))->adjust(value, value, value, value) + std::addressof(std::as_const(target))->adjust(value, value, value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 2, 4]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 3, 5]))
        );
    }

    #[test]
    fn collects_auto_reference_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; const Counter locked{}; Counter* pointer = &target; const Counter* const_pointer = &locked; std::reference_wrapper<Counter> wrapper(target); std::reference_wrapper<const Counter> const_wrapper(locked); auto& mutable_alias = target; const auto& const_alias = target; auto const& postfix_const_alias = target; auto&& forwarding_alias = locked; auto&& moved_alias = std::move(target); auto&& as_const_alias = std::as_const(target); auto&& forwarded_alias = std::forward<Counter&&>(target); auto&& const_forwarded_alias = std::forward<const Counter&&>(target); auto&& cast_alias = static_cast<Counter&&>(target); auto&& const_cast_alias = static_cast<const Counter&&>(target); auto& pointer_alias = *pointer; auto&& const_pointer_alias = *const_pointer; auto& address_alias = *std::addressof(std::as_const(target)); auto& wrapper_alias = wrapper.get(); auto&& const_wrapper_alias = const_wrapper.get(); auto&& ref_alias = std::ref(target).get(); auto&& cref_alias = std::cref(target).get(); return mutable_alias.adjust(value) + const_alias.adjust(value, value) + postfix_const_alias.adjust(value, value, value) + forwarding_alias.adjust(value, value, value, value) + moved_alias.adjust(value, value, value, value, value) + as_const_alias.adjust(value, value, value, value, value, value) + forwarded_alias.adjust(value, value, value, value, value, value, value) + const_forwarded_alias.adjust(value, value, value, value, value, value, value, value) + cast_alias.adjust(value, value, value, value, value, value, value, value, value) + const_cast_alias.adjust(value, value, value, value, value, value, value, value, value, value) + pointer_alias.adjust(value, value, value, value, value, value, value, value, value, value, value) + const_pointer_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value) + address_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value) + wrapper_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value, value) + const_wrapper_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value, value, value) + ref_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value) + cref_alias.adjust(value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 5, 7, 9, 11, 14, 16]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2, 3, 4, 6, 8, 10, 12, 13, 15, 17]))
        );
    }

    #[test]
    fn collects_decltype_auto_reference_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { Counter target{}; const Counter locked{}; Counter* pointer = &target; std::optional<Counter> optional; std::reference_wrapper<Counter> wrapper(target); decltype(auto) copied_value = target; decltype(auto) copied_const_value = locked; decltype(auto) parenthesized_alias = (target); decltype(auto) const_alias = (locked); decltype(auto) moved_alias = std::move(target); decltype(auto) pointer_alias = *pointer; decltype(auto) optional_alias = optional.value(); decltype(auto) wrapper_alias = wrapper.get(); return copied_value.adjust(value) + copied_const_value.adjust(value, value) + parenthesized_alias.adjust(value, value, value) + const_alias.adjust(value, value, value, value) + moved_alias.adjust(value, value, value, value, value) + pointer_alias.adjust(value, value, value, value, value, value) + optional_alias.adjust(value, value, value, value, value, value, value) + wrapper_alias.adjust(value, value, value, value, value, value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 3, 5, 6, 7, 8]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2, 4]))
        );
    }

    #[test]
    fn collects_auto_optional_value_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { std::optional<Counter> current; const std::optional<Counter> locked{}; auto& value_alias = current.value(); auto&& const_value_alias = locked.value(); auto&& moved_value_alias = std::move(current).value(); return value_alias.adjust(value) + const_value_alias.adjust(value, value) + moved_value_alias.adjust(value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 3]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2]))
        );
    }

    #[test]
    fn collects_auto_optional_dereference_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { std::optional<Counter> current; const std::optional<Counter> locked{}; auto& value_alias = *current; auto&& const_value_alias = *locked; auto&& moved_value_alias = *std::move(current); auto&& moved_alias = std::move(*current); auto&& as_const_alias = std::as_const(*current); auto&& forwarded_alias = std::forward<Counter&&>(*current); auto&& const_forwarded_alias = std::forward<const Counter&&>(*current); return value_alias.adjust(value) + const_value_alias.adjust(value, value) + moved_value_alias.adjust(value, value, value) + moved_alias.adjust(value, value, value, value) + as_const_alias.adjust(value, value, value, value, value) + forwarded_alias.adjust(value, value, value, value, value, value) + const_forwarded_alias.adjust(value, value, value, value, value, value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1, 3, 4, 6]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2, 5, 7]))
        );
    }

    #[test]
    fn collects_auto_smart_pointer_dereference_alias_member_call_arities() {
        let source = "class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value; } }; int caller(int value) { std::unique_ptr<Counter> current; std::shared_ptr<const Counter> locked; auto& value_alias = *current; auto&& const_value_alias = *locked; return value_alias.adjust(value) + const_value_alias.adjust(value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
        assert_eq!(
            arities.get(&format!(
                "{CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([2]))
        );
    }

    #[test]
    fn distinguishes_auto_direct_and_copy_list_initializers() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller(int value) { auto direct{Counter{}}; auto copied = {Counter{}}; return direct.adjust(value) + copied.adjust(value, value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn scopes_range_for_bindings_to_the_loop() {
        let source = "class Counter { public: int adjust(int value) & { return value; } }; int caller() { for (Counter current : values) { current.adjust(1); } return current.adjust(1, 2); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut arities = BTreeMap::new();

        collect_cpp_call_arities(document.tree.root_node(), source, &mut arities).unwrap();

        assert_eq!(
            arities.get(&format!(
                "{CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
            )),
            Some(&BTreeSet::from([1]))
        );
    }

    #[test]
    fn identifies_only_top_level_cpp_const_qualifiers() {
        assert!(cpp_type_is_top_level_const("const Counter&&"));
        assert!(cpp_type_is_top_level_const("Counter const &"));
        assert!(!cpp_type_is_top_level_const("constCounter&&"));
        assert!(!cpp_type_is_top_level_const("Wrapper<const Counter>&&"));
    }

    #[test]
    fn rejects_non_this_and_malformed_cpp_member_receivers() {
        assert!(cpp_this_receiver_from_expression("std::move(other)").is_none());
        assert!(cpp_this_receiver_from_expression("std::forward<Counter&>(other)").is_none());
        assert!(cpp_this_receiver_from_expression("static_cast<Counter&&>(*this").is_none());
    }

    #[test]
    fn skips_nested_cpp_type_template_arguments_when_collecting_references() {
        let source = "class Value {}; class Counter { public: int adjust(int) &; }; int caller(std::expected<Value, std::expected<Value, Counter>> current, int value) { auto error = current.error(); return error.error().adjust(value); }";
        let document = parse_document(Path::new("sample.cpp"), source).unwrap();
        let mut references = BTreeSet::new();

        collect_c_graph_references(document.tree.root_node(), source, &mut references).unwrap();

        assert!(!references.contains("Value"));
    }
}
