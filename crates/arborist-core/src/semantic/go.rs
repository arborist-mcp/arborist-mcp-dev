use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_go_skeleton(
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

    for node in collect_go_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("collecting Go semantic symbols")?;
        }
        let Some(name) = go_symbol_name(node, source)? else {
            continue;
        };
        let Some(semantic_path) = go_semantic_path(node, source, &name)? else {
            continue;
        };
        let signature = go_signature(node, source).ok_or_else(|| {
            anyhow!("Go semantic symbol `{semantic_path}` has an empty signature")
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
                parameters: go_parameters(node, source),
                return_type: go_return_type(node, source),
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
                "ambiguous Go semantic path `{selector}`; duplicate declarations cannot be expanded safely"
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
            deadline.check("rendering Go semantic skeleton")?;
        }
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, node))
        {
            continue;
        }

        if expand_set.contains(semantic_path.as_str()) || expand_set.contains(symbol_id.as_str()) {
            skeleton_items.push(go_full_declaration(node, source)?);
            expanded_items.push(node);
        } else {
            let signature = go_signature(node, source).ok_or_else(|| {
                anyhow!("Go semantic symbol `{semantic_path}` has an empty signature")
            })?;
            skeleton_items.push(format!("{signature} ..."));
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("validating Go semantic skeleton")?;
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

pub(crate) fn find_go_semantic_node<'tree>(
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

    for node in collect_go_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving Go semantic target")?;
        }
        let Some(name) = go_symbol_name(node, source)? else {
            continue;
        };
        if go_semantic_path(node, source, &name)?.as_deref() == Some(local_target) {
            matches.push(node);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous Go semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_go_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_declaration" | "method_declaration" | "method_elem" | "type_alias" | "type_spec"
    )
}

pub(crate) fn go_symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    node.child_by_field_name("name")
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

pub(crate) fn go_semantic_path(node: Node<'_>, source: &str, name: &str) -> Result<Option<String>> {
    match node.kind() {
        "method_declaration" => Ok(
            go_method_receiver_name(node, source)?.map(|receiver| format!("{receiver}::{name}"))
        ),
        "method_elem" => {
            Ok(go_interface_method_owner_name(node, source)?
                .map(|owner| format!("{owner}::{name}")))
        }
        _ => Ok(Some(name.to_string())),
    }
}

pub(crate) fn go_signature(node: Node<'_>, source: &str) -> Option<String> {
    let end_byte = node
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .unwrap_or(node.end_byte());
    let signature = source.get(node.start_byte()..end_byte)?.trim();
    if signature.is_empty() {
        return None;
    }
    if matches!(node.kind(), "type_alias" | "type_spec") {
        Some(format!("type {signature}"))
    } else {
        Some(signature.to_string())
    }
}

pub(crate) fn go_parameters(node: Node<'_>, source: &str) -> Vec<String> {
    if !matches!(
        node.kind(),
        "function_declaration" | "method_declaration" | "method_elem"
    ) {
        return Vec::new();
    }
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

pub(crate) fn go_return_type(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(
        node.kind(),
        "function_declaration" | "method_declaration" | "method_elem"
    ) {
        return None;
    }
    node.child_by_field_name("result")
        .and_then(|result| node_text(result, source).ok())
        .map(str::trim)
        .filter(|result| !result.is_empty())
        .map(str::to_string)
}

pub(crate) fn go_patch_replacement_node<'tree>(node: Node<'tree>) -> Node<'tree> {
    // Go symbol nodes are complete declarations: a `function_declaration` or
    // `method_declaration` includes its `func` keyword, receiver, signature, and body, while
    // `type_spec`/`type_alias` nodes exclude the enclosing `type` keyword. The normalize step
    // handles that keyword, and doc comments are separate sibling nodes that the splice range
    // leaves untouched, so returning the symbol node preserves comments and surrounding
    // declarations.
    node
}

