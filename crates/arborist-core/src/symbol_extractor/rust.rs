use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::Result;
use tree_sitter::Node;

use crate::language::{node_text, normalize_path, rust_direct_module_candidate_paths};
use crate::semantic::rust::{
    is_rust_symbol_node, rust_inherent_impl_scope_name, rust_parameters, rust_return_type,
    rust_semantic_path, rust_signature, rust_symbol_name,
};
use crate::symbol_index_model::{
    IndexedSymbol, ReferenceFact, ReferenceLanguageDetails, RustImportRoot, RustReferenceDetails,
    symbol_base_name,
};
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn index_rust_symbols_with_deadline(
    path: &Path,
    source: &str,
    root: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    collect_symbols(path, source, root, deadline, &mut symbols)?;
    Ok(symbols)
}

fn collect_symbols(
    path: &Path,
    source: &str,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
    symbols: &mut Vec<IndexedSymbol>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting Rust symbols")?;
    }
    if is_rust_symbol_node(node)
        && let Some(symbol) = indexed_symbol(path, source, node, deadline)?
    {
        symbols.push(symbol);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_symbols(path, source, child, deadline, symbols)?;
    }
    Ok(())
}

fn indexed_symbol(
    path: &Path,
    source: &str,
    node: Node<'_>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<IndexedSymbol>> {
    let Some(name) = rust_symbol_name(node, source)? else {
        return Ok(None);
    };
    let Some(semantic_path) = rust_semantic_path(node, source, &name)? else {
        return Ok(None);
    };
    let scope_path = semantic_path
        .rsplit_once("::")
        .map(|(scope_path, _)| scope_path.to_string());
    let references = collect_direct_local_calls(path, node, source, deadline)?;
    let references_by_name = references.paths();
    let reference_facts = references.reference_facts();
    let call_arities_by_name = BTreeMap::new();

    Ok(Some(IndexedSymbol {
        extension_receiver: None,
        symbol_id: semantic_path.clone(),
        base_name: symbol_base_name(&semantic_path),
        semantic_path,
        scope_path,
        file_path: normalize_path(path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: rust_signature(node, source),
        is_overload: false,
        parameters: rust_parameters(node, source),
        return_type: rust_return_type(node, source),
        docstring: None,
        reference_facts,
        references_by_name,
        call_arities_by_name,
    }))
}

fn collect_direct_local_calls(
    path: &Path,
    symbol_node: Node<'_>,
    source: &str,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<RustDirectCallReferences> {
    if symbol_node.kind() != "function_item" {
        return Ok(RustDirectCallReferences::default());
    }
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(RustDirectCallReferences::default());
    };
    let local_functions = local_module_function_paths(symbol_node, source)?.unwrap_or_default();
    let local_function_names =
        local_module_function_names(symbol_node, source)?.unwrap_or_default();
    let qualified_functions = source_file_module_function_paths(symbol_node, source)?;
    let imported_functions = source_file_imported_function_paths(symbol_node, source)?;
    let out_of_line_modules = source_file_out_of_line_module_names(path, symbol_node, source)?;
    let module_or_import_names = out_of_line_modules
        .iter()
        .chain(imported_functions.keys())
        .cloned()
        .collect::<BTreeSet<_>>();
    let local_variable_types =
        collect_rust_local_variable_types(symbol_node, source, &module_or_import_names)?;
    let self_type_path = rust_impl_self_type_path(symbol_node, source)?;
    if local_functions.is_empty()
        && imported_functions.is_empty()
        && qualified_functions.is_empty()
        && out_of_line_modules.is_empty()
        && local_variable_types.is_empty()
        && self_type_path.is_none()
    {
        return Ok(RustDirectCallReferences::default());
    }

    let mut bindings = BTreeSet::new();
    collect_function_bindings(symbol_node, source, &mut bindings)?;
    let context = RustDirectCallContext {
        source,
        deadline,
        local_functions: &local_functions,
        local_function_names: &local_function_names,
        imported_functions: &imported_functions,
        qualified_functions: &qualified_functions,
        out_of_line_modules: &out_of_line_modules,
        module_components: rust_inline_module_path_components(symbol_node, source)?,
        bindings: &bindings,
        local_variable_types: &local_variable_types,
        self_type_path,
    };
    let mut references = RustDirectCallReferences::default();
    collect_direct_local_calls_from_node(body, &context, &mut references)?;
    Ok(references)
}

struct RustDirectCallContext<'a> {
    source: &'a str,
    deadline: Option<&'a WorkspaceScanDeadline>,
    local_functions: &'a BTreeMap<String, String>,
    local_function_names: &'a BTreeSet<String>,
    imported_functions: &'a BTreeMap<String, RustImportedFunctionBinding>,
    qualified_functions: &'a BTreeMap<String, String>,
    out_of_line_modules: &'a BTreeSet<String>,
    module_components: Option<Vec<String>>,
    bindings: &'a BTreeSet<String>,
    local_variable_types: &'a BTreeMap<String, String>,
    self_type_path: Option<String>,
}

