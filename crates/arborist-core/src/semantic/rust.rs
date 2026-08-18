use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_rust_skeleton(
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

    for node in collect_rust_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("collecting Rust semantic symbols")?;
        }
        let Some(name) = rust_symbol_name(node, source)? else {
            continue;
        };
        let Some(semantic_path) = rust_semantic_path(node, source, &name)? else {
            continue;
        };
        let signature = rust_signature(node, source).ok_or_else(|| {
            anyhow!("Rust semantic symbol `{semantic_path}` has an empty signature")
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
                parameters: rust_parameters(node, source),
                return_type: rust_return_type(node, source),
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
                "ambiguous Rust semantic path `{selector}`; duplicate declarations cannot be expanded safely"
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
            deadline.check("rendering Rust semantic skeleton")?;
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
            let signature = rust_signature(node, source).ok_or_else(|| {
                anyhow!("Rust semantic symbol `{semantic_path}` has an empty signature")
            })?;
            skeleton_items.push(format!("{signature} ..."));
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("validating Rust semantic skeleton")?;
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

pub(crate) fn find_rust_semantic_node<'tree>(
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

    for node in collect_rust_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving Rust semantic target")?;
        }
        let Some(name) = rust_symbol_name(node, source)? else {
            continue;
        };
        if rust_semantic_path(node, source, &name)?.as_deref() == Some(local_target) {
            matches.push(node);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous Rust semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_rust_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "const_item"
            | "enum_item"
            | "function_item"
            | "function_signature_item"
            | "mod_item"
            | "static_item"
            | "struct_item"
            | "trait_item"
            | "type_item"
    )
}

pub(crate) fn rust_symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    node.child_by_field_name("name")
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

pub(crate) fn rust_semantic_path(
    node: Node<'_>,
    source: &str,
    name: &str,
) -> Result<Option<String>> {
    let mut ancestors = Vec::new();
    let mut current = node.parent();
    while let Some(parent) = current {
        if is_rust_symbol_node(parent) {
            if let Some(parent_name) = rust_symbol_name(parent, source)? {
                ancestors.push(parent_name);
            }
        } else if parent.kind() == "impl_item" {
            let Some(scope_name) = rust_inherent_impl_scope_name(parent, source)? else {
                return Ok(None);
            };
            ancestors.push(scope_name);
        }
        current = parent.parent();
    }
    ancestors.reverse();
    ancestors.push(name.to_string());
    Ok(Some(ancestors.join("::")))
}

pub(crate) fn rust_signature(node: Node<'_>, source: &str) -> Option<String> {
    let end_byte = node
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .unwrap_or(node.end_byte());
    source
        .get(node.start_byte()..end_byte)
        .map(str::trim)
        .filter(|signature| !signature.is_empty())
        .map(str::to_string)
}

pub(crate) fn rust_parameters(node: Node<'_>, source: &str) -> Vec<String> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") {
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

pub(crate) fn rust_return_type(node: Node<'_>, source: &str) -> Option<String> {
    if !matches!(node.kind(), "function_item" | "function_signature_item") {
        return None;
    }
    node.child_by_field_name("return_type")
        .and_then(|return_type| node_text(return_type, source).ok())
        .map(str::trim)
        .map(|return_type| return_type.trim_start_matches("->").trim())
        .filter(|return_type| !return_type.is_empty())
        .map(str::to_string)
}

pub(crate) fn rust_patch_replacement_node<'tree>(node: Node<'tree>) -> Node<'tree> {
    // Rust semantic symbol nodes are complete items: visibility, qualifiers, signature, and
    // body are part of the same node, while attribute items are separate sibling statements
    // outside the item's byte range. Replacing the symbol node therefore preserves both
    // attributes and any enclosing `impl` block or module structure.
    node
}

pub(crate) fn rust_inherent_impl_scope_name(
    node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    if node.child_by_field_name("trait").is_some() {
        return Ok(None);
    }
    let Some(implemented_type) = node.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_text = node_text(implemented_type, source)?.trim();
    let base_name = type_text.split('<').next().unwrap_or(type_text).trim();
    if is_ascii_rust_identifier(base_name) {
        Ok(Some(base_name.to_string()))
    } else {
        Ok(None)
    }
}

fn is_ascii_rust_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(character) if character == '_' || character.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn collect_rust_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    fn collect<'tree>(
        node: Node<'tree>,
        deadline: Option<&dyn DeadlineCheck>,
        nodes: &mut Vec<Node<'tree>>,
    ) -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("collecting Rust semantic symbols")?;
        }
        if is_rust_symbol_node(node) {
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

    use super::{build_rust_skeleton, find_rust_semantic_node};
    use crate::language::parse_document;

    #[test]
    fn builds_rust_skeletons_and_uses_inherent_impl_scope_for_methods() {
        let source = r#"
pub mod metrics {
    pub struct Counter;

    impl Counter {
        pub fn increment(&mut self, amount: u64) -> u64 { self.value() + amount }
    }

    pub trait Render {
        fn render(&self) -> String;
    }
}
"#;
        let path = Path::new("src/metrics.rs");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_rust_skeleton(path, source, &document.tree, 3, &[], None).unwrap();

        assert_eq!(
            skeleton.available_paths,
            vec![
                "metrics",
                "metrics::Counter",
                "metrics::Counter::increment",
                "metrics::Render",
                "metrics::Render::render",
            ]
        );
        let increment = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "metrics::Counter::increment")
            .unwrap();
        assert_eq!(increment.parameters, vec!["&mut self", "amount: u64"]);
        assert_eq!(increment.return_type.as_deref(), Some("u64"));
        assert!(
            skeleton
                .skeleton
                .contains("pub fn increment(&mut self, amount: u64) -> u64 ...")
        );

        let found = find_rust_semantic_node(
            path,
            &document.tree,
            source,
            "src/metrics.rs::metrics::Counter::increment",
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.kind(), "function_item");
    }

    #[test]
    fn skips_trait_impl_members_until_trait_impl_identity_is_supported() {
        let source = r#"
struct Counter;
trait Render { fn render(&self); }
impl Render for Counter { fn render(&self) {} }
"#;
        let path = Path::new("sample.rs");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_rust_skeleton(path, source, &document.tree, 3, &[], None).unwrap();

        assert_eq!(
            skeleton.available_paths,
            vec!["Counter", "Render", "Render::render"]
        );
    }
}
