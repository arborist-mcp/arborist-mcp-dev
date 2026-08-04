use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_java_skeleton(
    path: &Path,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<SemanticSkeleton> {
    let normalized_file_path = normalize_path(path);
    let expand_set: BTreeSet<String> = expand_nodes.iter().cloned().collect();
    let mut collected_items = Vec::new();

    for node in collect_java_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("collecting Java semantic symbols")?;
        }
        let Some(name) = java_symbol_name(node, source)? else {
            continue;
        };
        let Some(semantic_path) = java_semantic_path(tree.root_node(), node, source, &name)? else {
            continue;
        };
        let signature = java_signature(node, source).ok_or_else(|| {
            anyhow!("Java semantic symbol `{semantic_path}` has an empty signature")
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
                parameters: java_parameters(node, source),
                return_type: java_return_type(node, source),
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
                "ambiguous Java semantic path `{selector}`; duplicate declarations cannot be expanded safely"
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
    let mut expand_set = expand_set;
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
            deadline.check("rendering Java semantic skeleton")?;
        }
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, node))
        {
            continue;
        }

        if expand_set.contains(semantic_path.as_str()) || expand_set.contains(symbol_id.as_str()) {
            skeleton_items.push(java_full_declaration(node, source)?);
            expanded_items.push(node);
        } else {
            let signature = java_signature(node, source).ok_or_else(|| {
                anyhow!("Java semantic symbol `{semantic_path}` has an empty signature")
            })?;
            skeleton_items.push(format!("{signature} ..."));
        }
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

pub(crate) fn find_java_semantic_node<'tree>(
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

    for node in collect_java_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving Java semantic target")?;
        }
        let Some(name) = java_symbol_name(node, source)? else {
            continue;
        };
        if java_semantic_path(tree.root_node(), node, source, &name)?.as_deref()
            == Some(local_target)
        {
            matches.push(node);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous Java semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_java_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "annotation_type_declaration"
            | "class_declaration"
            | "enum_declaration"
            | "interface_declaration"
            | "method_declaration"
            | "constructor_declaration"
            | "record_declaration"
    )
}

pub(crate) fn java_symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    node.child_by_field_name("name")
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

pub(crate) fn java_semantic_path(
    root: Node<'_>,
    node: Node<'_>,
    source: &str,
    name: &str,
) -> Result<Option<String>> {
    let mut parts = Vec::new();
    if let Some(package) = java_package_name(root, source)? {
        parts.extend(package.split('.').map(str::to_string));
    }

    let mut ancestors = Vec::new();
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "annotation_type_declaration"
                | "class_declaration"
                | "enum_declaration"
                | "interface_declaration"
                | "record_declaration"
        ) && let Some(ancestor_name) = java_symbol_name(candidate, source)?
        {
            ancestors.push(ancestor_name);
        }
        current = candidate.parent();
    }
    ancestors.reverse();
    parts.extend(ancestors);
    parts.push(name.to_string());
    Ok((!parts.is_empty()).then(|| parts.join("::")))
}

pub(crate) fn java_signature(node: Node<'_>, source: &str) -> Option<String> {
    let end_byte = node
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .unwrap_or(node.end_byte());
    let signature = source.get(node.start_byte()..end_byte)?.trim();
    (!signature.is_empty()).then(|| signature.to_string())
}

pub(crate) fn java_parameters(node: Node<'_>, source: &str) -> Vec<String> {
    let Some(parameters) = node.child_by_field_name("parameters") else {
        return Vec::new();
    };
    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter_map(|parameter| node_text(parameter, source).ok().map(str::trim))
        .filter(|parameter| !parameter.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn java_return_type(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("type")
        .and_then(|type_node| node_text(type_node, source).ok())
        .map(str::trim)
        .filter(|return_type| !return_type.is_empty())
        .map(str::to_string)
}

fn java_full_declaration(node: Node<'_>, source: &str) -> Result<String> {
    Ok(node_text(node, source)?.trim().to_string())
}

fn java_package_name(root: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut cursor = root.walk();
    let Some(package) = root
        .named_children(&mut cursor)
        .find(|node| node.kind() == "package_declaration")
    else {
        return Ok(None);
    };
    let mut cursor = package.walk();
    let name = package
        .named_children(&mut cursor)
        .find(|node| matches!(node.kind(), "identifier" | "scoped_identifier"));
    name.map(|node| node_text(node, source).map(|text| text.trim().to_string()))
        .transpose()
}

fn collect_java_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    fn collect<'tree>(
        node: Node<'tree>,
        deadline: Option<&dyn DeadlineCheck>,
        nodes: &mut Vec<Node<'tree>>,
    ) -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("collecting Java semantic symbols")?;
        }
        if is_java_symbol_node(node) {
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

    use super::{build_java_skeleton, find_java_semantic_node};
    use crate::language::parse_document;

    #[test]
    fn builds_java_skeletons_for_packages_types_methods_and_constructors() {
        let source = r#"
package com.example;

public class Counter {
    public Counter(int initial) {}
    public int increment(int amount) { return amount; }
}
interface Renderer { String render(); }
enum Kind { BASIC }
"#;
        let path = Path::new("Counter.java");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_java_skeleton(path, source, &document.tree, 4, &[], None).unwrap();

        assert_eq!(
            skeleton.available_paths,
            vec![
                "com::example::Counter",
                "com::example::Counter::Counter",
                "com::example::Counter::increment",
                "com::example::Renderer",
                "com::example::Renderer::render",
                "com::example::Kind",
            ]
        );
        assert!(skeleton.skeleton.contains("public class Counter"));
        assert!(skeleton.skeleton.contains("int increment(int amount) ..."));

        let found = find_java_semantic_node(
            path,
            &document.tree,
            source,
            "Counter.java::com::example::Counter::increment",
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.kind(), "method_declaration");
    }
}
