use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_php_skeleton(
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

    for node in collect_php_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("collecting PHP semantic symbols")?;
        }
        let Some(_name) = php_symbol_name(node, source)? else {
            continue;
        };
        let Some(semantic_path) = php_symbol_path(node, source)? else {
            continue;
        };
        let signature = php_signature(node, source).ok_or_else(|| {
            anyhow!("PHP semantic symbol `{semantic_path}` has an empty signature")
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
                parameters: php_parameters(node, source),
                return_type: php_return_type(node, source)?,
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
                "ambiguous PHP semantic path `{selector}`; duplicate declarations cannot be expanded safely"
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
            deadline.check("rendering PHP semantic skeleton")?;
        }
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, node))
        {
            continue;
        }

        if expand_set.contains(semantic_path.as_str()) || expand_set.contains(symbol_id.as_str()) {
            skeleton_items.push(php_full_declaration(node, source)?);
            expanded_items.push(node);
        } else {
            let signature = php_signature(node, source).ok_or_else(|| {
                anyhow!("PHP semantic symbol `{semantic_path}` has an empty signature")
            })?;
            skeleton_items.push(format!("{signature} ..."));
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("validating PHP semantic skeleton")?;
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

pub(crate) fn find_php_semantic_node<'tree>(
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

    for node in collect_php_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving PHP semantic target")?;
        }
        if php_symbol_path(node, source)?.as_deref() == Some(local_target) {
            matches.push(node);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous PHP semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_php_symbol_node(node: Node<'_>) -> bool {
    matches!(node.kind(), "function_definition" | "method_declaration")
}

pub(crate) fn php_symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    node.child_by_field_name("name")
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

pub(crate) fn php_symbol_path(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(name) = php_symbol_name(node, source)? else {
        return Ok(None);
    };
    let path = if node.kind() == "method_declaration" {
        if let Some(class_name) = php_enclosing_class_name(node, source)? {
            format!("{class_name}::{name}")
        } else {
            name
        }
    } else {
        name
    };
    if path.is_empty() {
        Ok(None)
    } else {
        Ok(Some(path))
    }
}

fn php_enclosing_class_name<'tree>(node: Node<'tree>, source: &str) -> Result<Option<String>> {
    let mut current = node.parent();
    while let Some(parent) = current {
        if parent.kind() == "class_declaration"
            && let Some(name_node) = parent.child_by_field_name("name")
        {
            let class_name = node_text(name_node, source)?.trim();
            if !class_name.is_empty() {
                return Ok(Some(class_name.to_string()));
            }
        }
        current = parent.parent();
    }
    Ok(None)
}

pub(crate) fn php_signature(node: Node<'_>, source: &str) -> Option<String> {
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

pub(crate) fn php_return_type(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(return_type) = node.child_by_field_name("return_type") else {
        return Ok(None);
    };
    let trimmed = node_text(return_type, source)?.trim().to_string();
    Ok((!trimmed.is_empty()).then_some(trimmed))
}

pub(crate) fn php_parameters(node: Node<'_>, source: &str) -> Vec<String> {
    let Some(parameters) = node.child_by_field_name("parameters") else {
        return Vec::new();
    };
    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter_map(|parameter| {
            let name_node = parameter.child_by_field_name("name")?;
            node_text(name_node, source).ok().map(str::trim)
        })
        .filter(|parameter| !parameter.is_empty())
        .map(str::to_string)
        .collect()
}

fn php_full_declaration(node: Node<'_>, source: &str) -> Result<String> {
    let declaration = node_text(node, source)?.trim();
    if declaration.is_empty() {
        bail!("PHP semantic declaration has empty source text");
    }
    Ok(declaration.to_string())
}

fn collect_php_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    fn collect<'tree>(
        node: Node<'tree>,
        deadline: Option<&dyn DeadlineCheck>,
        nodes: &mut Vec<Node<'tree>>,
    ) -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("collecting PHP semantic symbols")?;
        }
        if is_php_symbol_node(node) {
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

    use super::{build_php_skeleton, find_php_semantic_node};
    use crate::LanguageId;
    use crate::language::parse_document;

    #[test]
    fn builds_php_skeleton_for_function_and_method_declarations() {
        let source = r#"<?php
function compute(int $value): int {
    return $value + 1;
}

class Greeter {
    public function greet(string $name): string {
        return "hi " . $name;
    }
}
"#;
        let path = Path::new("sample.php");
        let document = parse_document(path, source).unwrap();
        assert_eq!(document.language_id, LanguageId::Php);
        let skeleton = build_php_skeleton(path, source, &document.tree, 10, &[], None).unwrap();
        assert_eq!(skeleton.available_paths, vec!["compute", "Greeter::greet"]);
        let compute_symbol = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "compute")
            .unwrap();
        assert_eq!(compute_symbol.parameters, vec!["$value"]);
        let found = find_php_semantic_node(path, &document.tree, source, "compute", None)
            .unwrap()
            .unwrap();
        assert_eq!(found.kind(), "function_definition");
    }
}
