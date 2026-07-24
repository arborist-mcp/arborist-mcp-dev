use super::*;

#[test]
fn resolves_python_import_alias_patch_bindings_to_local_module_symbols() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("caller.py");

    fs::write(
        &helper,
        "def helper(value: int) -> int:\n    \"\"\"Imported helper.\"\"\"\n    return value + 1\n",
    )
    .unwrap();
    fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let alias_attribute = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "gb.helper")
        .unwrap();
    assert_eq!(alias_attribute.symbol.semantic_path, "helper");
    assert_eq!(
        alias_attribute.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(alias_attribute.symbol.origin_type, "imported_module");
    assert_eq!(
        alias_attribute.symbol.docstring.as_deref(),
        Some("\"\"\"Imported helper.\"\"\"")
    );

    let alias_import = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "h")
        .unwrap();
    assert_eq!(alias_import.symbol.semantic_path, "helper");
    assert_eq!(
        alias_import.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(alias_import.symbol.origin_type, "imported_module");
}

#[test]
fn resolves_python_relative_import_alias_patch_bindings_to_local_module_symbols() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_b.py");
    let local_helper = subpackage.join("local_mod.py");
    let caller = subpackage.join("caller.py");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
            &helper,
            "def helper(value: int) -> int:\n    \"\"\"Parent package helper.\"\"\"\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &local_helper,
            "def helper2(value: int) -> int:\n    \"\"\"Sibling package helper.\"\"\"\n    return value + 2\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from ..graph_b import helper as h\nfrom .local_mod import helper2 as h2\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return h(value) + h2(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let imported_helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "h")
        .unwrap();
    assert_eq!(imported_helper.symbol.semantic_path, "helper");
    assert_eq!(
        imported_helper.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(imported_helper.symbol.origin_type, "imported_module");
    assert_eq!(
        imported_helper.symbol.docstring.as_deref(),
        Some("\"\"\"Parent package helper.\"\"\"")
    );

    let sibling_helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "h2")
        .unwrap();
    assert_eq!(sibling_helper.symbol.semantic_path, "helper2");
    assert_eq!(
        sibling_helper.symbol.file_path,
        local_helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(sibling_helper.symbol.origin_type, "imported_module");
    assert_eq!(
        sibling_helper.symbol.docstring.as_deref(),
        Some("\"\"\"Sibling package helper.\"\"\"")
    );
}

#[test]
fn resolves_python_absolute_package_import_alias_patch_bindings_to_local_module_symbols() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_c.py");
    let caller = subpackage.join("caller.py");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Absolute package worker.\"\"\"\n    return value + 3\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "import pkg.graph_c as gc\nfrom pkg.graph_c import worker as w\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return gc.worker(value) + w(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let module_alias = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "gc.worker")
        .unwrap();
    assert_eq!(module_alias.symbol.semantic_path, "worker");
    assert_eq!(
        module_alias.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(module_alias.symbol.origin_type, "imported_module");

    let symbol_alias = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "w")
        .unwrap();
    assert_eq!(symbol_alias.symbol.semantic_path, "worker");
    assert_eq!(
        symbol_alias.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(symbol_alias.symbol.origin_type, "imported_module");
    assert_eq!(
        symbol_alias.symbol.docstring.as_deref(),
        Some("\"\"\"Absolute package worker.\"\"\"")
    );
}

