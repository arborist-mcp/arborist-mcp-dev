use super::*;

#[test]
fn traces_cpp_local_parameter_and_pointer_member_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; using Alias = Counter; int local_caller(int value) { Alias current{}; return current.adjust(value); } int postfix_const_caller(int value) { Alias const current{}; return current.adjust(value); } int static_caller(int value) { static Alias current{}; return current.adjust(value); } int auto_caller(int value) { auto current = Alias{}; return current.adjust(value); } int auto_direct_list_caller(int value) { auto current{Alias{}}; return current.adjust(value); } int deduced_pointer_caller(int value) { auto current = new Alias{}; return current->adjust(value); } int parenthesized_deduced_pointer_caller(int value) { auto current = new Alias(); return current->adjust(value); } int default_deduced_pointer_caller(int value) { auto current = new Alias; return current->adjust(value); } int pointee_const_deduced_pointer_caller(int value) { auto current = new const Alias{}; return current->adjust(value); } int postfix_pointee_const_deduced_pointer_caller(int value) { auto current = new Alias const{}; return current->adjust(value); } int make_unique_caller(int value) { auto current = std::make_unique<Alias>(); return current->adjust(value); } int make_shared_caller(int value) { auto current = std::make_shared<Alias>(); return current->adjust(value); } int const_deduced_pointer_caller(int value) { const auto current = new Alias{}; return current->adjust(value); } int auto_pointer_caller(int value) { auto* current = new Alias{}; return current->adjust(value); } int const_auto_pointer_caller(int value) { const auto* current = new Alias{}; return current->adjust(value); } int const_auto_caller(int value) { const auto current = Alias{}; return current.adjust(value); } int parameter_caller(const Alias& current, int value) { return current.adjust(value); } int postfix_const_parameter_caller(Alias const& current, int value) { return current.adjust(value); } int rvalue_reference_caller(Alias&& current, int value) { return current.adjust(value); } int moved_rvalue_reference_caller(Alias&& current, int value) { return std::move(current).adjust(value); } int pointer_caller(Alias* current, int value) { return current->adjust(value); } int const_pointer_caller(Alias* const current, int value) { return current->adjust(value); } int postfix_const_pointer_caller(Alias const* current, int value) { return current->adjust(value); } int pointer_reference_caller(Alias* const& current, int value) { return current->adjust(value); } int dereference_caller(Alias* current, int value) { return (*current).adjust(value); } int range_caller() { for (Alias current : values) { return current.adjust(1); } return 0; } int moved_caller(Alias& current, int value) { return std::move(current).adjust(value); } }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::local_caller", "api::Counter::adjust(int) &"),
        (
            "api::postfix_const_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::static_caller", "api::Counter::adjust(int) &"),
        ("api::auto_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_direct_list_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::deduced_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::parenthesized_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::default_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::pointee_const_deduced_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::postfix_pointee_const_deduced_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::make_unique_caller", "api::Counter::adjust(int) &"),
        ("api::make_shared_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_deduced_pointer_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::auto_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_auto_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_auto_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::parameter_caller", "api::Counter::adjust(int) const &"),
        (
            "api::postfix_const_parameter_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::rvalue_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_rvalue_reference_caller",
            "api::Counter::adjust(int) &&",
        ),
        ("api::pointer_caller", "api::Counter::adjust(int) &"),
        ("api::const_pointer_caller", "api::Counter::adjust(int) &"),
        (
            "api::postfix_const_pointer_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::pointer_reference_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::dereference_caller", "api::Counter::adjust(int) &"),
        ("api::range_caller", "api::Counter::adjust(int) &"),
        ("api::moved_caller", "api::Counter::adjust(int) &&"),
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
fn traces_cpp_standard_smart_pointer_member_calls_from_unsaved_virtual_changes() {
    let workspace = temp_workspace();
    let source = workspace.join("counter.cpp");
    fs::write(&source, "int caller(int value) { return value; }\n").unwrap();

    let mut vfs = VirtualFileSystem::new();
    vfs.open_file(
        &source,
        Some(
            "namespace api { class Counter { public: int adjust(int value) & { return value; } int adjust(int value) const & { return value + 1; } int adjust(int value) && { return value + 2; } }; struct Deleter {}; using Alias = Counter; int unique_caller(int value) { std::unique_ptr<Alias> current; return current->adjust(value); } int unique_get_caller(int value) { std::unique_ptr<Alias> current; return current.get()->adjust(value); } int auto_unique_caller(int value) { auto current = std::unique_ptr<Alias>{}; return current->adjust(value); } int auto_reference_alias_caller(int value) { Alias target{}; auto& current = target; return current.adjust(value); } int auto_const_reference_alias_caller(int value) { Alias target{}; const auto& current = target; return current.adjust(value); } int auto_forwarding_reference_alias_caller(int value) { const Alias target{}; auto&& current = target; return current.adjust(value); } int copy_list_caller(int value) { auto current = {Alias{}}; return current.adjust(value); } int moved_unique_dereference_caller(int value) { std::unique_ptr<Alias> current; return (*std::move(current)).adjust(value); } int reference_wrapper_get_caller(int value) { Alias target{}; std::reference_wrapper<Alias> current(target); return current.get().adjust(value); } int const_reference_wrapper_get_caller(int value) { const Alias target{}; std::reference_wrapper<const Alias> current(target); return current.get().adjust(value); } int auto_parenthesized_reference_wrapper_caller(int value) { Alias target{}; auto current = (std::reference_wrapper<Alias>(target)); return current.get().adjust(value); } int ref_factory_caller(int value) { Alias target{}; return std::ref(target).get().adjust(value); } int parenthesized_ref_factory_caller(int value) { Alias target{}; return (std::ref(target)).get().adjust(value); } int cref_factory_caller(int value) { Alias target{}; return std::cref(target).get().adjust(value); } int ref_as_const_factory_caller(int value) { Alias target{}; return std::ref(std::as_const(target)).get().adjust(value); } int auto_ref_factory_caller(int value) { Alias target{}; auto current = std::ref(target); return current.get().adjust(value); } int auto_cref_factory_caller(int value) { Alias target{}; auto current = std::cref(target); return current.get().adjust(value); } int auto_ref_as_const_factory_caller(int value) { Alias target{}; auto current = std::ref(std::as_const(target)); return current.get().adjust(value); } int optional_arrow_caller(int value) { std::optional<Alias> current; return current->adjust(value); } int auto_optional_arrow_caller(int value) { auto current = std::optional<Alias>{}; return current->adjust(value); } int nested_optional_unique_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return (*current)->adjust(value); } int nested_optional_unique_value_arrow_caller(int value) { std::optional<std::unique_ptr<Alias>> current; return current.value()->adjust(value); } int moved_optional_arrow_caller(int value) { std::optional<Alias> current; return std::move(current)->adjust(value); } int const_optional_arrow_caller(int value) { std::optional<Alias> current; return std::as_const(current)->adjust(value); } int optional_dereference_caller(int value) { std::optional<Alias> current; return (*current).adjust(value); } int moved_optional_value_caller(int value) { std::optional<Alias> current; return std::move(current).value().adjust(value); } int moved_optional_dereference_caller(int value) { std::optional<Alias> current; return (*std::move(current)).adjust(value); } int const_optional_value_caller(int value) { const std::optional<Alias> current{}; return current.value().adjust(value); } int const_optional_dereference_caller(int value) { const std::optional<Alias> current{}; return (*current).adjust(value); } int addressof_caller(int value) { Alias current{}; return std::addressof(current)->adjust(value); } int const_addressof_caller(int value) { const Alias current{}; return std::addressof(current)->adjust(value); } int auto_addressof_caller(int value) { Alias current{}; auto pointer = std::addressof(current); return pointer->adjust(value); } int auto_const_addressof_caller(int value) { const Alias current{}; auto pointer = std::addressof(current); return pointer->adjust(value); } int auto_native_addressof_caller(int value) { Alias current{}; auto pointer = &current; return pointer->adjust(value); } int auto_const_native_addressof_caller(int value) { const Alias current{}; auto pointer = &current; return pointer->adjust(value); } int custom_unique_caller(int value) { std::unique_ptr<Alias, Deleter> current; return current->adjust(value); } int shared_caller(int value) { std::shared_ptr<Alias> current; return current->adjust(value); } int const_unique_caller(int value) { std::unique_ptr<const Alias> current; return current->adjust(value); } int const_unique_get_caller(int value) { std::unique_ptr<const Alias> current; return current.get()->adjust(value); } }\n",
        ),
    )
    .unwrap();

    for (caller, expected_callee) in [
        ("api::unique_caller", "api::Counter::adjust(int) &"),
        ("api::unique_get_caller", "api::Counter::adjust(int) &"),
        ("api::auto_unique_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_reference_alias_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_forwarding_reference_alias_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::moved_unique_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::reference_wrapper_get_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_reference_wrapper_get_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_parenthesized_reference_wrapper_caller",
            "api::Counter::adjust(int) &",
        ),
        ("api::ref_factory_caller", "api::Counter::adjust(int) &"),
        (
            "api::parenthesized_ref_factory_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::ref_as_const_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_ref_factory_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_cref_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_ref_as_const_factory_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::optional_arrow_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_unique_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::nested_optional_unique_value_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_optional_arrow_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::const_optional_arrow_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::optional_dereference_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::moved_optional_value_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::moved_optional_dereference_caller",
            "api::Counter::adjust(int) &&",
        ),
        (
            "api::const_optional_value_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_optional_dereference_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::addressof_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_addressof_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::auto_addressof_caller", "api::Counter::adjust(int) &"),
        (
            "api::auto_const_addressof_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::auto_native_addressof_caller",
            "api::Counter::adjust(int) &",
        ),
        (
            "api::auto_const_native_addressof_caller",
            "api::Counter::adjust(int) const &",
        ),
        ("api::custom_unique_caller", "api::Counter::adjust(int) &"),
        ("api::shared_caller", "api::Counter::adjust(int) &"),
        (
            "api::const_unique_caller",
            "api::Counter::adjust(int) const &",
        ),
        (
            "api::const_unique_get_caller",
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
    assert!(
        vfs.trace_symbol_graph(&workspace, "api::copy_list_caller", TraceDirection::Both)
            .unwrap()
            .callees
            .is_empty()
    );
}
