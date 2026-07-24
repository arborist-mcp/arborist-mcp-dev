use super::*;

#[test]
fn traces_decorated_python_symbol_metadata_through_index() {
    let dir = temporary_dir();
    let helper = dir.join("helper.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
            &helper,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from helper import helper\n\ndef decorator(func):\n    return func\n\n@decorator\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

    let helper_source = fs::read_to_string(&helper).unwrap();
    let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
    let helper_start = helper_source.find(helper_text).unwrap();
    let helper_end = helper_start + helper_text.len();

    let caller_source = fs::read_to_string(&caller).unwrap();
    let orchestrate_text =
        "@decorator\ndef orchestrate(value: int) -> int:\n    return helper(value)";
    let orchestrate_start = caller_source.find(orchestrate_text).unwrap();
    let orchestrate_end = orchestrate_start + orchestrate_text.len();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.symbol.origin_type, "trace_root");
    assert_eq!(
        live_trace.symbol.evidence_key,
        live_trace.evidence_keys.symbol
    );
    assert_eq!(
        live_trace.symbol.signature.as_deref(),
        Some("@decorator\ndef orchestrate(value: int) -> int:")
    );
    assert_eq!(
        live_trace.symbol.byte_range,
        (orchestrate_start, orchestrate_end)
    );
    let live_helper = live_trace
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .unwrap();
    assert_eq!(
        live_helper.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert_eq!(live_helper.byte_range, (helper_start, helper_end));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.symbol.origin_type, "trace_root");
    assert_eq!(
        persisted_trace.symbol.evidence_key,
        persisted_trace.evidence_keys.symbol
    );
    assert_eq!(
        persisted_trace.symbol.signature.as_deref(),
        Some("@decorator\ndef orchestrate(value: int) -> int:")
    );
    assert_eq!(
        persisted_trace.symbol.byte_range,
        (orchestrate_start, orchestrate_end)
    );
    let persisted_helper = persisted_trace
        .callees
        .iter()
        .find(|symbol| symbol.semantic_path == "helper")
        .unwrap();
    assert_eq!(
        persisted_helper.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert_eq!(persisted_helper.byte_range, (helper_start, helper_end));
}

#[test]
fn traces_python_alias_import_calls_across_files() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\n\ndef orchestrate(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 1);
    assert_eq!(live_trace.callees[0].semantic_path, "helper");
    assert_eq!(
        live_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "helper");
    assert_eq!(
        persisted_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_python_absolute_package_alias_import_calls_across_files() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_c.py");
    let caller = subpackage.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
        &helper,
        "def worker(value: int) -> int:\n    return value + 3\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "import pkg.graph_c as gc\nfrom pkg.graph_c import worker as w\n\n\ndef orchestrate(value: int) -> int:\n    return gc.worker(value) + w(value)\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 1);
    assert_eq!(live_trace.callees[0].semantic_path, "worker");
    assert_eq!(
        live_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "worker");
    assert_eq!(
        persisted_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
}

#[test]
fn traces_python_instance_method_calls_across_files() {
    let dir = temporary_dir();
    let model = dir.join("product.py");
    let caller = dir.join("orchestrate.py");
    let db_path = dir.join("symbols.db");

    fs::write(
        &model,
        "class Product:\n    def price_with_tax(self, rate: float) -> float:\n        return self.price * rate\n",
    )
    .unwrap();
    fs::write(
        &caller,
        "from product import Product\n\n\ndef orchestrate(rate: float) -> float:\n    product = Product()\n    return product.price_with_tax(rate)\n",
    )
    .unwrap();

    let live_trace =
        trace_symbol_graph(&dir, "Product.price_with_tax", TraceDirection::Both).unwrap();
    assert!(
        live_trace
            .callers
            .iter()
            .any(|symbol| symbol.semantic_path == "orchestrate")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "Product.price_with_tax", TraceDirection::Both)
            .unwrap();
    assert!(
        persisted_trace
            .callers
            .iter()
            .any(|symbol| symbol.semantic_path == "orchestrate")
    );
}

#[test]
fn traces_python_import_from_module_alias_calls_across_files() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_c.py");
    let local_helper = subpackage.join("local_mod.py");
    let caller = subpackage.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
        &helper,
        "def worker(value: int) -> int:\n    return value + 3\n",
    )
    .unwrap();
    fs::write(
        &local_helper,
        "def helper2(value: int) -> int:\n    return value + 2\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from pkg import graph_c as gc\nfrom . import local_mod as lm\n\n\ndef orchestrate(value: int) -> int:\n    return gc.worker(value) + lm.helper2(value)\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 2);
    assert!(live_trace.callees.iter().any(|symbol| {
        symbol.semantic_path == "worker"
            && symbol.file_path == helper.to_string_lossy().replace('\\', "/")
    }));
    assert!(live_trace.callees.iter().any(|symbol| {
        symbol.semantic_path == "helper2"
            && symbol.file_path == local_helper.to_string_lossy().replace('\\', "/")
    }));

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 2);
    assert!(persisted_trace.callees.iter().any(|symbol| {
        symbol.semantic_path == "worker"
            && symbol.file_path == helper.to_string_lossy().replace('\\', "/")
    }));
    assert!(persisted_trace.callees.iter().any(|symbol| {
        symbol.semantic_path == "helper2"
            && symbol.file_path == local_helper.to_string_lossy().replace('\\', "/")
    }));
}

#[test]
fn traces_python_package_reexport_calls_across_files() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let helper = package.join("graph_c.py");
    let caller = dir.join("caller.py");
    let db_path = dir.join("symbols.db");

    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("__init__.py"),
        "from .graph_c import worker as worker\n",
    )
    .unwrap();
    fs::write(
        &helper,
        "def worker(value: int) -> int:\n    return value + 4\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "from pkg import worker\n\n\ndef orchestrate(value: int) -> int:\n    return worker(value)\n",
        )
        .unwrap();

    let live_trace = trace_symbol_graph(&dir, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(live_trace.callees.len(), 1);
    assert_eq!(live_trace.callees[0].semantic_path, "worker");
    assert_eq!(
        live_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );

    rebuild_symbol_index(&dir, &db_path).unwrap();
    let persisted_trace =
        trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
    assert_eq!(persisted_trace.callees.len(), 1);
    assert_eq!(persisted_trace.callees[0].semantic_path, "worker");
    assert_eq!(
        persisted_trace.callees[0].file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
}
