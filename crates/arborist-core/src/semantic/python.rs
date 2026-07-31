use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Query, QueryCursor, StreamingIterator, Tree};

use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, language_for_id, node_text, normalize_path};
use crate::model::{LanguageId, SemanticSkeleton, SemanticSkeletonSymbol};

use super::{
    PythonSymbolIdentity, python_symbol_ids, semantic_depth, semantic_parent_path, semantic_path,
};

fn python_display_node(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == "decorated_definition")
        .unwrap_or(node)
}

pub(crate) fn python_display_byte_range(node: Node<'_>) -> (usize, usize) {
    let display_node = python_display_node(node);
    (display_node.start_byte(), display_node.end_byte())
}

#[derive(Debug)]
struct PythonOverloadBinding {
    end_byte: usize,
    active: bool,
}

#[derive(Debug)]
pub(crate) struct PythonOverloadNames(BTreeMap<String, Vec<PythonOverloadBinding>>);

impl PythonOverloadNames {
    fn contains_before(&self, name: &str, byte_offset: usize) -> bool {
        self.0
            .get(name)
            .and_then(|bindings| {
                bindings
                    .iter()
                    .rev()
                    .find(|binding| binding.end_byte <= byte_offset)
            })
            .is_some_and(|binding| binding.active)
    }
}

fn python_add_overload_binding(
    names: &mut BTreeMap<String, Vec<PythonOverloadBinding>>,
    name: String,
    end_byte: usize,
    active: bool,
) {
    names
        .entry(name)
        .or_default()
        .push(PythonOverloadBinding { end_byte, active });
}

fn python_collect_pattern_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut BTreeSet<String>,
) -> Result<()> {
    match node.kind() {
        "identifier" | "keyword_identifier" => {
            let name = node_text(node, source)?.trim();
            if name != "_" {
                bindings.insert(name.to_string());
            }
        }
        "attribute" | "subscript" => {}
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                python_collect_pattern_bindings(child, source, bindings)?;
            }
        }
    }
    Ok(())
}

fn python_module_binding_names(statement: Node<'_>, source: &str) -> Result<BTreeSet<String>> {
    let mut bindings = BTreeSet::new();
    match statement.kind() {
        "assignment" | "augmented_assignment" | "for_statement" => {
            if let Some(left) = statement.child_by_field_name("left") {
                python_collect_pattern_bindings(left, source, &mut bindings)?;
            }
        }
        "expression_statement" => {
            let mut cursor = statement.walk();
            for child in statement.named_children(&mut cursor) {
                match child.kind() {
                    "assignment" | "augmented_assignment" => {
                        if let Some(left) = child.child_by_field_name("left") {
                            python_collect_pattern_bindings(left, source, &mut bindings)?;
                        }
                    }
                    "named_expression" => {
                        if let Some(name) = child.child_by_field_name("name") {
                            python_collect_pattern_bindings(name, source, &mut bindings)?;
                        }
                    }
                    _ => {}
                }
            }
        }
        "delete_statement" => {
            let mut cursor = statement.walk();
            for child in statement.named_children(&mut cursor) {
                python_collect_pattern_bindings(child, source, &mut bindings)?;
            }
        }
        "function_definition" | "class_definition" => {
            if let Some(name) = statement.child_by_field_name("name") {
                bindings.insert(node_text(name, source)?.trim().to_string());
            }
        }
        "decorated_definition" => {
            if let Some(definition) = statement.child_by_field_name("definition")
                && let Some(name) = definition.child_by_field_name("name")
            {
                bindings.insert(node_text(name, source)?.trim().to_string());
            }
        }
        "type_alias_statement" => {
            if let Some(left) = statement.child_by_field_name("left") {
                python_collect_pattern_bindings(left, source, &mut bindings)?;
            }
        }
        "import_statement" | "import_from_statement" => {
            let mut cursor = statement.walk();
            let children = statement.named_children(&mut cursor).collect::<Vec<_>>();
            let import_start = usize::from(statement.kind() == "import_from_statement");
            for child in children.into_iter().skip(import_start) {
                let binding = if child.kind() == "aliased_import" {
                    let mut alias_cursor = child.walk();
                    child
                        .named_children(&mut alias_cursor)
                        .last()
                        .map(|alias| node_text(alias, source).map(str::to_string))
                        .transpose()?
                } else if matches!(child.kind(), "dotted_name" | "identifier") {
                    node_text(child, source)?
                        .split('.')
                        .next()
                        .map(str::to_string)
                } else {
                    None
                };
                if let Some(binding) = binding {
                    bindings.insert(binding.trim().to_string());
                }
            }
        }
        _ => {}
    }
    Ok(bindings)
}

