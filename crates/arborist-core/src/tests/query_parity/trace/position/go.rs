use super::*;
use crate::{trace_symbol_graph_from_index_with_source, trace_symbol_graph_with_source};

#[test]
fn traces_go_unshadowed_same_file_direct_calls_in_live_workspace_and_persisted_index() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\nfunc caller() int { return helper() }\nfunc helper() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "helper");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    let position = Position { row: 3, column: 5 };
    let live_at_position =
        trace_symbol_graph_at_position(&dir, &source_path, &position, TraceDirection::Callers)
            .unwrap();
    assert_eq!(live_at_position.symbol.symbol_id, "helper");
    assert_eq!(live_at_position.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "helper");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");

    let persisted_at_position = trace_symbol_graph_at_position_from_index(
        &db_path,
        &source_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_at_position.symbol.symbol_id, "helper");
    assert_eq!(persisted_at_position.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_unshadowed_named_receiver_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc (counter Counter) caller() int { return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "Counter::caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "Counter::caller");
}

#[test]
fn traces_go_direct_pointer_and_generic_composite_literal_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Box[T any] struct{}\nfunc (Counter) Value() int { return 1 }\nfunc (Box[T]) Value() int { return 2 }\nfunc pointerCaller() int { return (&Counter{}).Value() }\nfunc genericCaller() int { return Box[int]{}.Value() }\n",
    )
    .unwrap();

    let pointer_live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(pointer_live.callers.len(), 1);
    assert_eq!(pointer_live.callers[0].symbol_id, "pointerCaller");
    let generic_live = trace_symbol_graph(&dir, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(generic_live.callers.len(), 1);
    assert_eq!(generic_live.callers[0].symbol_id, "genericCaller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let pointer_persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(pointer_persisted.callers.len(), 1);
    assert_eq!(pointer_persisted.callers[0].symbol_id, "pointerCaller");
    let generic_persisted =
        trace_symbol_graph_from_index(&db_path, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(generic_persisted.callers.len(), 1);
    assert_eq!(generic_persisted.callers[0].symbol_id, "genericCaller");
}

#[test]
fn traces_go_explicit_type_conversion_method_receiver_forms() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Scalar int\ntype Box[T ~int] int\nfunc (Scalar) Value() int { return 1 }\nfunc (Box[T]) Value() int { return 2 }\nfunc pointerCaller(value *Scalar) int { return (*Scalar)(value).Value() }\nfunc parenthesizedCaller(value int) int { return (Scalar)(value).Value() }\nfunc genericCaller(value int) int { return Box[int](value).Value() }\n",
    )
    .unwrap();

    let scalar_live = trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_live.callers.len(), 2);
    assert_eq!(scalar_live.callers[0].symbol_id, "parenthesizedCaller");
    assert_eq!(scalar_live.callers[1].symbol_id, "pointerCaller");
    let box_live = trace_symbol_graph(&dir, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(box_live.callers.len(), 1);
    assert_eq!(box_live.callers[0].symbol_id, "genericCaller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let scalar_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_persisted.callers.len(), 2);
    assert_eq!(scalar_persisted.callers[0].symbol_id, "parenthesizedCaller");
    assert_eq!(scalar_persisted.callers[1].symbol_id, "pointerCaller");
    let box_persisted =
        trace_symbol_graph_from_index(&db_path, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(box_persisted.callers.len(), 1);
    assert_eq!(box_persisted.callers[0].symbol_id, "genericCaller");
}

#[test]
fn traces_go_explicit_type_conversion_method_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("scalar.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Scalar int\nfunc caller(value *Scalar) int { return (*Scalar)(value).Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_type_conversion_methods_and_preserves_factory_call_edges() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Scalar int\ntype Alias = Scalar\ntype Chained = Alias\ntype LoopA = LoopB\ntype LoopB = LoopA\ntype Broken = Missing\ntype Result struct{}\nfunc (Scalar) Value() int { return 1 }\nfunc (Result) Value() int { return 2 }\nfunc Broken(value int) Result { return Result{} }\nfunc Factory(value int) Result { return Result{} }\nfunc aliasConversionCaller(value int) int { return Alias(value).Value() }\nfunc chainedConversionCaller(value int) int { return Chained(value).Value() }\nfunc conversionCaller(value int) int { return Scalar(value).Value() }\nfunc factoryCaller(value int) int { return Factory(value).Value() }\nfunc loopConversionCaller(value int) int { return LoopA(value).Value() }\nfunc brokenConversionCaller(value int) int { return Broken(value).Value() }\nfunc parenthesizedFactoryCaller(value int) int { return (Factory)(value).Value() }\n",
    )
    .unwrap();

    let conversion_live =
        trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(conversion_live.callers.len(), 3);
    assert_eq!(
        conversion_live.callers[0].symbol_id,
        "aliasConversionCaller"
    );
    assert_eq!(
        conversion_live.callers[1].symbol_id,
        "chainedConversionCaller"
    );
    assert_eq!(conversion_live.callers[2].symbol_id, "conversionCaller");
    let factory_live = trace_symbol_graph(&dir, "Factory", TraceDirection::Callers).unwrap();
    assert_eq!(factory_live.callers.len(), 2);
    assert_eq!(factory_live.callers[0].symbol_id, "factoryCaller");
    assert_eq!(
        factory_live.callers[1].symbol_id,
        "parenthesizedFactoryCaller"
    );
    let broken_live = trace_symbol_graph(&dir, "Broken", TraceDirection::Callers).unwrap();
    assert!(broken_live.callers.is_empty());
    let result_method_live =
        trace_symbol_graph(&dir, "Result::Value", TraceDirection::Callers).unwrap();
    assert!(result_method_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let conversion_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(conversion_persisted.callers.len(), 3);
    assert_eq!(
        conversion_persisted.callers[0].symbol_id,
        "aliasConversionCaller"
    );
    assert_eq!(
        conversion_persisted.callers[1].symbol_id,
        "chainedConversionCaller"
    );
    assert_eq!(
        conversion_persisted.callers[2].symbol_id,
        "conversionCaller"
    );
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "Factory", TraceDirection::Callers).unwrap();
    assert_eq!(factory_persisted.callers.len(), 2);
    assert_eq!(factory_persisted.callers[0].symbol_id, "factoryCaller");
    assert_eq!(
        factory_persisted.callers[1].symbol_id,
        "parenthesizedFactoryCaller"
    );
    let broken_persisted =
        trace_symbol_graph_from_index(&db_path, "Broken", TraceDirection::Callers).unwrap();
    assert!(broken_persisted.callers.is_empty());
    let result_method_persisted =
        trace_symbol_graph_from_index(&db_path, "Result::Value", TraceDirection::Callers).unwrap();
    assert!(result_method_persisted.callers.is_empty());
}

#[test]
fn traces_go_simple_type_alias_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\ntype Chained = Alias\ntype LoopA = LoopB\ntype LoopB = LoopA\nfunc (Counter) Value() int { return 1 }\nfunc compositeCaller() int { return Alias{}.Value() }\nfunc parameterCaller(value Alias) int { return value.Value() }\nfunc localCaller() int { value := Alias{}; return value.Value() }\nfunc chainedCaller() int { return Chained{}.Value() }\nfunc loopCaller() int { return LoopA{}.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    assert_eq!(live.callers[0].symbol_id, "chainedCaller");
    assert_eq!(live.callers[1].symbol_id, "compositeCaller");
    assert_eq!(live.callers[2].symbol_id, "localCaller");
    assert_eq!(live.callers[3].symbol_id, "parameterCaller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    assert_eq!(persisted.callers[0].symbol_id, "chainedCaller");
    assert_eq!(persisted.callers[1].symbol_id, "compositeCaller");
    assert_eq!(persisted.callers[2].symbol_id, "localCaller");
    assert_eq!(persisted.callers[3].symbol_id, "parameterCaller");
}

#[test]
fn traces_go_simple_type_alias_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\ntype Chained = Alias\nfunc caller() int { return Chained{}.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_type_assertion_methods_and_does_not_fallback_to_functions() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Scalar int\ntype Alias = Scalar\ntype Chained = Alias\ntype LoopA = LoopB\ntype LoopB = LoopA\ntype Result struct{}\nfunc (Scalar) Value() int { return 1 }\nfunc (Result) Value() int { return 2 }\nfunc Factory(value int) Result { return Result{} }\nfunc aliasAssertionCaller(value any) int { return value.(Alias).Value() }\nfunc assertionCaller(value any) int { return value.(Scalar).Value() }\nfunc chainedAssertionCaller(value any) int { return value.(Chained).Value() }\nfunc invalidFactoryAssertion(value any) int { return value.(Factory).Value() }\nfunc loopAssertionCaller(value any) int { return value.(LoopA).Value() }\n",
    )
    .unwrap();

    let assertion_live =
        trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(assertion_live.callers.len(), 3);
    assert_eq!(assertion_live.callers[0].symbol_id, "aliasAssertionCaller");
    assert_eq!(assertion_live.callers[1].symbol_id, "assertionCaller");
    assert_eq!(
        assertion_live.callers[2].symbol_id,
        "chainedAssertionCaller"
    );
    let factory_live = trace_symbol_graph(&dir, "Factory", TraceDirection::Callers).unwrap();
    assert!(factory_live.callers.is_empty());
    let result_method_live =
        trace_symbol_graph(&dir, "Result::Value", TraceDirection::Callers).unwrap();
    assert!(result_method_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let assertion_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(assertion_persisted.callers.len(), 3);
    assert_eq!(
        assertion_persisted.callers[0].symbol_id,
        "aliasAssertionCaller"
    );
    assert_eq!(assertion_persisted.callers[1].symbol_id, "assertionCaller");
    assert_eq!(
        assertion_persisted.callers[2].symbol_id,
        "chainedAssertionCaller"
    );
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "Factory", TraceDirection::Callers).unwrap();
    assert!(factory_persisted.callers.is_empty());
    let result_method_persisted =
        trace_symbol_graph_from_index(&db_path, "Result::Value", TraceDirection::Callers).unwrap();
    assert!(result_method_persisted.callers.is_empty());
}

#[test]
fn traces_go_type_assertion_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("scalar.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Scalar int\ntype Alias = Scalar\ntype Chained = Alias\nfunc caller(value any) int { return value.(Chained).Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn fails_closed_for_go_type_conversion_methods_with_ambiguous_type_declarations() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let duplicate_type_path = dir.join("duplicate.go");
    let method_path = dir.join("scalar.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\ntype Scalar int\nfunc caller(value int) int { return Scalar(value).Value() }\nfunc assertionCaller(value any) int { return value.(Scalar).Value() }\n",
    )
    .unwrap();
    fs::write(&duplicate_type_path, "package metrics\n\ntype Scalar int\n").unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_go_type_conversion_methods_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("scalar.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Scalar int\ntype Alias = Scalar\ntype Chained = Alias\nfunc caller(value int) int { return Chained(value).Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_unshadowed_function_body_local_variable_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc caller() int { counter := Counter{}; return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_variables_initialized_from_named_type_conversions() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Scalar int\ntype Box[T ~int] int\ntype Counter struct{}\nfunc (Scalar) Value() int { return 1 }\nfunc (Box[T]) Value() int { return 2 }\nfunc (Counter) Value() int { return 3 }\nfunc Factory() Counter { return Counter{} }\nfunc caller() int { scalar := Scalar(1); box := Box[int](1); factory := Factory(); return scalar.Value() + box.Value() + factory.Value() }\n",
    )
    .unwrap();

    let scalar_live = trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_live.callers.len(), 1);
    assert_eq!(scalar_live.callers[0].symbol_id, "caller");
    let box_live = trace_symbol_graph(&dir, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(box_live.callers.len(), 1);
    assert_eq!(box_live.callers[0].symbol_id, "caller");
    let counter_live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(counter_live.callers.len(), 1);
    assert_eq!(counter_live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let scalar_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_persisted.callers.len(), 1);
    assert_eq!(scalar_persisted.callers[0].symbol_id, "caller");
    let box_persisted =
        trace_symbol_graph_from_index(&db_path, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(box_persisted.callers.len(), 1);
    assert_eq!(box_persisted.callers[0].symbol_id, "caller");
    let counter_persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(counter_persisted.callers.len(), 1);
    assert_eq!(counter_persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_named_conversion_local_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Scalar) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Scalar int\nfunc caller() int { var scalar = Scalar(1); return scalar.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Scalar::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_variables_initialized_from_same_file_factories() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter(seed int) Counter { return Counter{} }\nfunc caller() int { counter := NewCounter(1); var second = NewCounter(2); return counter.Value() + second.Value() }\nfunc direct() int { return NewCounter(3).Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");
    let factory_live = trace_symbol_graph(&dir, "NewCounter", TraceDirection::Callers).unwrap();
    assert_eq!(factory_live.callers.len(), 2);
    assert_eq!(factory_live.callers[0].symbol_id, "caller");
    assert_eq!(factory_live.callers[1].symbol_id, "direct");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "NewCounter", TraceDirection::Callers).unwrap();
    assert_eq!(factory_persisted.callers.len(), 2);
    assert_eq!(factory_persisted.callers[0].symbol_id, "caller");
    assert_eq!(factory_persisted.callers[1].symbol_id, "direct");
}

#[test]
fn traces_go_factory_initialized_local_receivers_through_same_file_aliases() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\nfunc (Counter) Value() int { return 1 }\nfunc NewAlias() Alias { return Alias{} }\nfunc caller() int { counter := NewAlias(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_factory_initialized_local_receivers_through_grouped_aliases() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype (\n    Counter struct{}\n    Alias = Counter\n    Chained = Alias\n)\nfunc (Counter) Value() int { return 1 }\nfunc NewChained() Chained { return Chained{} }\nfunc caller() int { counter := NewChained(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_factory_initialized_local_receivers_through_alias_cycles() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Chained\ntype Chained = Alias\nfunc (Counter) Value() int { return 1 }\nfunc NewChained() Chained { return Chained{} }\nfunc caller() int { counter := NewChained(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());
    let factory_live = trace_symbol_graph(&dir, "NewChained", TraceDirection::Callers).unwrap();
    assert_eq!(factory_live.callers.len(), 1);
    assert_eq!(factory_live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "NewChained", TraceDirection::Callers).unwrap();
    assert_eq!(factory_persisted.callers.len(), 1);
    assert_eq!(factory_persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_factory_initialized_local_receivers_through_unresolved_aliases() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Missing\nfunc (Counter) Value() int { return 1 }\nfunc NewAlias() Alias { return Alias{} }\nfunc caller() int { counter := NewAlias(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn traces_go_factory_initialized_pointer_receivers_through_aliases() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\nfunc (Counter) Value() int { return 1 }\nfunc NewAlias() *Alias { return &Alias{} }\nfunc caller() int { counter := NewAlias(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_factory_receivers_through_ambiguous_aliases() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Other struct{}\ntype Alias = Counter\ntype Alias = Other\nfunc (Counter) Value() int { return 1 }\nfunc (Other) Value() int { return 2 }\nfunc NewAlias() Alias { return Alias{} }\nfunc caller() int { counter := NewAlias(); return counter.Value() }\n",
    )
    .unwrap();

    let counter_live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(counter_live.callers.is_empty());
    let other_live = trace_symbol_graph(&dir, "Other::Value", TraceDirection::Callers).unwrap();
    assert!(other_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let counter_persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(counter_persisted.callers.is_empty());
    let other_persisted =
        trace_symbol_graph_from_index(&db_path, "Other::Value", TraceDirection::Callers).unwrap();
    assert!(other_persisted.callers.is_empty());
}

#[test]
fn traces_go_var_initialized_receivers_through_factory_aliases() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\nfunc (Counter) Value() int { return 1 }\nfunc NewAlias() Alias { return Alias{} }\nfunc caller() int { var counter = NewAlias(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_multi_result_factory_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() (Counter, error) { return Counter{}, nil }\nfunc caller() int { counter, err := NewCounter(); _ = err; return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
    let factory =
        trace_symbol_graph_from_index(&db_path, "NewCounter", TraceDirection::Callers).unwrap();
    assert_eq!(factory.callers.len(), 1);
    assert_eq!(factory.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_grouped_var_factory_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Left struct{}\ntype Right struct{}\nfunc (Left) Value() int { return 1 }\nfunc (Right) Value() int { return 2 }\nfunc NewLeft() Left { return Left{} }\nfunc NewRight() Right { return Right{} }\nfunc caller() int { var ( left = NewLeft() ; right = NewRight() ); return left.Value() + right.Value() }\n",
    )
    .unwrap();

    let left_live = trace_symbol_graph(&dir, "Left::Value", TraceDirection::Callers).unwrap();
    assert_eq!(left_live.callers.len(), 1);
    assert_eq!(left_live.callers[0].symbol_id, "caller");
    let right_live = trace_symbol_graph(&dir, "Right::Value", TraceDirection::Callers).unwrap();
    assert_eq!(right_live.callers.len(), 1);
    assert_eq!(right_live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let left_persisted =
        trace_symbol_graph_from_index(&db_path, "Left::Value", TraceDirection::Callers).unwrap();
    assert_eq!(left_persisted.callers.len(), 1);
    assert_eq!(left_persisted.callers[0].symbol_id, "caller");
    let right_persisted =
        trace_symbol_graph_from_index(&db_path, "Right::Value", TraceDirection::Callers).unwrap();
    assert_eq!(right_persisted.callers.len(), 1);
    assert_eq!(right_persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_named_result_factory_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() (result Counter) { return Counter{} }\nfunc caller() int { counter := NewCounter(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_named_result_factory_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\nfunc NewCounter() (result Counter) { return Counter{} }\nfunc caller() int { counter := NewCounter(); return counter.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_named_result_factory_alias_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\nfunc NewCounter() (result Alias) { return Alias{} }\nfunc caller() int { counter := NewCounter(); return counter.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_pointer_named_result_factory_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() (result *Counter) { return &Counter{} }\nfunc caller() int { counter := NewCounter(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_named_result_factory_receivers_through_aliases() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\ntype Final = Alias\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() (result Final) { return Final{} }\nfunc caller() int { counter := NewCounter(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_pointer_named_result_factory_receivers_through_aliases() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\ntype Final = Alias\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() (result *Final) { return &Final{} }\nfunc caller() int { counter := NewCounter(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_parenthesized_single_result_factory_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() (Counter) { return Counter{} }\nfunc caller() int { counter := NewCounter(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_multi_value_var_factory_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Left struct{}\ntype Right struct{}\nfunc (Left) Value() int { return 1 }\nfunc (Right) Value() int { return 2 }\nfunc NewLeft() Left { return Left{} }\nfunc NewRight() Right { return Right{} }\nfunc caller() int { var left, right = NewLeft(), NewRight(); return left.Value() + right.Value() }\n",
    )
    .unwrap();

    let left_live = trace_symbol_graph(&dir, "Left::Value", TraceDirection::Callers).unwrap();
    assert_eq!(left_live.callers.len(), 1);
    assert_eq!(left_live.callers[0].symbol_id, "caller");
    let right_live = trace_symbol_graph(&dir, "Right::Value", TraceDirection::Callers).unwrap();
    assert_eq!(right_live.callers.len(), 1);
    assert_eq!(right_live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let left_persisted =
        trace_symbol_graph_from_index(&db_path, "Left::Value", TraceDirection::Callers).unwrap();
    assert_eq!(left_persisted.callers.len(), 1);
    assert_eq!(left_persisted.callers[0].symbol_id, "caller");
    let right_persisted =
        trace_symbol_graph_from_index(&db_path, "Right::Value", TraceDirection::Callers).unwrap();
    assert_eq!(right_persisted.callers.len(), 1);
    assert_eq!(right_persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_multi_value_var_factory_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\ntype Left struct{}\ntype Right struct{}\nfunc (Left) Value() int { return 1 }\nfunc (Right) Value() int { return 2 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Left struct{}\ntype Right struct{}\nfunc NewLeft() Left { return Left{} }\nfunc NewRight() Right { return Right{} }\nfunc caller() int { var left, right = NewLeft(), NewRight(); return left.Value() + right.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Left::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");
    let live_right = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Right::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live_right.callers.len(), 1);
    assert_eq!(live_right.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Left::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
    let persisted_right = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Right::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_right.callers.len(), 1);
    assert_eq!(persisted_right.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_parenthesized_factory_alias_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\nfunc (Counter) Value() int { return 1 }\nfunc NewAlias() Alias { return Alias{} }\nfunc caller() int { counter := (NewAlias)(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn preserves_go_direct_factory_alias_call_edges() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\nfunc (Counter) Value() int { return 1 }\nfunc NewAlias() Alias { return Alias{} }\nfunc caller() int { return NewAlias().Value() }\n",
    )
    .unwrap();

    let method_live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(method_live.callers.is_empty());
    let factory_live = trace_symbol_graph(&dir, "NewAlias", TraceDirection::Callers).unwrap();
    assert_eq!(factory_live.callers.len(), 1);
    assert_eq!(factory_live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let method_persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(method_persisted.callers.is_empty());
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "NewAlias", TraceDirection::Callers).unwrap();
    assert_eq!(factory_persisted.callers.len(), 1);
    assert_eq!(factory_persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_parenthesized_factory_initialized_local_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() Counter { return Counter{} }\nfunc caller() int { counter := (NewCounter)(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_factory_initialized_pointer_local_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() *Counter { return &Counter{} }\nfunc caller() int { counter := NewCounter(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_factory_local_receivers_with_unresolved_return_types() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc Broken() Missing { return Missing{} }\nfunc caller() int { counter := Broken(); return counter.Value() }\n",
    )
    .unwrap();

    let method_live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(method_live.callers.is_empty());
    let factory_live = trace_symbol_graph(&dir, "Broken", TraceDirection::Callers).unwrap();
    assert_eq!(factory_live.callers.len(), 1);
    assert_eq!(factory_live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let method_persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(method_persisted.callers.is_empty());
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "Broken", TraceDirection::Callers).unwrap();
    assert_eq!(factory_persisted.callers.len(), 1);
    assert_eq!(factory_persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_ambiguous_same_file_factory_return_types() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\ntype Other struct{}\nfunc (Counter) Value() int { return 1 }\nfunc (Other) Value() int { return 2 }\nfunc New() Counter { return Counter{} }\nfunc New() Other { return Other{} }\nfunc caller() int { counter := New(); return counter.Value() }\n",
    )
    .unwrap();

    let counter_live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(counter_live.callers.is_empty());
    let other_live = trace_symbol_graph(&dir, "Other::Value", TraceDirection::Callers).unwrap();
    assert!(other_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let counter_persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(counter_persisted.callers.is_empty());
    let other_persisted =
        trace_symbol_graph_from_index(&db_path, "Other::Value", TraceDirection::Callers).unwrap();
    assert!(other_persisted.callers.is_empty());
}

#[test]
fn does_not_trace_go_shadowed_factory_names_as_local_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() Counter { return Counter{} }\nfunc caller() int { NewCounter := func() Counter { return Counter{} }; counter := NewCounter(); return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn does_not_trace_go_shadowed_named_result_factory_as_local_receiver() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc NewCounter() (result Counter) { return Counter{} }\nfunc caller() int { NewCounter := func() Counter { return Counter{} }; counter := NewCounter(); return counter.Value() }\n",
    )
    .unwrap();

    let method_live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(method_live.callers.is_empty());
    let factory_live = trace_symbol_graph(&dir, "NewCounter", TraceDirection::Callers).unwrap();
    assert!(factory_live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let method_persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(method_persisted.callers.is_empty());
    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "NewCounter", TraceDirection::Callers).unwrap();
    assert!(factory_persisted.callers.is_empty());
}

#[test]
fn traces_go_factory_alias_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\ntype Alias = Counter\nfunc NewAlias() Alias { return Alias{} }\nfunc caller() int { counter := NewAlias(); return counter.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_factory_initialized_local_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\nfunc NewCounter() Counter { return Counter{} }\nfunc caller() int { counter := NewCounter(); var second = NewCounter(); return counter.Value() + second.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_variable_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\nfunc caller() int { counter := Counter{}; return counter.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_variables_initialized_from_parenthesized_and_pointer_literals() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc parenthesized() int { counter := (Counter{}); return counter.Value() }\nfunc pointer() int { counter := &Counter{}; return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort_unstable();
    assert_eq!(callers, ["parenthesized", "pointer"]);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort_unstable();
    assert_eq!(callers, ["parenthesized", "pointer"]);
}
#[test]
fn traces_go_var_wrapped_literal_local_receivers() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc caller() int { var parenthesized = (Counter{}); var pointer = &Counter{}; return parenthesized.Value() + pointer.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}
#[test]
fn traces_go_wrapped_literal_local_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\nfunc parenthesized() int { counter := (Counter{}); return counter.Value() }\nfunc pointer() int { counter := &Counter{}; return counter.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    let mut callers = live
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort_unstable();
    assert_eq!(callers, ["parenthesized", "pointer"]);

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    let mut callers = persisted
        .callers
        .iter()
        .map(|caller| caller.symbol_id.as_str())
        .collect::<Vec<_>>();
    callers.sort_unstable();
    assert_eq!(callers, ["parenthesized", "pointer"]);
}
#[test]
fn traces_go_unshadowed_named_parameter_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc caller(counter *Counter) int { return counter.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_parenthesized_and_pointer_composite_literal_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc caller() int { return (Counter{}).Value() + (&Counter{}).Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}
#[test]
fn traces_go_same_file_direct_composite_literal_method_calls() {
    let dir = temporary_dir();
    let source_path = dir.join("metrics.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &source_path,
        "package metrics\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc caller() int { return Counter{}.Value() }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 1);
    assert_eq!(live.symbol.symbol_id, "Counter::Value");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 1);
    assert_eq!(persisted.symbol.symbol_id, "Counter::Value");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_same_package_direct_composite_literal_method_calls_across_source_files() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\ntype Counter struct{}\nfunc caller() int { return Counter{}.Value() }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.symbol.symbol_id, "Counter::Value");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.symbol.symbol_id, "Counter::Value");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_same_package_composite_literal_method_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let method_path = dir.join("counter.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &method_path,
        "package metrics\n\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\ntype Counter struct{}\nfunc caller() int { return Counter{}.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_package_imported_function_calls_in_live_and_persisted_indexes() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_path = dir.join("internal").join("service").join("service.go");
    let utility_path = dir.join("internal").join("utility").join("utility.go");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    fs::create_dir_all(utility_path.parent().unwrap()).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport (\n    \"example.com/project/internal/service\"\n    utility_alias \"example.com/project/internal/utility\"\n)\n\ntype local struct{}\nfunc (local) Value() int { return 0 }\nfunc caller() int { return service.Value() + utility_alias.Other() }\nfunc shadowed() int { service := local{}; return service.Value() }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\nfunc Value() int { return 1 }\n",
    )
    .unwrap();
    fs::write(
        &utility_path,
        "package utility\n\nfunc Other() int { return 2 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 3);
    assert_eq!(live.symbol.symbol_id, "Value");
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 3);
    assert_eq!(persisted.symbol.symbol_id, "Value");
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");

    let position = Position { row: 2, column: 5 };
    let persisted_at_position = trace_symbol_graph_at_position_from_index(
        &db_path,
        &service_path,
        &position,
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_at_position.symbol.symbol_id, "Value");
    assert_eq!(persisted_at_position.callers.len(), 1);
    assert_eq!(persisted_at_position.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_local_package_imported_type_method_receivers() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_path = dir.join("internal").join("service").join("service.go");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc composite() int { return svc.Counter{}.Value() }\nfunc pointer() int { return (&svc.Counter{}).Value() }\nfunc generic() int { return svc.Box[int]{}.Value() }\nfunc conversion(value int) int { return svc.Scalar(value).Value() }\nfunc assertion(value any) int { return value.(svc.Counter).Value() }\nfunc parameter(value svc.Counter) int { return value.Value() }\nfunc factory(value int) int { return svc.New(value).Value() }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\nfunc New(value int) Counter { return Counter{} }\ntype Scalar int\nfunc (Scalar) Value() int { return 2 }\ntype Box[T any] struct{}\nfunc (Box[T]) Value() int { return 3 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 4);
    for caller in ["composite", "pointer", "assertion", "parameter"] {
        assert!(
            live.callers
                .iter()
                .any(|candidate| candidate.symbol_id == caller),
            "missing Counter caller {caller}"
        );
    }

    let box_live = trace_symbol_graph(&dir, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(box_live.callers.len(), 1);
    assert_eq!(box_live.callers[0].symbol_id, "generic");

    let scalar_live = trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_live.callers.len(), 1);
    assert_eq!(scalar_live.callers[0].symbol_id, "conversion");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    for caller in ["composite", "pointer", "assertion", "parameter"] {
        assert!(
            persisted
                .callers
                .iter()
                .any(|candidate| candidate.symbol_id == caller),
            "missing persisted Counter caller {caller}"
        );
    }

    let box_persisted =
        trace_symbol_graph_from_index(&db_path, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(box_persisted.callers.len(), 1);
    assert_eq!(box_persisted.callers[0].symbol_id, "generic");

    let scalar_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_persisted.callers.len(), 1);
    assert_eq!(scalar_persisted.callers[0].symbol_id, "conversion");

    let factory_persisted =
        trace_symbol_graph_from_index(&db_path, "New", TraceDirection::Callers).unwrap();
    assert_eq!(factory_persisted.callers.len(), 1);
    assert_eq!(factory_persisted.callers[0].symbol_id, "factory");
}

#[test]
fn traces_go_imported_type_alias_method_receivers() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_path = dir.join("internal").join("service").join("service.go");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc caller() int { return svc.Alias{}.Value() }\nfunc chained() int { return svc.Chained{}.Value() }\nfunc pointerAlias(value svc.PointerAlias) int { return value.Value() }\nfunc parenthesizedAlias(value svc.ParenthesizedAlias) int { return value.Value() }\nfunc genericAlias() int { return svc.IntBox{}.Value() }\nfunc scalarConversion(value int) int { return svc.ScalarAlias(value).Value() }\nfunc scalarAssertion(value any) int { return value.(svc.ScalarAlias).Value() }\nfunc cycle() int { return svc.LoopA{}.Value() }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\ntype Counter struct{}\ntype Alias = Counter\ntype Chained = Alias\ntype PointerAlias = *Counter\ntype ParenthesizedAlias = (Counter)\ntype Box[T any] struct{}\ntype IntBox = Box[int]\ntype Scalar int\ntype ScalarAlias = Scalar\ntype LoopA = LoopB\ntype LoopB = LoopA\nfunc (Counter) Value() int { return 1 }\nfunc (Box[T]) Value() int { return 2 }\nfunc (Scalar) Value() int { return 3 }\n",
    )
    .unwrap();
    fs::write(
        service_path.parent().unwrap().join("broken_test.go"),
        "package service\n\nfunc broken( {\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(live.callers.len(), 4);
    assert_eq!(live.callers[0].symbol_id, "caller");
    assert_eq!(live.callers[1].symbol_id, "chained");
    assert_eq!(live.callers[2].symbol_id, "parenthesizedAlias");
    assert_eq!(live.callers[3].symbol_id, "pointerAlias");
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.callers.len(), 4);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
    assert_eq!(persisted.callers[1].symbol_id, "chained");
    assert_eq!(persisted.callers[2].symbol_id, "parenthesizedAlias");
    assert_eq!(persisted.callers[3].symbol_id, "pointerAlias");

    let generic_live = trace_symbol_graph(&dir, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(generic_live.callers.len(), 1);
    assert_eq!(generic_live.callers[0].symbol_id, "genericAlias");
    let generic_persisted =
        trace_symbol_graph_from_index(&db_path, "Box::Value", TraceDirection::Callers).unwrap();
    assert_eq!(generic_persisted.callers.len(), 1);
    assert_eq!(generic_persisted.callers[0].symbol_id, "genericAlias");

    let scalar_live = trace_symbol_graph(&dir, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_live.callers.len(), 2);
    assert_eq!(scalar_live.callers[0].symbol_id, "scalarAssertion");
    assert_eq!(scalar_live.callers[1].symbol_id, "scalarConversion");
    let scalar_persisted =
        trace_symbol_graph_from_index(&db_path, "Scalar::Value", TraceDirection::Callers).unwrap();
    assert_eq!(scalar_persisted.callers.len(), 2);
    assert_eq!(scalar_persisted.callers[0].symbol_id, "scalarAssertion");
    assert_eq!(scalar_persisted.callers[1].symbol_id, "scalarConversion");
}

#[test]
fn traces_go_imported_type_method_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_path = dir.join("internal").join("service").join("service.go");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\ntype Counter struct{}\ntype CounterAlias = Counter\nfunc (Counter) Value() int { return 1 }\ntype Pointer struct{}\ntype PointerAlias = *Pointer\nfunc (*Pointer) Value() int { return 2 }\ntype Box[T any] struct{}\ntype IntBox = Box[int]\nfunc (Box[T]) Value() int { return 3 }\n",
    )
    .unwrap();
    let overlay = "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc caller() int { return svc.CounterAlias{}.Value() }\nfunc pointer() int { var value svc.PointerAlias; return value.Value() }\nfunc generic() int { return svc.IntBox{}.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");
    let live_pointer = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Pointer::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live_pointer.callers.len(), 1);
    assert_eq!(live_pointer.callers[0].symbol_id, "pointer");
    let live_box = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Box::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live_box.callers.len(), 1);
    assert_eq!(live_box.callers[0].symbol_id, "generic");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
    let persisted_pointer = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Pointer::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_pointer.callers.len(), 1);
    assert_eq!(persisted_pointer.callers[0].symbol_id, "pointer");
    let persisted_box = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Box::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted_box.callers.len(), 1);
    assert_eq!(persisted_box.callers[0].symbol_id, "generic");
}

#[test]
fn does_not_trace_go_imported_unsupported_alias_targets() {
    let cases = [
        (
            "unexported-alias",
            "type Counter struct{}\ntype alias = Counter\nfunc (Counter) Value() int { return 1 }\n",
            "svc.alias{}.Value()",
            "Counter::Value",
        ),
        (
            "qualified-alias",
            "type Counter struct{}\ntype Alias = other.Counter\nfunc (Counter) Value() int { return 1 }\n",
            "svc.Alias{}.Value()",
            "Counter::Value",
        ),
    ];
    for (name, service_source, call, target) in cases {
        let dir = temporary_dir();
        let caller_path = dir.join("cmd").join("main.go");
        let service_path = dir.join("internal").join("service").join("service.go");
        fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
        fs::create_dir_all(service_path.parent().unwrap()).unwrap();
        fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
        fs::write(
            &caller_path,
            format!("package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc caller() int {{ return {call} }}\n"),
        )
        .unwrap();
        fs::write(
            &service_path,
            format!("package service\n\n{service_source}"),
        )
        .unwrap();

        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty(), "{name}: {live:#?}");
    }
}

#[test]
fn does_not_trace_go_imported_test_only_type_aliases() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_dir = dir.join("internal").join("service");
    let service_path = service_dir.join("service.go");
    let alias_test_path = service_dir.join("alias_test.go");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(&service_dir).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc caller() int { return svc.Alias{}.Value() }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\ntype Counter struct{}\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    fs::write(
        &alias_test_path,
        "package service\n\ntype Alias = Counter\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());
    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}

#[test]
fn does_not_trace_go_imported_ambiguous_type_aliases() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_dir = dir.join("internal").join("service");
    let service_path = service_dir.join("service.go");
    let duplicate_path = service_dir.join("duplicate.go");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(&service_dir).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc caller() int { return svc.Alias{}.Value() }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\ntype Counter struct{}\ntype Alias = Counter\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    fs::write(&duplicate_path, "package service\n\ntype Alias = Counter\n").unwrap();

    let live = trace_symbol_graph(&dir, "Counter::Value", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());
}

#[test]
fn traces_go_imported_type_alias_method_receivers_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("cmd").join("main.go");
    let service_path = dir.join("internal").join("service").join("service.go");
    let db_path = dir.join("symbols.db");
    fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
    fs::create_dir_all(service_path.parent().unwrap()).unwrap();
    fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
    fs::write(
        &caller_path,
        "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &service_path,
        "package service\n\ntype Counter struct{}\ntype Alias = Counter\nfunc (Counter) Value() int { return 1 }\n",
    )
    .unwrap();
    let overlay = "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc caller() int { return svc.Alias{}.Value() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        overlay,
        "Counter::Value",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_imported_type_methods_from_invalid_or_unexported_packages() {
    let cases = [
        (
            "mismatched-package",
            "type Counter struct{}\nfunc (Counter) Value() int { return 1 }\n",
            Some("package other\n"),
            "Counter::Value",
        ),
        (
            "unexported-type",
            "type counter struct{}\nfunc (counter) Value() int { return 1 }\n",
            None,
            "counter::Value",
        ),
    ];

    for (name, service_source, extra_source, target) in cases {
        let dir = temporary_dir();
        let caller_path = dir.join("cmd").join("main.go");
        let service_dir = dir.join("internal").join("service");
        let service_path = service_dir.join("service.go");
        let db_path = dir.join("symbols.db");
        fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
        fs::create_dir_all(&service_dir).unwrap();
        fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
        fs::write(
            &caller_path,
            "package main\n\nimport svc \"example.com/project/internal/service\"\n\nfunc caller() int { return svc.Counter{}.Value() }\n",
        )
        .unwrap();
        fs::write(
            &service_path,
            format!("package service\n\n{service_source}"),
        )
        .unwrap();
        if let Some(extra_source) = extra_source {
            fs::write(service_dir.join("other.go"), extra_source).unwrap();
        }

        let live = trace_symbol_graph(&dir, target, TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty(), "{name}: {live:#?}");

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, target, TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty(), "{name}: {persisted:#?}");
    }
}

#[test]
fn does_not_trace_go_dot_or_blank_imports_as_package_qualified_calls() {
    for (suffix, import) in [
        ("dot", ". \"example.com/project/internal/service\""),
        ("blank", "_ \"example.com/project/internal/service\""),
    ] {
        let dir = temporary_dir();
        let caller_path = dir.join("cmd").join("main.go");
        let service_path = dir.join("internal").join("service").join("service.go");
        let db_path = dir.join("symbols.db");
        fs::create_dir_all(caller_path.parent().unwrap()).unwrap();
        fs::create_dir_all(service_path.parent().unwrap()).unwrap();
        fs::write(dir.join("go.mod"), "module example.com/project\n").unwrap();
        fs::write(
            &caller_path,
            format!(
                "package main\n\nimport {import}\n\nfunc caller() int {{ return service.Value() }}\n"
            ),
        )
        .unwrap();
        fs::write(
            &service_path,
            "package service\n\nfunc Value() int { return 1 }\n",
        )
        .unwrap();

        let live = trace_symbol_graph(&dir, "Value", TraceDirection::Callers).unwrap();
        assert!(live.callers.is_empty(), "{suffix}: {live:#?}");

        rebuild_symbol_index(&dir, &db_path).unwrap();
        let persisted =
            trace_symbol_graph_from_index(&db_path, "Value", TraceDirection::Callers).unwrap();
        assert!(persisted.callers.is_empty(), "{suffix}: {persisted:#?}");
    }
}

#[test]
fn traces_go_same_package_direct_calls_across_source_files() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let helper_path = dir.join("helper.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc caller() int { return helper() }\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package metrics\n\nfunc helper() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(live.indexed_files, 2);
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert_eq!(persisted.indexed_files, 2);
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn traces_go_same_package_direct_calls_from_dirty_vfs_overrides() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let helper_path = dir.join("helper.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc stale() int { return 0 }\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package metrics\n\nfunc helper() int { return 1 }\n",
    )
    .unwrap();
    let caller_overlay = "package metrics\n\nfunc caller() int { return helper() }\n";

    let live = trace_symbol_graph_with_source(
        &dir,
        &caller_path,
        caller_overlay,
        "helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(live.callers.len(), 1);
    assert_eq!(live.callers[0].symbol_id, "caller");

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted = trace_symbol_graph_from_index_with_source(
        &db_path,
        &caller_path,
        caller_overlay,
        "helper",
        TraceDirection::Callers,
    )
    .unwrap();
    assert_eq!(persisted.callers.len(), 1);
    assert_eq!(persisted.callers[0].symbol_id, "caller");
}

#[test]
fn does_not_trace_go_same_directory_calls_across_different_or_test_packages() {
    let dir = temporary_dir();
    let caller_path = dir.join("caller.go");
    let helper_path = dir.join("helper_test.go");
    let db_path = dir.join("symbols.db");
    fs::write(
        &caller_path,
        "package metrics\n\nfunc caller() int { return helper() }\n",
    )
    .unwrap();
    fs::write(
        &helper_path,
        "package metrics_test\n\nfunc helper() int { return 1 }\n",
    )
    .unwrap();

    let live = trace_symbol_graph(&dir, "helper", TraceDirection::Callers).unwrap();
    assert!(live.callers.is_empty());

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted =
        trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Callers).unwrap();
    assert!(persisted.callers.is_empty());
}
