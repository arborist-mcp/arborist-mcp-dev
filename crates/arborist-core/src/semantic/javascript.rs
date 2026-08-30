use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_javascript_skeleton(
    path: &Path,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<SemanticSkeleton> {
    let normalized_file_path = normalize_path(path);
    let nodes = collect_javascript_symbol_nodes(tree.root_node(), deadline)?;
    let mut expand_set: BTreeSet<String> = expand_nodes.iter().cloned().collect();
    let mut collected_items = Vec::with_capacity(nodes.len());

    for node in nodes {
        if let Some(deadline) = deadline {
            deadline.check("collecting JavaScript/TypeScript semantic symbols")?;
        }
        let Some(name) = javascript_symbol_name(node, source)? else {
            continue;
        };
        let semantic_path = javascript_semantic_path(node, source, &name)?;
        let signature = javascript_signature(node, source).ok_or_else(|| {
            anyhow!(
                "JavaScript/TypeScript semantic symbol `{semantic_path}` has an empty signature"
            )
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
                parameters: javascript_parameters(node, source),
                return_type: javascript_return_type(node, source),
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
                "ambiguous JavaScript/TypeScript semantic path `{selector}`; this adapter cannot expand duplicate declarations"
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
    drop(candidates_by_path);

    let mut symbol_items = Vec::new();
    let mut available_paths = Vec::new();
    let mut available_symbols = Vec::new();
    for (node, symbol) in collected_items {
        if javascript_semantic_depth(node, source)? > depth_limit
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
            deadline.check("rendering JavaScript/TypeScript semantic skeleton")?;
        }
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, node))
        {
            continue;
        }

        if expand_set.contains(semantic_path.as_str()) || expand_set.contains(symbol_id.as_str()) {
            skeleton_items.push(node_text(node, source)?.trim().to_string());
            expanded_items.push(node);
        } else {
            let signature = javascript_signature(node, source).ok_or_else(|| {
                anyhow!(
                    "JavaScript/TypeScript semantic symbol `{semantic_path}` has an empty signature"
                )
            })?;
            skeleton_items.push(format!("{signature} ..."));
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("validating JavaScript/TypeScript semantic skeleton")?;
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

pub(crate) fn find_javascript_semantic_node<'tree>(
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

    for node in collect_javascript_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving JavaScript/TypeScript semantic target")?;
        }
        let Some(name) = javascript_symbol_name(node, source)? else {
            continue;
        };
        if javascript_semantic_path(node, source, &name)? == local_target {
            matches.push(node);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous JavaScript/TypeScript semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_javascript_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "abstract_class_declaration"
            | "class_declaration"
            | "enum_declaration"
            | "function_declaration"
            | "generator_function_declaration"
            | "interface_declaration"
            | "method_definition"
    ) || is_javascript_callable_variable_declarator(node)
}

pub(crate) fn javascript_patch_replacement_node(node: Node<'_>) -> Node<'_> {
    let declaration = if node.kind() == "variable_declarator" {
        node.parent()
            .filter(|parent| {
                matches!(
                    parent.kind(),
                    "lexical_declaration" | "variable_declaration"
                )
            })
            .unwrap_or(node)
    } else {
        node
    };

    declaration
        .parent()
        .filter(|parent| parent.kind() == "export_statement")
        .unwrap_or(declaration)
}

pub(crate) fn javascript_symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let name = node.child_by_field_name("name");
    name.map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

pub(crate) fn javascript_semantic_path(node: Node<'_>, source: &str, name: &str) -> Result<String> {
    let mut ancestors = Vec::new();
    let mut current = node.parent();
    while let Some(parent) = current {
        if is_javascript_symbol_node(parent)
            && let Some(parent_name) = javascript_symbol_name(parent, source)?
        {
            ancestors.push(parent_name);
        } else if let Some(namespace_name) = javascript_namespace_scope_name(parent, source)? {
            ancestors.push(namespace_name);
        }
        current = parent.parent();
    }
    ancestors.reverse();
    ancestors.push(name.to_string());
    Ok(ancestors.join("::"))
}

pub(crate) fn javascript_namespace_scope_name(
    node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    if !matches!(node.kind(), "internal_module" | "module") {
        return Ok(None);
    }
    let Some(name) = node.child_by_field_name("name") else {
        return Ok(None);
    };
    javascript_namespace_name(name, source)
}

fn javascript_namespace_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "identifier" => node_text(node, source)
            .map(|name| name.trim().to_string())
            .map(|name| (!name.is_empty()).then_some(name)),
        "nested_identifier" => {
            let Some(object) = node.child_by_field_name("object") else {
                return Ok(None);
            };
            let Some(property) = node.child_by_field_name("property") else {
                return Ok(None);
            };
            let Some(object) = javascript_namespace_name(object, source)? else {
                return Ok(None);
            };
            let property = node_text(property, source)?.trim().to_string();
            Ok((!property.is_empty()).then(|| format!("{object}::{property}")))
        }
        _ => Ok(None),
    }
}

pub(crate) fn javascript_signature(node: Node<'_>, source: &str) -> Option<String> {
    let callable = javascript_callable_value(node);
    let end_byte = callable
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .unwrap_or(callable.end_byte());
    source
        .get(node.start_byte()..end_byte)
        .map(str::trim)
        .filter(|signature| !signature.is_empty())
        .map(str::to_string)
}

pub(crate) fn javascript_parameters(node: Node<'_>, source: &str) -> Vec<String> {
    let callable = javascript_callable_value(node);
    let parameters = callable
        .child_by_field_name("parameters")
        .or_else(|| callable.child_by_field_name("parameter"));
    let Some(parameters) = parameters else {
        return Vec::new();
    };
    if parameters.kind() == "identifier" {
        return node_text(parameters, source)
            .ok()
            .map(str::trim)
            .filter(|parameter| !parameter.is_empty())
            .map(|parameter| vec![parameter.to_string()])
            .unwrap_or_default();
    }

    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter_map(|parameter| node_text(parameter, source).ok().map(str::trim))
        .filter(|parameter| !parameter.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn javascript_return_type(node: Node<'_>, source: &str) -> Option<String> {
    javascript_callable_value(node)
        .child_by_field_name("return_type")
        .and_then(|return_type| node_text(return_type, source).ok())
        .map(str::trim)
        .map(|return_type| return_type.trim_start_matches(':').trim())
        .filter(|return_type| !return_type.is_empty())
        .map(str::to_string)
}

fn is_javascript_callable_variable_declarator(node: Node<'_>) -> bool {
    node.kind() == "variable_declarator"
        && node.child_by_field_name("value").is_some_and(|value| {
            matches!(
                value.kind(),
                "arrow_function" | "function_expression" | "generator_function"
            )
        })
}

fn javascript_callable_value(node: Node<'_>) -> Node<'_> {
    node.child_by_field_name("value")
        .filter(|value| {
            matches!(
                value.kind(),
                "arrow_function" | "function_expression" | "generator_function"
            )
        })
        .unwrap_or(node)
}

fn javascript_semantic_depth(node: Node<'_>, source: &str) -> Result<usize> {
    let mut depth = 0;
    let mut current = Some(node);
    while let Some(candidate) = current {
        if is_javascript_symbol_node(candidate)
            || javascript_namespace_scope_name(candidate, source)?.is_some()
        {
            depth += 1;
        }
        current = candidate.parent();
    }
    Ok(depth)
}

fn collect_javascript_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    let mut nodes = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if let Some(deadline) = deadline {
            deadline.check("collecting JavaScript/TypeScript semantic symbols")?;
        }
        if is_javascript_symbol_node(node) {
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

    use super::{build_javascript_skeleton, find_javascript_semantic_node};
    use crate::language::parse_document;

    #[test]
    fn builds_javascript_skeletons_and_expands_selected_symbols() {
        let source = "export class Counter { increment(value) { return helper(value); } }\nexport const helper = (value) => value + 1;\n";
        let path = Path::new("sample.js");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_javascript_skeleton(
            path,
            source,
            &document.tree,
            2,
            &["Counter".to_string()],
            None,
        )
        .unwrap();

        assert!(
            skeleton
                .skeleton
                .contains("class Counter { increment(value)")
        );
        assert!(!skeleton.skeleton.contains("class Counter ..."));
        assert_eq!(skeleton.skeleton.matches("increment(value)").count(), 1);
        assert_eq!(
            skeleton.available_paths,
            vec!["Counter", "Counter::increment", "helper"]
        );
        assert_eq!(
            skeleton.available_symbols[1].scope_path.as_deref(),
            Some("Counter")
        );
        assert_eq!(skeleton.available_symbols[2].parameters, vec!["value"]);
    }

    #[test]
    fn resolves_unambiguous_javascript_semantic_nodes() {
        let source = "class Counter { increment(value) { return value; } }\n";
        let path = Path::new("sample.js");
        let document = parse_document(path, source).unwrap();
        let node =
            find_javascript_semantic_node(path, &document.tree, source, "Counter::increment", None)
                .unwrap()
                .unwrap();

        assert_eq!(node.kind(), "method_definition");
    }
}