fn python_typing_overload_import_aliases(
    statement: Node<'_>,
    source: &str,
) -> Result<BTreeSet<String>> {
    if statement.kind() != "import_from_statement" {
        return Ok(BTreeSet::new());
    }
    let mut cursor = statement.walk();
    let children = statement.named_children(&mut cursor).collect::<Vec<_>>();
    let Some(module) = children.first() else {
        return Ok(BTreeSet::new());
    };
    if !matches!(
        node_text(*module, source)?.trim(),
        "typing" | "typing_extensions"
    ) {
        return Ok(BTreeSet::new());
    }

    let mut aliases = BTreeSet::new();
    for imported in children.into_iter().skip(1) {
        match imported.kind() {
            "aliased_import" => {
                let mut alias_cursor = imported.walk();
                let alias_children = imported
                    .named_children(&mut alias_cursor)
                    .collect::<Vec<_>>();
                if alias_children.len() >= 2
                    && node_text(alias_children[0], source)?.trim() == "overload"
                    && let Some(alias) = alias_children.last()
                {
                    aliases.insert(node_text(*alias, source)?.trim().to_string());
                }
            }
            "identifier" | "dotted_name" if node_text(imported, source)?.trim() == "overload" => {
                aliases.insert("overload".to_string());
            }
            _ => {}
        }
    }
    Ok(aliases)
}

pub(crate) fn python_overload_names(
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<PythonOverloadNames> {
    let mut names = BTreeMap::new();
    let mut tracked_names = BTreeSet::new();
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check("collecting Python overload aliases")?;
        }
        let aliases = python_typing_overload_import_aliases(statement, source)?;
        for alias in &aliases {
            if let Some(deadline) = deadline {
                deadline.check("collecting Python overload aliases")?;
            }
            tracked_names.insert(alias.clone());
            python_add_overload_binding(&mut names, alias.clone(), statement.end_byte(), true);
        }
        if tracked_names.is_empty() {
            continue;
        }
        for binding in python_module_binding_names(statement, source)? {
            if tracked_names.contains(&binding) && !aliases.contains(&binding) {
                python_add_overload_binding(&mut names, binding, statement.end_byte(), false);
            }
        }
    }
    Ok(PythonOverloadNames(names))
}

pub(crate) fn python_is_overload(
    node: Node<'_>,
    source: &str,
    overload_names: &PythonOverloadNames,
) -> bool {
    let Some(parent) = node
        .parent()
        .filter(|parent| parent.kind() == "decorated_definition")
    else {
        return false;
    };

    let mut cursor = parent.walk();
    parent.named_children(&mut cursor).any(|child| {
        child.kind() == "decorator"
            && node_text(child, source).ok().is_some_and(|text| {
                let decorator = text
                    .trim()
                    .strip_prefix('@')
                    .unwrap_or_default()
                    .trim_start();
                let name = decorator.split(['(', ' ', '\t']).next().unwrap_or_default();
                name.rsplit('.').next() == Some("overload")
                    || overload_names.contains_before(name, node.start_byte())
            })
    })
}

pub(crate) fn python_display_header(node: Node<'_>, source: &str) -> Result<String> {
    let body = node
        .child_by_field_name("body")
        .ok_or_else(|| anyhow!("python symbol missing body"))?;
    let display_node = python_display_node(node);
    Ok(source[display_node.start_byte()..body.start_byte()]
        .trim_end()
        .to_string())
}

pub(crate) fn python_docstring(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(body) = node.child_by_field_name("body") else {
        return Ok(None);
    };
    let Some(first_statement) = body.named_child(0) else {
        return Ok(None);
    };
    if first_statement.kind() != "expression_statement" {
        return Ok(None);
    }

    let Some(first_expr) = first_statement.named_child(0) else {
        return Ok(None);
    };
    if !matches!(first_expr.kind(), "string" | "concatenated_string") {
        return Ok(None);
    }

    Ok(Some(node_text(first_expr, source)?.trim().to_string()))
}

pub(crate) fn python_parameters(node: Node<'_>, source: &str) -> Result<Vec<String>> {
    let Some(parameters) = node.child_by_field_name("parameters") else {
        return Ok(Vec::new());
    };

    let mut cursor = parameters.walk();
    let mut values = Vec::new();
    for child in parameters.named_children(&mut cursor) {
        values.push(node_text(child, source)?.trim().to_string());
    }
    Ok(values)
}

pub(crate) fn python_return_type(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(return_type) = node.child_by_field_name("return_type") else {
        return Ok(None);
    };

    Ok(Some(node_text(return_type, source)?.trim().to_string()))
}

