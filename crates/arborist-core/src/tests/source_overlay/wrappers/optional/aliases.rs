use super::*;

#[test]
fn traces_cpp_forwarded_optional_base_alias_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");

    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();

    let source = "namespace api {\nclass Base { public: int adjust(int value) & { return value; } };\nclass Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } };\nint caller(int value) { std::optional<Derived> current; auto&& alias = std::forward<Base&&>(*current); return alias.adjust(value); }\n}\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        source,
        "api::caller",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"],
    );
}

#[test]
fn traces_cpp_cast_optional_base_alias_from_unsaved_source_overlay() {
    let dir = temporary_dir();
    let source_path = dir.join("counter.cpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "namespace api { int caller(int value) { return value; } }\n",
    )
    .unwrap();
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let source = "namespace api { class Base { public: int adjust(int value) & { return value; } }; class Derived : public Base { public: int adjust(int value, int extra) & { return value + extra; } }; int caller(int value) { std::optional<Derived> current; auto&& alias = static_cast<Base&&>(*current); return alias.adjust(value); } }\n";
    let trace = trace_symbol_graph_from_index_with_source(
        &db_path,
        &source_path,
        source,
        "api::caller",
        TraceDirection::Both,
    )
    .unwrap();
    assert_eq!(
        trace
            .callees
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<Vec<_>>(),
        vec!["api::Base::adjust(int) &"]
    );
}