fn go_full_declaration(node: Node<'_>, source: &str) -> Result<String> {
    let declaration = node_text(node, source)?.trim();
    if declaration.is_empty() {
        bail!("Go semantic declaration has empty source text");
    }
    Ok(if matches!(node.kind(), "type_alias" | "type_spec") {
        format!("type {declaration}")
    } else {
        declaration.to_string()
    })
}

fn go_interface_method_owner_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(interface_type) = node
        .parent()
        .filter(|parent| parent.kind() == "interface_type")
    else {
        return Ok(None);
    };
    let Some(type_spec) = interface_type
        .parent()
        .filter(|parent| matches!(parent.kind(), "type_spec" | "type_alias"))
    else {
        return Ok(None);
    };
    let Some(name) = type_spec.child_by_field_name("name") else {
        return Ok(None);
    };
    let name = node_text(name, source)?.trim();
    Ok((!name.is_empty()).then(|| name.to_string()))
}

fn go_method_receiver_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(receiver) = node.child_by_field_name("receiver") else {
        return Ok(None);
    };
    let mut cursor = receiver.walk();
    let Some(parameter) = receiver.named_children(&mut cursor).next() else {
        return Ok(None);
    };
    let Some(receiver_type) = parameter.child_by_field_name("type") else {
        return Ok(None);
    };
    go_named_receiver_type(receiver_type, source)
}

fn go_named_receiver_type(node: Node<'_>, source: &str) -> Result<Option<String>> {
    match node.kind() {
        "type_identifier" => node_text(node, source)
            .map(str::trim)
            .map(str::to_string)
            .map(Some),
        "generic_type" => node
            .child_by_field_name("type")
            .map(|inner| go_named_receiver_type(inner, source))
            .transpose()
            .map(Option::flatten),
        "pointer_type" | "parenthesized_type" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .next()
                .map(|inner| go_named_receiver_type(inner, source))
                .transpose()
                .map(Option::flatten)
        }
        _ => Ok(None),
    }
}

fn collect_go_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    fn collect<'tree>(
        node: Node<'tree>,
        deadline: Option<&dyn DeadlineCheck>,
        nodes: &mut Vec<Node<'tree>>,
    ) -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("collecting Go semantic symbols")?;
        }
        if is_go_symbol_node(node) {
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

    use super::{build_go_skeleton, find_go_semantic_node};
    use crate::language::parse_document;

    #[test]
    fn builds_go_skeletons_for_named_types_functions_and_methods() {
        let source = r#"
package metrics

type Counter struct { value int }
type Renderer interface { Render() string }
type Alias = Counter

func NewCounter(value int) Counter { return Counter{value: value} }
func (counter *Counter) Increment(amount int) int { return counter.value + amount }
"#;
        let path = Path::new("metrics.go");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_go_skeleton(path, source, &document.tree, 2, &[], None).unwrap();

        assert_eq!(
            skeleton.available_paths,
            vec![
                "Counter",
                "Renderer",
                "Renderer::Render",
                "Alias",
                "NewCounter",
                "Counter::Increment"
            ]
        );
        let render = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Renderer::Render")
            .unwrap();
        assert_eq!(render.parameters, Vec::<String>::new());
        assert_eq!(render.return_type.as_deref(), Some("string"));
        let increment = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "Counter::Increment")
            .unwrap();
        assert_eq!(increment.parameters, vec!["amount int"]);
        assert_eq!(increment.return_type.as_deref(), Some("int"));
        assert!(
            skeleton
                .skeleton
                .contains("func (counter *Counter) Increment(amount int) int ...")
        );

        let found = find_go_semantic_node(
            path,
            &document.tree,
            source,
            "metrics.go::Counter::Increment",
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.kind(), "method_declaration");
    }

    #[test]
    fn skips_methods_without_a_named_local_receiver_type() {
        let source = r#"
package metrics

func (value []int) Invalid() {}
"#;
        let path = Path::new("metrics.go");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_go_skeleton(path, source, &document.tree, 2, &[], None).unwrap();

        assert!(skeleton.available_paths.is_empty());
    }
}