fn source_file_out_of_line_module_names(
    path: &Path,
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeSet<String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    Ok(rust_direct_module_candidate_paths(path, root, source)?
        .into_keys()
        .collect())
}

fn local_module_function_names(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<BTreeSet<String>>> {
    let Some(container) = local_module_function_container(symbol_node) else {
        return Ok(None);
    };

    let mut names = BTreeSet::new();
    let mut cursor = container.walk();
    for child in container.named_children(&mut cursor) {
        if child.kind() != "function_item" {
            continue;
        }
        if let Some(name) = rust_symbol_name(child, source)? {
            names.insert(name);
        }
    }
    Ok(Some(names))
}

#[derive(Debug, Clone)]
struct RustImportedFunctionBinding {
    target_path: String,
    import_root: RustImportRoot,
}

#[derive(Debug, Default)]
struct RustDirectCallReferences {
    paths_by_import_root: BTreeMap<String, BTreeSet<Option<RustImportRoot>>>,
}

impl RustDirectCallReferences {
    fn insert(&mut self, path: String, import_root: Option<RustImportRoot>) {
        self.paths_by_import_root
            .entry(path)
            .or_default()
            .insert(import_root);
    }

    fn paths(&self) -> BTreeSet<String> {
        self.paths_by_import_root.keys().cloned().collect()
    }

    fn reference_facts(&self) -> Vec<ReferenceFact> {
        self.paths_by_import_root
            .iter()
            .filter_map(|(path, roots)| {
                let language_details = if roots.len() == 1 {
                    match roots.first()? {
                        Some(import_root) => ReferenceLanguageDetails::Rust(RustReferenceDetails {
                            import_root: Some(import_root.clone()),
                        }),
                        None => ReferenceLanguageDetails::None,
                    }
                } else {
                    return None;
                };
                Some(ReferenceFact {
                    spelling: path.clone(),
                    call_arities: None,
                    language_details,
                })
            })
            .collect()
    }
}

fn source_file_imported_function_paths(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, RustImportedFunctionBinding>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut paths_by_local_name = BTreeMap::<String, Vec<RustImportedFunctionBinding>>::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "use_declaration" {
            continue;
        }
        let Some(argument) = node.child_by_field_name("argument") else {
            continue;
        };
        let mut bindings = Vec::new();
        collect_rust_function_import_bindings(argument, &[], source, &mut bindings)?;
        for (local_name, binding) in bindings {
            paths_by_local_name
                .entry(local_name)
                .or_default()
                .push(binding);
        }
    }

    Ok(paths_by_local_name
        .into_iter()
        .filter_map(|(name, paths)| (paths.len() == 1).then(|| (name, paths[0].clone())))
        .collect())
}

fn collect_rust_function_import_bindings(
    node: Node<'_>,
    prefix: &[String],
    source: &str,
    bindings: &mut Vec<(String, RustImportedFunctionBinding)>,
) -> Result<()> {
    match node.kind() {
        "scoped_use_list" => {
            let Some(path) = node.child_by_field_name("path") else {
                return Ok(());
            };
            let Some(path_components) = rust_import_path_components(path, source)? else {
                return Ok(());
            };
            let Some(prefix) = rust_join_import_path_components(prefix, &path_components) else {
                return Ok(());
            };
            let Some(list) = node.child_by_field_name("list") else {
                return Ok(());
            };
            collect_rust_function_import_bindings(list, &prefix, source, bindings)?;
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                collect_rust_function_import_bindings(child, prefix, source, bindings)?;
            }
        }
        "use_as_clause" => {
            let Some(path) = node.child_by_field_name("path") else {
                return Ok(());
            };
            let Some(alias) = node.child_by_field_name("alias") else {
                return Ok(());
            };
            let Some(path_components) = rust_import_path_components(path, source)? else {
                return Ok(());
            };
            let Some(target_components) =
                rust_join_import_path_components(prefix, &path_components)
            else {
                return Ok(());
            };
            let alias = node_text(alias, source)?.trim();
            if let Some(binding) = rust_function_import_binding(&target_components, alias) {
                bindings.push(binding);
            }
        }
        "scoped_identifier" | "identifier" => {
            let Some(path_components) = rust_import_path_components(node, source)? else {
                return Ok(());
            };
            let Some(target_components) =
                rust_join_import_path_components(prefix, &path_components)
            else {
                return Ok(());
            };
            let Some(local_name) = target_components.last() else {
                return Ok(());
            };
            if let Some(binding) = rust_function_import_binding(&target_components, local_name) {
                bindings.push(binding);
            }
        }
        _ => {}
    }
    Ok(())
}