#[test]
fn resolves_python_import_from_module_alias_patch_bindings_to_local_module_symbols() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let subpackage = package.join("sub");
    let helper = package.join("graph_c.py");
    let local_helper = subpackage.join("local_mod.py");
    let caller = subpackage.join("caller.py");

    fs::create_dir_all(&subpackage).unwrap();
    fs::write(package.join("__init__.py"), "").unwrap();
    fs::write(subpackage.join("__init__.py"), "").unwrap();
    fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Absolute package worker.\"\"\"\n    return value + 3\n",
        )
        .unwrap();
    fs::write(
            &local_helper,
            "def helper2(value: int) -> int:\n    \"\"\"Sibling module helper.\"\"\"\n    return value + 2\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "from pkg import graph_c as gc\nfrom . import local_mod as lm\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return gc.worker(value) + lm.helper2(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let package_module_alias = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "gc.worker")
        .unwrap();
    assert_eq!(package_module_alias.symbol.semantic_path, "worker");
    assert_eq!(
        package_module_alias.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(package_module_alias.symbol.origin_type, "imported_module");

    let sibling_module_alias = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "lm.helper2")
        .unwrap();
    assert_eq!(sibling_module_alias.symbol.semantic_path, "helper2");
    assert_eq!(
        sibling_module_alias.symbol.file_path,
        local_helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(sibling_module_alias.symbol.origin_type, "imported_module");
}

#[test]
fn resolves_python_package_reexport_patch_bindings_to_underlying_local_symbols() {
    let dir = temporary_dir();
    let package = dir.join("pkg");
    let helper = package.join("graph_c.py");
    let caller = dir.join("caller.py");

    fs::create_dir_all(&package).unwrap();
    fs::write(
        package.join("__init__.py"),
        "from .graph_c import worker as worker\n",
    )
    .unwrap();
    fs::write(
            &helper,
            "def worker(value: int) -> int:\n    \"\"\"Re-exported package worker.\"\"\"\n    return value + 4\n",
        )
        .unwrap();
    fs::write(
        &caller,
        "from pkg import worker\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
    )
    .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return worker(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let imported_worker = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "worker")
        .unwrap();
    assert_eq!(imported_worker.symbol.semantic_path, "worker");
    assert_eq!(
        imported_worker.symbol.file_path,
        helper.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(imported_worker.symbol.origin_type, "imported_module");
    assert_eq!(
        imported_worker.symbol.docstring.as_deref(),
        Some("\"\"\"Re-exported package worker.\"\"\"")
    );
}

#[test]
fn resolves_decorated_python_local_bindings_for_patch_validation() {
    let source = r#"
def decorator(func):
    return func

@decorator
def helper(value: int) -> int:
    return value + 1

def top_level(value: int) -> int:
    return value + 1
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level(value: int) -> int:\n    return helper(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
    let helper_start = source.find(helper_text).unwrap();
    let helper_end = helper_start + helper_text.len();

    let helper_binding = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "helper")
        .unwrap();
    assert_eq!(helper_binding.symbol.semantic_path, "helper");
    assert_eq!(helper_binding.symbol.origin_type, "module_scope");
    assert_eq!(
        helper_binding.symbol.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert_eq!(helper_binding.symbol.byte_range, (helper_start, helper_end));
}

#[test]
fn resolves_decorated_python_import_metadata_for_patch_validation() {
    let dir = temporary_dir();
    let helper = dir.join("graph_b.py");
    let caller = dir.join("caller.py");

    fs::write(
            &helper,
            "def decorator(func):\n    return func\n\n@decorator\ndef helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
    fs::write(
            &caller,
            "import graph_b as gb\nfrom graph_b import helper as h\n\ndef top_level(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();

    let source = fs::read_to_string(&caller).unwrap();
    let result = patch_ast_node(
        &caller,
        &source,
        "top_level",
        "def top_level(value: int) -> int:\n    return gb.helper(value) + h(value)\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.commit_gate.allowed);
    assert!(result.validation.unresolved_identifiers.is_empty());

    let helper_source = fs::read_to_string(&helper).unwrap();
    let helper_text = "@decorator\ndef helper(value: int) -> int:\n    return value + 1";
    let helper_start = helper_source.find(helper_text).unwrap();
    let helper_end = helper_start + helper_text.len();

    let imported_helper = result
        .validation
        .resolved_identifiers
        .iter()
        .find(|binding| binding.name == "h")
        .unwrap();
    assert_eq!(imported_helper.symbol.semantic_path, "helper");
    assert_eq!(imported_helper.symbol.origin_type, "imported_module");
    assert_eq!(
        imported_helper.symbol.signature.as_deref(),
        Some("@decorator\ndef helper(value: int) -> int:")
    );
    assert_eq!(
        imported_helper.symbol.byte_range,
        (helper_start, helper_end)
    );
    assert!(
        imported_helper
            .symbol
            .evidence_key
            .contains("@decorator\ndef helper(value: int) -> int:")
    );
}

#[test]
fn resolves_imported_module_attribute_calls_for_patch_validation() {
    let source = r#"
import json
import os

def top_level() -> str:
    return ""
"#;

    let result = patch_ast_node(
        Path::new("sample.py"),
        source,
        "top_level",
        "def top_level() -> str:\n    return json.dumps({'pid': os.getpid()})\n",
        None,
    )
    .unwrap();

    assert!(result.applied);
    assert!(result.validation.unresolved_identifiers.is_empty());
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .any(|binding| binding.name == "json.dumps")
    );
    assert!(
        result
            .validation
            .resolved_identifiers
            .iter()
            .any(|binding| binding.name == "os.getpid")
    );
}
