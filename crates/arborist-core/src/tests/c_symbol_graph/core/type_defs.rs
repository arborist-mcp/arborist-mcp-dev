use super::*;

#[test]
fn traces_cpp_concept_definitions() {
    let dir = temporary_dir();
    let source = dir.join("concepts.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T>\nconcept Incrementable = requires(T value) { value + 1; };\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Incrementable")
    );

    let trace = trace_symbol_graph(&dir, "api::Incrementable", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Incrementable", TraceDirection::Both)
            .unwrap();
    assert_eq!(
        persisted_trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_cpp_class_definitions() {
    let dir = temporary_dir();
    let source = dir.join("config.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\ntemplate <typename T>\nclass Config {\npublic:\n    class State {};\n    T value(T input) { return input; }\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let config = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Config")
        .expect("class definition should be indexed");
    assert_eq!(config.scope_path.as_deref(), Some("api"));
    let state = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Config::State")
        .expect("nested class definition should be indexed");
    assert_eq!(state.scope_path.as_deref(), Some("api::Config"));

    let trace = trace_symbol_graph(&dir, "api::Config", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Config", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.scope_path.as_deref(), Some("api"));
}

#[test]
fn traces_named_c_struct_and_union_definitions() {
    let dir = temporary_dir();
    let source = dir.join("protocol.c");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "struct Packet { int id; };\nunion Payload { int count; float ratio; };\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    let packet = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "Packet")
        .expect("named C struct definition should be indexed");
    assert_eq!(packet.node_kind, "struct_specifier");
    assert_eq!(packet.scope_path, None);
    let payload = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "Payload")
        .expect("named C union definition should be indexed");
    assert_eq!(payload.node_kind, "union_specifier");
    assert_eq!(payload.scope_path, None);

    let trace = trace_symbol_graph(&dir, "Packet", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Payload", TraceDirection::Both).unwrap();
    assert_eq!(
        persisted_trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_c_enum_members() {
    let dir = temporary_dir();
    let source = dir.join("status.c");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "enum Status { STATUS_READY = 1, STATUS_FAILED = 2 };\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "STATUS_READY")
    );

    let trace = trace_symbol_graph(&dir, "STATUS_FAILED", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.node_kind, "enumerator");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "STATUS_READY", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.node_kind, "enumerator");
}

#[test]
fn traces_cpp_enum_definitions() {
    let dir = temporary_dir();
    let source = dir.join("status.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nenum class Status : unsigned char { idle, busy };\n\nclass Task {\npublic:\n    enum class State { queued, running };\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    assert!(
        skeleton
            .available_symbols
            .iter()
            .any(|symbol| symbol.semantic_path == "api::Status")
    );
    let state = skeleton
        .available_symbols
        .iter()
        .find(|symbol| symbol.semantic_path == "api::Task::State")
        .expect("nested enum definition should be indexed");
    assert_eq!(state.scope_path.as_deref(), Some("api::Task"));

    let trace = trace_symbol_graph(&dir, "api::Status", TraceDirection::Both).unwrap();
    assert_eq!(
        trace.symbol.file_path,
        source.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Status", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.scope_path.as_deref(), Some("api"));
}

#[test]
fn traces_cpp_enum_members() {
    let dir = temporary_dir();
    let source = dir.join("status.hpp");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source,
        "namespace api {\nenum class Status : unsigned char { idle = 0, busy };\nenum Legacy { pending, complete };\n\nclass Task {\npublic:\n    enum class State { queued, running };\n    enum Mode { paused, active };\n};\n}\n",
    )
    .unwrap();

    let source_text = fs::read_to_string(&source).unwrap();
    let skeleton = get_semantic_skeleton(&source, &source_text, 1, &[]).unwrap();
    for expected_path in [
        "api::Status::idle",
        "api::Status::busy",
        "api::pending",
        "api::Task::State::queued",
        "api::Task::paused",
    ] {
        assert!(
            skeleton
                .available_symbols
                .iter()
                .any(|symbol| symbol.semantic_path == expected_path),
            "missing {expected_path} in {:#?}",
            skeleton.available_symbols
        );
    }

    let trace = trace_symbol_graph(&dir, "api::Status::busy", TraceDirection::Both).unwrap();
    assert_eq!(trace.symbol.node_kind, "enumerator");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "api::Task::State::queued", TraceDirection::Both)
            .unwrap();
    assert_eq!(persisted_trace.symbol.node_kind, "enumerator");
    assert_eq!(
        persisted_trace.symbol.scope_path.as_deref(),
        Some("api::Task::State")
    );
}