fn rust_import_path_components(node: Node<'_>, source: &str) -> Result<Option<Vec<String>>> {
    if !matches!(
        node.kind(),
        "crate" | "self" | "super" | "identifier" | "scoped_identifier"
    ) {
        return Ok(None);
    }
    let spelling = node_text(node, source)?.trim();
    let components = spelling
        .split("::")
        .filter(|component| !component.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if components.is_empty() || spelling.split("::").any(str::is_empty) {
        return Ok(None);
    }
    Ok(Some(components))
}

fn rust_join_import_path_components(prefix: &[String], path: &[String]) -> Option<Vec<String>> {
    if prefix.is_empty() {
        return Some(path.to_vec());
    }
    (!matches!(
        path.first().map(String::as_str),
        Some("crate" | "self" | "super")
    ))
    .then(|| prefix.iter().chain(path).cloned().collect::<Vec<String>>())
}

fn rust_function_import_binding(
    target_components: &[String],
    local_name: &str,
) -> Option<(String, RustImportedFunctionBinding)> {
    if local_name.is_empty()
        || target_components.len() < 2
        || target_components
            .iter()
            .any(|component| component.is_empty())
    {
        return None;
    }
    let (import_root, root_len) = match target_components.first()?.as_str() {
        "crate" => (RustImportRoot::Crate, 1),
        "self" => (RustImportRoot::SelfModule, 1),
        "super" => {
            let levels = target_components
                .iter()
                .take_while(|component| component.as_str() == "super")
                .count();
            (RustImportRoot::Super { levels }, levels)
        }
        _ => return None,
    };
    let target_components = target_components.get(root_len..)?;
    (!target_components.is_empty()).then(|| {
        (
            local_name.to_string(),
            RustImportedFunctionBinding {
                target_path: target_components.join("::"),
                import_root,
            },
        )
    })
}

fn local_module_function_paths(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<BTreeMap<String, String>>> {
    let Some(container) = local_module_function_container(symbol_node) else {
        return Ok(None);
    };

    let mut paths_by_name = BTreeMap::<String, Vec<String>>::new();
    let mut cursor = container.walk();
    for child in container.named_children(&mut cursor) {
        if child.kind() != "function_item" {
            continue;
        }
        let Some(name) = rust_symbol_name(child, source)? else {
            continue;
        };
        let Some(path) = rust_semantic_path(child, source, &name)? else {
            continue;
        };
        paths_by_name.entry(name).or_default().push(path);
    }

    Ok(Some(
        paths_by_name
            .into_iter()
            .filter_map(|(name, paths)| (paths.len() == 1).then(|| (name, paths[0].clone())))
            .collect(),
    ))
}

fn source_file_module_function_paths(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeMap<String, String>> {
    let mut root = symbol_node;
    while let Some(parent) = root.parent() {
        root = parent;
    }

    let mut paths_by_name = BTreeMap::<String, Vec<String>>::new();
    collect_source_file_module_function_paths(root, source, &mut paths_by_name)?;
    Ok(paths_by_name
        .into_iter()
        .filter_map(|(path, candidates)| {
            (candidates.len() == 1).then(|| (path, candidates[0].clone()))
        })
        .collect())
}

fn collect_source_file_module_function_paths(
    node: Node<'_>,
    source: &str,
    paths_by_name: &mut BTreeMap<String, Vec<String>>,
) -> Result<()> {
    if node.kind() == "function_item"
        && local_module_function_container(node).is_some()
        && let Some(name) = rust_symbol_name(node, source)?
        && let Some(path) = rust_semantic_path(node, source, &name)?
    {
        paths_by_name.entry(path.clone()).or_default().push(path);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_source_file_module_function_paths(child, source, paths_by_name)?;
    }
    Ok(())
}

fn local_module_function_container(symbol_node: Node<'_>) -> Option<Node<'_>> {
    let parent = symbol_node.parent()?;
    if parent.kind() == "source_file" {
        return Some(parent);
    }
    (parent.kind() == "declaration_list"
        && parent
            .parent()
            .is_some_and(|owner| owner.kind() == "mod_item"))
    .then_some(parent)
}

fn collect_function_bindings(
    symbol_node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    if let Some(parameters) = symbol_node.child_by_field_name("parameters") {
        let mut cursor = parameters.walk();
        for parameter in parameters.named_children(&mut cursor) {
            if let Some(pattern) = parameter.child_by_field_name("pattern") {
                collect_pattern_bindings(pattern, source, bindings)?;
            }
        }
    }
    if let Some(body) = symbol_node.child_by_field_name("body") {
        collect_body_bindings(body, source, bindings)?;
    }
    Ok(())
}

fn collect_body_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    if matches!(
        node.kind(),
        "closure_expression" | "function_item" | "function_signature_item"
    ) {
        return Ok(());
    }
    if let Some(pattern) = node.child_by_field_name("pattern") {
        collect_pattern_bindings(pattern, source, bindings)?;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_body_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn collect_pattern_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    if node.kind() == "identifier" {
        let name = node_text(node, source)?.trim();
        if !name.is_empty() {
            bindings.insert(name.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_pattern_bindings(child, source, bindings)?;
    }
    Ok(())
}

fn collect_rust_local_variable_types(
    symbol_node: Node<'_>,
    source: &str,
    module_or_import_names: &BTreeSet<String>,
) -> Result<BTreeMap<String, String>> {
    let Some(body) = symbol_node.child_by_field_name("body") else {
        return Ok(BTreeMap::new());
    };
    let mut types_by_name = BTreeMap::<String, BTreeSet<String>>::new();
    collect_rust_parameter_types(symbol_node, source, &mut types_by_name)?;
    collect_rust_let_binding_types(body, source, module_or_import_names, &mut types_by_name)?;
    let module_components = rust_inline_module_path_components(symbol_node, source)?;
    Ok(types_by_name
        .into_iter()
        .filter_map(|(name, types)| {
            let type_name = types.iter().next()?.clone();
            (types.len() == 1).then(|| {
                let mut path = module_components.clone().unwrap_or_default();
                path.push(type_name);
                (name, path.join("::"))
            })
        })
        .collect())
}

fn collect_rust_let_binding_types(
    node: Node<'_>,
    source: &str,
    module_or_import_names: &BTreeSet<String>,
    types_by_name: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<()> {
    if matches!(
        node.kind(),
        "closure_expression" | "function_item" | "function_signature_item"
    ) {
        return Ok(());
    }
    if node.kind() == "let_declaration"
        && let Some(pattern) = node.child_by_field_name("pattern")
        && let Some(value) = node.child_by_field_name("value")
        && let Some(type_name) = rust_let_binding_type_name(value, source, module_or_import_names)?
        && pattern.kind() == "identifier"
        && let name = node_text(pattern, source)?.trim()
        && !name.is_empty()
    {
        types_by_name
            .entry(name.to_string())
            .or_default()
            .insert(type_name);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_rust_let_binding_types(child, source, module_or_import_names, types_by_name)?;
    }
    Ok(())
}

fn rust_let_binding_type_name(
    value: Node<'_>,
    source: &str,
    module_or_import_names: &BTreeSet<String>,
) -> Result<Option<String>> {
    if let Some(type_name) = rust_struct_expression_type_name(value, source)? {
        return Ok(Some(type_name));
    }
    rust_constructor_call_type_name(value, source, module_or_import_names)
}

fn rust_constructor_call_type_name(
    value: Node<'_>,
    source: &str,
    module_or_import_names: &BTreeSet<String>,
) -> Result<Option<String>> {
    if value.kind() != "call_expression" {
        return Ok(None);
    }
    let Some(function) = value.child_by_field_name("function") else {
        return Ok(None);
    };
    if function.kind() != "scoped_identifier" {
        return Ok(None);
    }
    let spelling = node_text(function, source)?.trim();
    let mut components = spelling.split("::");
    let Some(type_name) = components.next() else {
        return Ok(None);
    };
    let Some(constructor_name) = components.next() else {
        return Ok(None);
    };
    if type_name.is_empty()
        || constructor_name.is_empty()
        || components.next().is_some()
        || module_or_import_names.contains(type_name)
        || !rust_type_name_like(type_name)
    {
        return Ok(None);
    }
    Ok(Some(type_name.to_string()))
}

fn rust_impl_self_type_path(symbol_node: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut current = symbol_node.parent();
    while let Some(parent) = current {
        if parent.kind() == "impl_item" {
            let Some(scope_name) = rust_inherent_impl_scope_name(parent, source)? else {
                return Ok(None);
            };
            let mut path =
                rust_inline_module_path_components(symbol_node, source)?.unwrap_or_default();
            path.push(scope_name);
            return Ok(Some(path.join("::")));
        }
        if matches!(parent.kind(), "function_item" | "closure_expression") {
            return Ok(None);
        }
        current = parent.parent();
    }
    Ok(None)
}

fn collect_rust_parameter_types(
    symbol_node: Node<'_>,
    source: &str,
    types_by_name: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<()> {
    let Some(parameters) = symbol_node.child_by_field_name("parameters") else {
        return Ok(());
    };
    let generic_parameters = rust_generic_type_parameter_names(symbol_node, source)?;
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        if parameter.kind() != "parameter" {
            continue;
        }
        let Some(pattern) = parameter.child_by_field_name("pattern") else {
            continue;
        };
        if pattern.kind() != "identifier" {
            continue;
        }
        let Some(type_node) = parameter.child_by_field_name("type") else {
            continue;
        };
        let Some(type_name) = rust_parameter_local_type(type_node, source)? else {
            continue;
        };
        if generic_parameters.contains(&type_name) {
            continue;
        }
        let name = node_text(pattern, source)?.trim();
        if !name.is_empty() {
            types_by_name
                .entry(name.to_string())
                .or_default()
                .insert(type_name);
        }
    }
    Ok(())
}

fn rust_parameter_local_type(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let type_node = match node.kind() {
        "type_identifier" => Some(node),
        "reference_type" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| child.kind() == "type_identifier")
        }
        _ => None,
    };
    let Some(type_node) = type_node else {
        return Ok(None);
    };
    let type_name = node_text(type_node, source)?.trim();
    Ok((!type_name.is_empty()).then(|| type_name.to_string()))
}

fn rust_generic_type_parameter_names(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<BTreeSet<String>> {
    let Some(type_parameters) = symbol_node.child_by_field_name("type_parameters") else {
        return Ok(BTreeSet::new());
    };
    let mut names = BTreeSet::new();
    let mut cursor = type_parameters.walk();
    for parameter in type_parameters.named_children(&mut cursor) {
        if parameter.kind() != "type_parameter" {
            continue;
        }
        let Some(name_node) = parameter.child_by_field_name("name") else {
            continue;
        };
        let name = node_text(name_node, source)?.trim();
        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }
    Ok(names)
}

fn rust_struct_expression_type_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    if node.kind() != "struct_expression" {
        return Ok(None);
    }
    let Some(type_node) = node.named_child(0) else {
        return Ok(None);
    };
    if type_node.kind() != "type_identifier" {
        return Ok(None);
    }
    let type_name = node_text(type_node, source)?.trim();
    Ok((!type_name.is_empty()).then(|| type_name.to_string()))
}

fn rust_method_call_target_path(
    function: Node<'_>,
    context: &RustDirectCallContext<'_>,
) -> Result<Option<String>> {
    let Some(receiver) = function.child_by_field_name("value") else {
        return Ok(None);
    };
    let Some(field) = function.child_by_field_name("field") else {
        return Ok(None);
    };
    let method_name = node_text(field, context.source)?.trim();
    if method_name.is_empty() {
        return Ok(None);
    }
    let type_path = if receiver.kind() == "identifier" {
        let name = node_text(receiver, context.source)?.trim();
        if name.is_empty() {
            return Ok(None);
        }
        let Some(type_path) = context.local_variable_types.get(name) else {
            return Ok(None);
        };
        type_path.clone()
    } else if receiver.kind() == "struct_expression" {
        let Some(type_name) = rust_struct_expression_type_name(receiver, context.source)? else {
            return Ok(None);
        };
        let mut path = context.module_components.clone().unwrap_or_default();
        path.push(type_name);
        path.join("::")
    } else if receiver.kind() == "self" {
        let Some(self_type_path) = context.self_type_path.as_deref() else {
            return Ok(None);
        };
        self_type_path.to_string()
    } else {
        return Ok(None);
    };
    Ok(Some(format!("{type_path}::{method_name}")))
}

fn collect_direct_local_calls_from_node(
    node: Node<'_>,
    context: &RustDirectCallContext<'_>,
    references: &mut RustDirectCallReferences,
) -> Result<()> {
    if let Some(deadline) = context.deadline {
        deadline.check("collecting Rust direct calls")?;
    }
    if matches!(
        node.kind(),
        "closure_expression" | "function_item" | "function_signature_item"
    ) {
        return Ok(());
    }
    if node.kind() == "call_expression"
        && let Some(function) = node.child_by_field_name("function")
    {
        if function.kind() == "identifier" {
            let name = node_text(function, context.source)?.trim();
            if !name.is_empty() && !context.bindings.contains(name) {
                if let Some(path) = context.local_functions.get(name) {
                    references.insert(path.clone(), None);
                } else if !context.local_function_names.contains(name)
                    && let Some(path) = context.imported_functions.get(name)
                {
                    references.insert(path.target_path.clone(), Some(path.import_root.clone()));
                }
            }
        } else if function.kind() == "field_expression"
            && let Some(method_path) = rust_method_call_target_path(function, context)?
        {
            references.insert(method_path, None);
        } else if function.kind() == "scoped_identifier"
            && let Some(module_components) = context.module_components.as_deref()
            && let Some((path, import_root)) =
                rust_qualified_call_target_path(function, module_components, context.source)?
        {
            if let Some(path) = context.qualified_functions.get(&path) {
                references.insert(path.clone(), import_root);
            } else if is_rust_parent_qualified_module_call(import_root.as_ref(), &path)
                || is_out_of_line_module_call(&path, context.out_of_line_modules)
                || is_rust_type_qualified_static_call(
                    &path,
                    import_root.as_ref(),
                    context.out_of_line_modules,
                )
            {
                references.insert(path, import_root);
            } else if let Some((target_path, target_root)) = is_rust_module_binding_qualified_call(
                &path,
                import_root.as_ref(),
                context.imported_functions,
                context.out_of_line_modules,
            ) {
                references.insert(target_path, Some(target_root));
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_direct_local_calls_from_node(child, context, references)?;
    }
    Ok(())
}

fn is_rust_parent_qualified_module_call(import_root: Option<&RustImportRoot>, path: &str) -> bool {
    match import_root {
        Some(RustImportRoot::Crate) | Some(RustImportRoot::Super { .. }) => !path.is_empty(),
        Some(RustImportRoot::SelfModule) | None => false,
    }
}

fn is_rust_type_qualified_static_call(
    path: &str,
    import_root: Option<&RustImportRoot>,
    out_of_line_modules: &BTreeSet<String>,
) -> bool {
    if import_root.is_some() {
        return false;
    }
    let mut components = path.split("::");
    let Some(first) = components.next() else {
        return false;
    };
    let Some(second) = components.next() else {
        return false;
    };
    if second.is_empty() || components.any(|component| component.is_empty()) {
        return false;
    }
    if out_of_line_modules.contains(first) {
        return false;
    }
    rust_type_name_like(first)
}

fn is_rust_module_binding_qualified_call(
    path: &str,
    import_root: Option<&RustImportRoot>,
    imported_functions: &BTreeMap<String, RustImportedFunctionBinding>,
    out_of_line_modules: &BTreeSet<String>,
) -> Option<(String, RustImportRoot)> {
    if import_root.is_some() {
        return None;
    }
    let mut components = path.split("::");
    let first = components.next()?;
    if out_of_line_modules.contains(first) {
        return None;
    }
    let binding = imported_functions.get(first)?;
    if binding.import_root != RustImportRoot::Crate {
        return None;
    }
    let rest = components.collect::<Vec<_>>();
    if rest.is_empty() || rest.iter().any(|component| component.is_empty()) {
        return None;
    }
    let mut target = binding.target_path.clone();
    for component in rest {
        target.push_str("::");
        target.push_str(component);
    }
    Some((target, binding.import_root.clone()))
}

fn rust_type_name_like(name: &str) -> bool {
    name.chars()
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
}

fn is_out_of_line_module_call(path: &str, out_of_line_modules: &BTreeSet<String>) -> bool {
    let mut components = path.split("::");
    let Some(module_name) = components.next() else {
        return false;
    };
    let Some(next_component) = components.next() else {
        return false;
    };
    !next_component.is_empty()
        && components.all(|component| !component.is_empty())
        && out_of_line_modules.contains(module_name)
}

fn rust_inline_module_path_components(
    symbol_node: Node<'_>,
    source: &str,
) -> Result<Option<Vec<String>>> {
    let mut components = Vec::new();
    let mut current = symbol_node.parent();
    while let Some(parent) = current {
        if parent.kind() == "mod_item" {
            let Some(name) = rust_symbol_name(parent, source)? else {
                return Ok(None);
            };
            components.push(name);
        }
        current = parent.parent();
    }
    components.reverse();
    Ok(Some(components))
}

fn rust_qualified_call_target_path(
    function: Node<'_>,
    module_components: &[String],
    source: &str,
) -> Result<Option<(String, Option<RustImportRoot>)>> {
    let spelling = node_text(function, source)?.trim();
    if spelling.is_empty() {
        return Ok(None);
    }
    let components = spelling.split("::").collect::<Vec<_>>();
    if components.iter().any(|component| component.is_empty()) {
        return Ok(None);
    }

    let mut module_components = module_components.to_vec();
    let mut components = components.into_iter();
    let mut import_root = None;
    match components.next() {
        Some("crate") => {
            module_components.clear();
            import_root = Some(RustImportRoot::Crate);
        }
        Some("self") => {}
        Some("super") => {
            let mut parent_count = 1;
            while matches!(components.clone().next(), Some("super")) {
                components.next();
                parent_count += 1;
            }
            if parent_count > module_components.len() {
                let levels = parent_count - module_components.len();
                module_components.clear();
                import_root = Some(RustImportRoot::Super { levels });
            } else {
                module_components.truncate(module_components.len() - parent_count);
            }
        }
        Some(first) => module_components.push(first.to_string()),
        None => return Ok(None),
    }
    module_components.extend(components.map(str::to_string));
    Ok((!module_components.is_empty()).then(|| (module_components.join("::"), import_root)))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::index_rust_symbols_with_deadline;
    use crate::language::parse_document;
    use crate::symbol_index_model::{ReferenceLanguageDetails, RustImportRoot};

    #[test]
    fn indexes_unshadowed_direct_calls_to_local_module_functions() {
        let source = r#"
fn root_caller() { root_helper(); }
fn root_helper() {}

mod api {
    fn caller() {
        helper();
    }

    fn helper() {}
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "api::caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            ["api::helper".to_string()].into()
        );
        assert_eq!(caller.reference_facts.len(), 1);
        assert_eq!(caller.reference_facts[0].spelling, "api::helper");
        assert!(caller.call_arities_by_name.is_empty());

        let root_caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "root_caller")
            .unwrap();
        assert_eq!(
            root_caller.references_by_name,
            ["root_helper".to_string()].into()
        );
    }

    #[test]
    fn indexes_qualified_calls_between_inline_modules() {
        let source = r#"
fn root_helper() {}

fn crate_caller() {
    crate::root_helper();
}

fn caller() {
    api::helper();
}

mod api {
    fn helper() {}

    fn self_caller() {
        self::helper();
    }

    mod nested {
        fn caller() {
            super::helper();
        }

        mod leaf {
            fn caller() {
                super::super::helper();
            }
        }
    }
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for (caller_path, callee_path) in [
            ("crate_caller", "root_helper"),
            ("caller", "api::helper"),
            ("api::self_caller", "api::helper"),
            ("api::nested::caller", "api::helper"),
            ("api::nested::leaf::caller", "api::helper"),
        ] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(
                caller.references_by_name,
                [callee_path.to_string()].into(),
                "unexpected references for {caller_path}",
            );
        }
    }

    #[test]
    fn indexes_unshadowed_direct_calls_to_root_function_imports() {
        let source = r#"
mod api;
use crate::api::helper;
use crate::api::helper as aliased_helper;

fn caller() { helper(); }
fn alias_caller() { aliased_helper(); }
"#;
        let path = Path::new("src/lib.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for caller_path in ["caller", "alias_caller"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(
                caller.references_by_name,
                ["api::helper".to_string()].into()
            );
        }
    }

    #[test]
    fn indexes_unshadowed_direct_calls_to_self_function_imports() {
        let source = r#"
mod nested;
use self::nested::value;
use self::{nested::value as grouped_value};

fn caller() { value(); }
fn grouped_caller() { grouped_value(); }
"#;
        let path = Path::new("src/api/mod.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for caller_path in ["caller", "grouped_caller"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(
                caller.references_by_name,
                ["nested::value".to_string()].into()
            );
        }
    }

    #[test]
    fn indexes_unshadowed_direct_calls_to_super_function_imports_with_ancestor_metadata() {
        let source = r#"
use super::root_helper;
use super::{root_helper as grouped_root_helper};

fn caller() { root_helper(); }
fn grouped_caller() { grouped_root_helper(); }
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for caller_path in ["caller", "grouped_caller"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert_eq!(
                caller.references_by_name,
                ["root_helper".to_string()].into()
            );
            assert!(matches!(
                caller.reference_facts.as_slice(),
                [reference]
                    if reference.spelling == "root_helper"
                        && matches!(
                            reference.language_details,
                            ReferenceLanguageDetails::Rust(ref details)
                                if details.import_root == Some(RustImportRoot::Super { levels: 1 })
                        )
            ));
        }
    }

    #[test]
    fn prefers_local_functions_over_root_function_imports() {
        let source = r#"
mod api;
use crate::api::helper;

fn helper() {}
fn caller() { helper(); }
"#;
        let path = Path::new("src/lib.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert_eq!(caller.references_by_name, ["helper".to_string()].into());
    }

    #[test]
    fn ignores_ambiguous_and_shadowed_root_function_import_calls() {
        let source = r#"
mod api;
use crate::api::helper;
use crate::api::helper as helper;
use crate::api::other::*;

fn caller() { helper(); }
fn shadowed(helper: fn()) { helper(); }
"#;
        let path = Path::new("src/lib.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        for caller_path in ["caller", "shadowed"] {
            let caller = symbols
                .iter()
                .find(|symbol| symbol.semantic_path == caller_path)
                .unwrap();
            assert!(caller.references_by_name.is_empty());
        }
    }

    #[test]
    fn indexes_parent_qualified_calls_with_ancestor_metadata() {
        let source = r#"
fn caller() {
    crate::sibling::helper();
    super::parent_helper();
    super::super::grandparent_helper();
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            [
                "sibling::helper".to_string(),
                "parent_helper".to_string(),
                "grandparent_helper".to_string(),
            ]
            .into()
        );
        assert!(caller.reference_facts.iter().any(|reference| {
            reference.spelling == "sibling::helper"
                && matches!(
                    reference.language_details,
                    ReferenceLanguageDetails::Rust(ref details)
                        if details.import_root == Some(RustImportRoot::Crate)
                )
        }));
        assert!(caller.reference_facts.iter().any(|reference| {
            reference.spelling == "parent_helper"
                && matches!(
                    reference.language_details,
                    ReferenceLanguageDetails::Rust(ref details)
                        if details.import_root == Some(RustImportRoot::Super { levels: 1 })
                )
        }));
        assert!(caller.reference_facts.iter().any(|reference| {
            reference.spelling == "grandparent_helper"
                && matches!(
                    reference.language_details,
                    ReferenceLanguageDetails::Rust(ref details)
                        if details.import_root == Some(RustImportRoot::Super { levels: 2 })
                )
        }));
    }

    #[test]
    fn indexes_qualified_calls_to_declared_out_of_line_modules() {
        let source = r#"
mod api;

fn caller() {
    api::helper();
}
"#;
        let path = Path::new("src/lib.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert_eq!(
            caller.references_by_name,
            ["api::helper".to_string()].into()
        );
    }

    #[test]
    fn resolves_qualified_calls_from_nested_functions_relative_to_their_module() {
        let source = r#"
fn root_helper() {}

fn outer() {
    fn nested() {
        self::root_helper();
        super::root_helper();
    }
}

mod outer {
    fn root_helper() {}
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let nested = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "outer::nested")
            .unwrap();
        assert_eq!(
            nested.references_by_name,
            ["root_helper".to_string()].into(),
        );
    }

    #[test]
    fn ignores_shadowed_and_nonlocal_rust_calls() {
        let source = r#"
mod api {
    fn caller(helper: fn()) {
        helper();
        let helper = || {};
        helper();
        if let Some(helper) = Some(|| {}) {
            helper();
        }
        crate::outside();
    }

    fn helper() {}
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "api::caller")
            .unwrap();
        assert!(caller.references_by_name.is_empty());
        assert!(caller.reference_facts.is_empty());
    }

    #[test]
    fn indexes_rust_declarations_and_inherent_impl_methods_without_references() {
        let source = r#"
pub mod metrics {
    pub struct Counter;
    pub type Count = u64;
    pub const DEFAULT: Count = 1;
    pub static ACTIVE: bool = true;
    pub enum Event { Tick }
    pub trait Render { fn render(&self) -> String; }

    impl Counter {
        pub fn increment(&mut self, amount: Count) -> Count { amount }
    }
}
"#;
        let path = Path::new("src/metrics.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();
        let paths = symbols
            .iter()
            .map(|symbol| symbol.semantic_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "metrics",
                "metrics::Counter",
                "metrics::Count",
                "metrics::DEFAULT",
                "metrics::ACTIVE",
                "metrics::Event",
                "metrics::Render",
                "metrics::Render::render",
                "metrics::Counter::increment",
            ]
        );

        let increment = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "metrics::Counter::increment")
            .unwrap();
        assert_eq!(increment.parameters, vec!["&mut self", "amount: Count"]);
        assert_eq!(increment.return_type.as_deref(), Some("Count"));
        assert!(increment.references_by_name.is_empty());
        assert!(increment.reference_facts.is_empty());
        assert_eq!(
            increment.byte_range.0,
            source.find("pub fn increment").unwrap()
        );
        assert_eq!(
            &source[increment.byte_range.0..increment.byte_range.1],
            "pub fn increment(&mut self, amount: Count) -> Count { amount }"
        );
    }

    #[test]
    fn ignores_self_method_calls_outside_inherent_impls() {
        let source = r#"
struct Counter {}

impl Counter {
    fn increment(&self) {}
}

fn caller() {
    self.increment();
}
"#;
        let path = Path::new("src/api.rs");
        let document = parse_document(path, source).unwrap();
        let symbols =
            index_rust_symbols_with_deadline(path, source, document.tree.root_node(), None)
                .unwrap();

        let caller = symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "caller")
            .unwrap();
        assert!(caller.references_by_name.is_empty());
        assert!(caller.reference_facts.is_empty());
    }
}
