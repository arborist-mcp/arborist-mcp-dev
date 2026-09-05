use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_swift_skeleton(
    path: &Path,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<SemanticSkeleton> {
    let normalized_file_path = normalize_path(path);
    let mut expand_set: BTreeSet<String> = expand_nodes.iter().cloned().collect();
    let mut collected_items = Vec::new();

    for node in collect_swift_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("collecting Swift semantic symbols")?;
        }
        let Some(name) = swift_symbol_name(node, source)? else {
            continue;
        };
        let Some(semantic_path) = swift_semantic_path(name.as_str())? else {
            continue;
        };
        let signature = swift_signature(node, source).ok_or_else(|| {
            anyhow!("Swift semantic symbol `{semantic_path}` has an empty signature")
        })?;
        collected_items.push((
            node,
            SemanticSkeletonSymbol {
                symbol_id: semantic_path.clone(),
                semantic_path: semantic_path.clone(),
                scope_path: semantic_parent_path(&semantic_path),
                node_kind: node.kind().to_string(),
                byte_range: (node.start_byte(), node.end_byte()),
                signature: Some(signature),
                parameters: swift_parameters(node, source),
                return_type: None,
                docstring: None,
            },
        ));
    }

    let mut candidates_by_path: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (_, symbol) in &collected_items {
        candidates_by_path
            .entry(&symbol.semantic_path)
            .or_default()
            .push(&symbol.symbol_id);
    }
    for selector in &expand_set {
        if let Some(candidates) = candidates_by_path.get(selector.as_str())
            && candidates.len() > 1
        {
            bail!(
                "ambiguous Swift semantic path `{selector}`; duplicate declarations cannot be expanded safely"
            );
        }
    }
    let file_prefix = format!("{normalized_file_path}::");
    let qualified_singletons = expand_set
        .iter()
        .filter_map(|selector| selector.strip_prefix(&file_prefix))
        .filter(|local_path| {
            candidates_by_path
                .get(*local_path)
                .is_some_and(|candidates| candidates.len() == 1)
        })
        .map(str::to_string)
        .collect::<Vec<_>>();
    expand_set.extend(qualified_singletons);

    let mut symbol_items = Vec::new();
    let mut available_paths = Vec::new();
    let mut available_symbols = Vec::new();
    for (node, symbol) in collected_items {
        if symbol.semantic_path.split("::").count() > depth_limit
            && !expand_set.contains(symbol.semantic_path.as_str())
            && !expand_set.contains(symbol.symbol_id.as_str())
        {
            continue;
        }
        available_paths.push(symbol.semantic_path.clone());
        symbol_items.push((node, symbol.semantic_path.clone(), symbol.symbol_id.clone()));
        available_symbols.push(symbol);
    }

    let mut skeleton_items = Vec::new();
    let mut expanded_items = Vec::new();
    for (node, semantic_path, symbol_id) in symbol_items {
        if let Some(deadline) = deadline {
            deadline.check("rendering Swift semantic skeleton")?;
        }
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, node))
        {
            continue;
        }

        if expand_set.contains(semantic_path.as_str()) || expand_set.contains(symbol_id.as_str()) {
            skeleton_items.push(swift_full_declaration(node, source)?);
            expanded_items.push(node);
        } else {
            let signature = swift_signature(node, source).ok_or_else(|| {
                anyhow!("Swift semantic symbol `{semantic_path}` has an empty signature")
            })?;
            skeleton_items.push(format!("{signature} ..."));
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("validating Swift semantic skeleton")?;
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

pub(crate) fn find_swift_semantic_node<'tree>(
    path: &Path,
    tree: &'tree Tree,
    source: &str,
    target_path: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<Node<'tree>>> {
    let normalized_file_path = normalize_path(path);
    let local_target = target_path
        .strip_prefix(&format!("{normalized_file_path}::"))
        .unwrap_or(target_path);
    let mut matches = Vec::new();

    for node in collect_swift_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving Swift semantic target")?;
        }
        let Some(name) = swift_symbol_name(node, source)? else {
            continue;
        };
        if swift_semantic_path(name.as_str())?.as_deref() == Some(local_target) {
            matches.push(node);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous Swift semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_swift_symbol_node(node: Node<'_>) -> bool {
    matches!(node.kind(), "function_declaration")
}

pub(crate) fn swift_symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    node.child_by_field_name("name")
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

pub(crate) fn swift_semantic_path(name: &str) -> Result<Option<String>> {
    if name.is_empty() {
        return Ok(None);
    }
    Ok(Some(name.to_string()))
}

pub(crate) fn swift_signature(node: Node<'_>, source: &str) -> Option<String> {
    let end_byte = node
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .unwrap_or(node.end_byte());
    let signature = source.get(node.start_byte()..end_byte)?.trim();
    if signature.is_empty() {
        return None;
    }
    Some(signature.to_string())
}

pub(crate) fn swift_parameters(node: Node<'_>, source: &str) -> Vec<String> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .filter(|child| child.kind() == "parameter")
        .filter_map(|parameter| {
            let name_node = parameter.child_by_field_name("name")?;
            node_text(name_node, source).ok().map(str::trim)
        })
        .filter(|parameter| !parameter.is_empty())
        .map(str::to_string)
        .collect()
}

fn swift_full_declaration(node: Node<'_>, source: &str) -> Result<String> {
    let declaration = node_text(node, source)?.trim();
    if declaration.is_empty() {
        bail!("Swift semantic declaration has empty source text");
    }
    Ok(declaration.to_string())
}

fn collect_swift_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    fn collect<'tree>(
        node: Node<'tree>,
        deadline: Option<&dyn DeadlineCheck>,
        nodes: &mut Vec<Node<'tree>>,
    ) -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("collecting Swift semantic symbols")?;
        }
        if is_swift_symbol_node(node) {
            nodes.push(node);
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, deadline, nodes)?;
        }
        Ok(())
    }

    let mut nodes = Vec::new();
    collect(root, deadline, &mut nodes)?;
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{build_swift_skeleton, find_swift_semantic_node};
    use crate::LanguageId;
    use crate::language::parse_document;

    #[test]
    fn builds_swift_skeleton_for_function_declarations() {
        let source = r#"func compute(value: Int) -> Int {
    return value + 1;
}

    func greet(name: String, greeting: String) -> String {
    return name + greeting;
}
        "#;
        let path = Path::new("sample.swift");
        let document = parse_document(path, source).unwrap();
        assert_eq!(document.language_id, LanguageId::Swift);
        let skeleton = build_swift_skeleton(path, source, &document.tree, 10, &[], None).unwrap();
        assert_eq!(skeleton.available_paths, vec!["compute", "greet"]);
        let compute_symbol = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "compute")
            .unwrap();
        assert_eq!(compute_symbol.parameters, vec!["value"]);
        let found = find_swift_semantic_node(path, &document.tree, source, "compute", None)
            .unwrap()
            .unwrap();
        assert_eq!(found.kind(), "function_declaration");
    }
}