pub(super) fn build_python_skeleton(
    path: &Path,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<SemanticSkeleton> {
    let language = language_for_id(LanguageId::Python);
    let query = Query::new(
        &language,
        r#"
        (class_definition
            name: (identifier) @name
            body: (block) @body) @item

        (function_definition
            name: (identifier) @name
            body: (block) @body) @item
        "#,
    )?;

    let mut cursor = QueryCursor::new();
    let normalized_file_path = normalize_path(path);
    let overload_names = python_overload_names(tree.root_node(), source, deadline)?;
    let mut collected_items = Vec::new();
    let mut expand_set: BTreeSet<String> = expand_nodes.iter().cloned().collect();

    let mut matches = cursor.matches(&query, tree.root_node(), source.as_bytes());
    while let Some(query_match) = matches.next() {
        if let Some(deadline) = deadline {
            deadline.check("collecting Python semantic symbols")?;
        }
        let mut item_node = None;

        for capture in query_match.captures.iter() {
            if let Some(deadline) = deadline {
                deadline.check("collecting Python semantic captures")?;
            }
            let capture_name = &query.capture_names()[capture.index as usize];
            if *capture_name == "item" {
                item_node = Some(capture.node);
            }
        }

        let Some(item) = item_node else {
            continue;
        };
        let path = semantic_path(item, source)?;
        collected_items.push((
            item,
            SemanticSkeletonSymbol {
                symbol_id: path.clone(),
                semantic_path: path.clone(),
                scope_path: semantic_parent_path(&path),
                node_kind: item.kind().to_string(),
                byte_range: python_display_byte_range(item),
                signature: Some(python_display_header(item, source)?),
                parameters: python_parameters(item, source)?,
                return_type: python_return_type(item, source)?,
                docstring: python_docstring(item, source)?,
            },
        ));
    }

    let identity_entries = collected_items
        .iter()
        .map(|(item, symbol)| PythonSymbolIdentity {
            file_path: &normalized_file_path,
            semantic_path: &symbol.semantic_path,
            is_overload: python_is_overload(*item, source, &overload_names),
            byte_range: symbol.byte_range,
        })
        .collect::<Vec<_>>();
    let resolved_ids = python_symbol_ids(&identity_entries);
    drop(identity_entries);
    for ((_, symbol), resolved_id) in collected_items.iter_mut().zip(resolved_ids) {
        symbol.symbol_id = resolved_id;
    }

    let mut candidate_ids_by_path: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (_, symbol) in &collected_items {
        candidate_ids_by_path
            .entry(&symbol.semantic_path)
            .or_default()
            .push(&symbol.symbol_id);
    }
    for selector in &expand_set {
        if let Some(candidates) = candidate_ids_by_path.get(selector.as_str())
            && candidates.len() > 1
        {
            bail!(
                "ambiguous Python semantic path `{selector}`; use one of these symbol_id candidates: {}",
                candidates.join(", ")
            );
        }
    }
    let file_prefix = format!("{normalized_file_path}::");
    let qualified_singletons = expand_set
        .iter()
        .filter_map(|selector| selector.strip_prefix(&file_prefix))
        .filter(|local_path| {
            candidate_ids_by_path
                .get(*local_path)
                .is_some_and(|candidates| candidates.len() == 1)
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    expand_set.extend(qualified_singletons);
    drop(candidate_ids_by_path);

    let mut symbol_items = Vec::new();
    let mut available_paths = Vec::new();
    let mut available_symbols = Vec::new();
    for (item, symbol) in collected_items {
        if semantic_depth(item) > depth_limit
            && !expand_set.contains(symbol.semantic_path.as_str())
            && !expand_set.contains(symbol.symbol_id.as_str())
        {
            continue;
        }
        available_paths.push(symbol.semantic_path.clone());
        symbol_items.push((item, symbol.semantic_path.clone(), symbol.symbol_id.clone()));
        available_symbols.push(symbol);
    }

    let mut skeleton_items = Vec::new();
    let mut expanded_items = Vec::new();
    for (item, path, symbol_id) in symbol_items {
        if let Some(deadline) = deadline {
            deadline.check("rendering Python semantic skeleton")?;
        }
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, item))
        {
            continue;
        }

        let display_item = python_display_node(item);
        if expand_set.contains(path.as_str()) || expand_set.contains(symbol_id.as_str()) {
            skeleton_items.push(node_text(display_item, source)?.trim().to_string());
            expanded_items.push(item);
        } else {
            let header = python_display_header(item, source)?;
            skeleton_items.push(format!("{header} ..."));
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("validating Python semantic skeleton")?;
    }
    let result = SemanticSkeleton {
        file: normalized_file_path,
        skeleton: skeleton_items.join("\n\n"),
        available_paths,
        available_symbols,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(super) fn find_python_semantic_node<'tree>(
    path: &Path,
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<Node<'tree>>> {
    let nodes = collect_python_symbol_nodes(tree.root_node(), deadline)?;
    let overload_names = python_overload_names(tree.root_node(), source, deadline)?;
    let mut paths = Vec::with_capacity(nodes.len());
    let mut ranges = Vec::with_capacity(nodes.len());
    for node in &nodes {
        if let Some(deadline) = deadline {
            deadline.check("resolving Python semantic target")?;
        }
        paths.push(semantic_path(*node, source)?);
        ranges.push(python_display_byte_range(*node));
    }
    let normalized_file_path = normalize_path(path);
    let entries = nodes
        .iter()
        .zip(paths.iter())
        .zip(ranges.iter())
        .map(|((node, path), byte_range)| PythonSymbolIdentity {
            file_path: &normalized_file_path,
            semantic_path: path,
            is_overload: python_is_overload(*node, source, &overload_names),
            byte_range: *byte_range,
        })
        .collect::<Vec<_>>();
    let symbol_ids = python_symbol_ids(&entries);

    if let Some(index) = symbol_ids
        .iter()
        .position(|symbol_id| symbol_id == target_path)
    {
        return Ok(Some(nodes[index]));
    }
    if let Some(local_path) = target_path.strip_prefix(&format!("{normalized_file_path}::")) {
        let local_candidates = paths
            .iter()
            .enumerate()
            .filter_map(|(index, path)| (path == local_path).then_some(index))
            .collect::<Vec<_>>();
        if let [index] = local_candidates.as_slice() {
            return Ok(Some(nodes[*index]));
        }
    }
    let candidates = paths
        .iter()
        .enumerate()
        .filter_map(|(index, path)| (path == target_path).then_some(index))
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [] => Ok(None),
        [index] => Ok(Some(nodes[*index])),
        _ => {
            let candidate_ids = candidates
                .iter()
                .map(|index| symbol_ids[*index].as_str())
                .collect::<Vec<_>>()
                .join(", ");
            bail!(
                "ambiguous Python semantic path `{target_path}`; use one of these symbol_id candidates: {candidate_ids}"
            )
        }
    }
}

pub(crate) fn python_symbol_id_for_node(
    path: &Path,
    node: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<String> {
    let mut root = node;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    let normalized_file_path = normalize_path(path);
    let nodes = collect_python_symbol_nodes(root, deadline)?;
    let overload_names = python_overload_names(root, source, deadline)?;
    let mut entries = Vec::with_capacity(nodes.len());
    let mut paths = Vec::with_capacity(nodes.len());
    for candidate in &nodes {
        let path = semantic_path(*candidate, source)?;
        paths.push(path);
    }
    for (candidate, path) in nodes.iter().zip(paths.iter()) {
        entries.push(PythonSymbolIdentity {
            file_path: &normalized_file_path,
            semantic_path: path,
            is_overload: python_is_overload(*candidate, source, &overload_names),
            byte_range: python_display_byte_range(*candidate),
        });
    }
    let ids = python_symbol_ids(&entries);
    let target_range = python_display_byte_range(node);
    nodes
        .iter()
        .position(|candidate| python_display_byte_range(*candidate) == target_range)
        .map(|index| ids[index].clone())
        .ok_or_else(|| {
            anyhow!(
                "Python symbol identity not found for {}",
                semantic_path(node, source).unwrap_or_default()
            )
        })
}

fn collect_python_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    let mut nodes = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if let Some(deadline) = deadline {
            deadline.check("resolving Python semantic target")?;
        }
        if matches!(node.kind(), "class_definition" | "function_definition") {
            nodes.push(node);
        }
        let mut cursor = node.walk();
        let children = node.named_children(&mut cursor).collect::<Vec<_>>();
        pending.extend(children.into_iter().rev());
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use anyhow::{Result, bail};

    use super::python_overload_names;
    use crate::deadline::DeadlineCheck;
    use crate::language::parse_document;

    struct RejectDeadlineChecks;

    impl DeadlineCheck for RejectDeadlineChecks {
        fn check(&self, phase: &str) -> Result<()> {
            bail!("deadline check reached {phase}")
        }
    }

    #[test]
    fn overload_alias_collection_checks_deadlines_before_scanning_imports() {
        let source = "from typing import overload as typed_overload\n";
        let document = parse_document(Path::new("sample.py"), source).unwrap();

        let error = python_overload_names(
            document.tree.root_node(),
            source,
            Some(&RejectDeadlineChecks),
        )
        .expect_err("deadline must be checked before scanning overload aliases");

        assert!(
            error
                .to_string()
                .contains("collecting Python overload aliases")
        );
    }
}
