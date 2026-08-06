use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_kotlin_skeleton(
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

    for node in collect_kotlin_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("collecting Kotlin semantic symbols")?;
        }
        let Some(name) = kotlin_symbol_name(node, source)? else {
            continue;
        };
        let Some(semantic_path) = kotlin_semantic_path(tree.root_node(), node, source, &name)?
        else {
            continue;
        };
        let signature = kotlin_signature(node, source).ok_or_else(|| {
            anyhow!("Kotlin semantic symbol `{semantic_path}` has an empty signature")
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
                parameters: kotlin_parameters(node, source),
                return_type: kotlin_return_type(node, source),
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
                "ambiguous Kotlin semantic path `{selector}`; duplicate declarations cannot be expanded safely"
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
            deadline.check("rendering Kotlin semantic skeleton")?;
        }
        if expanded_items
            .iter()
            .any(|ancestor: &Node<'_>| contains_node(*ancestor, node))
        {
            continue;
        }

        if expand_set.contains(semantic_path.as_str()) || expand_set.contains(symbol_id.as_str()) {
            skeleton_items.push(kotlin_full_declaration(node, source)?);
            expanded_items.push(node);
        } else {
            let signature = kotlin_signature(node, source).ok_or_else(|| {
                anyhow!("Kotlin semantic symbol `{semantic_path}` has an empty signature")
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

pub(crate) fn find_kotlin_semantic_node<'tree>(
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

    for node in collect_kotlin_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving Kotlin semantic target")?;
        }
        let Some(name) = kotlin_symbol_name(node, source)? else {
            continue;
        };
        if kotlin_semantic_path(tree.root_node(), node, source, &name)?.as_deref()
            == Some(local_target)
        {
            matches.push(node);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous Kotlin semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_kotlin_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class_declaration"
            | "object_declaration"
            | "function_declaration"
            | "property_declaration"
            | "type_alias"
            | "companion_object"
    )
}

pub(crate) fn is_kotlin_semantic_symbol_node(node: Node<'_>) -> bool {
    if !is_kotlin_symbol_node(node) {
        return false;
    }

    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(candidate.kind(), "function_declaration" | "block") {
            return false;
        }
        current = candidate.parent();
    }
    true
}

pub(crate) fn kotlin_symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let name = match node.kind() {
        "class_declaration"
        | "object_declaration"
        | "function_declaration"
        | "companion_object" => node.child_by_field_name("name"),
        "property_declaration" => kotlin_property_name_node(node),
        "type_alias" => kotlin_type_alias_name_node(node),
        _ => None,
    };
    name.map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

pub(crate) fn kotlin_semantic_path(
    root: Node<'_>,
    node: Node<'_>,
    source: &str,
    name: &str,
) -> Result<Option<String>> {
    if !is_kotlin_semantic_symbol_node(node) {
        return Ok(None);
    }

    let mut parts = Vec::new();
    if let Some(package) = kotlin_package_name(root, source)? {
        parts.extend(package.split('.').map(str::to_string));
    }

    let mut ancestors = Vec::new();
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(candidate.kind(), "class_declaration" | "object_declaration")
            && let Some(ancestor_name) = kotlin_symbol_name(candidate, source)?
        {
            ancestors.push(ancestor_name);
        } else if candidate.kind() == "companion_object" {
            // Companion members are indexed under `Type::Companion::member` so
            // class-name receiver calls can dispatch without confusing them
            // with instance members.
            ancestors.push("Companion".to_string());
        }
        current = candidate.parent();
    }
    ancestors.reverse();
    parts.extend(ancestors);
    parts.push(name.to_string());
    Ok((!parts.is_empty()).then(|| parts.join("::")))
}

pub(crate) fn kotlin_signature(node: Node<'_>, source: &str) -> Option<String> {
    let end_byte = match node.kind() {
        "class_declaration" | "object_declaration" | "companion_object" => {
            kotlin_direct_child_by_kind(node, &["class_body", "enum_class_body"])
                .map(|body| body.start_byte())
                .unwrap_or(node.end_byte())
        }
        "function_declaration" => kotlin_direct_child_by_kind(node, &["function_body"])
            .map(|body| body.start_byte())
            .unwrap_or(node.end_byte()),
        "property_declaration" => kotlin_direct_child_by_kind(node, &["variable_declaration"])
            .map(|declaration| declaration.end_byte())
            .unwrap_or(node.end_byte()),
        _ => node.end_byte(),
    };
    let signature = source
        .get(node.start_byte()..end_byte)?
        .trim()
        .trim_end_matches(';')
        .trim();
    (!signature.is_empty()).then(|| signature.to_string())
}

pub(crate) fn kotlin_parameters(node: Node<'_>, source: &str) -> Vec<String> {
    if node.kind() != "function_declaration" {
        return Vec::new();
    }
    let Some(parameters) = kotlin_direct_child_by_kind(node, &["function_value_parameters"]) else {
        return Vec::new();
    };
    let mut cursor = parameters.walk();
    parameters
        .named_children(&mut cursor)
        .filter(|parameter| parameter.kind() == "parameter")
        .filter_map(|parameter| node_text(parameter, source).ok().map(str::trim))
        .filter(|parameter| !parameter.is_empty())
        .map(str::to_string)
        .collect()
}

pub(crate) fn kotlin_return_type(node: Node<'_>, source: &str) -> Option<String> {
    match node.kind() {
        "function_declaration" => {
            let parameters = kotlin_direct_child_by_kind(node, &["function_value_parameters"])?;
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| {
                    child.start_byte() >= parameters.end_byte() && is_kotlin_type_node(*child)
                })
                .and_then(|return_type| node_text(return_type, source).ok())
                .map(str::trim)
                .filter(|return_type| !return_type.is_empty())
                .map(str::to_string)
        }
        "property_declaration" => kotlin_property_declared_or_inferred_type(node, source),
        "type_alias" => {
            let mut cursor = node.walk();
            node.named_children(&mut cursor)
                .find(|child| is_kotlin_type_node(*child))
                .and_then(|type_node| node_text(type_node, source).ok())
                .map(str::trim)
                .filter(|return_type| !return_type.is_empty())
                .map(str::to_string)
        }
        _ => None,
    }
}

/// Returns a property's explicit declared type, or infers a bare constructor
/// initializer such as `val value = Other()`. Complex, nullable, generic, and
/// non-identifier initializers fail closed so inferred types never guess a
/// target, keeping property chains and summaries conservative.
fn kotlin_property_declared_or_inferred_type(node: Node<'_>, source: &str) -> Option<String> {
    let declared = kotlin_direct_child_by_kind(node, &["variable_declaration"])
        .and_then(|declaration| {
            let mut cursor = declaration.walk();
            declaration
                .named_children(&mut cursor)
                .find(|child| is_kotlin_type_node(*child))
        })
        .and_then(|return_type| node_text(return_type, source).ok())
        .map(str::trim)
        .filter(|return_type| !return_type.is_empty())
        .map(str::to_string);
    if declared.is_some() {
        return declared;
    }
    let call = kotlin_direct_child_by_kind(node, &["call_expression"])?;
    let callee = call.named_child(0)?;
    kotlin_constructor_callee_name(callee, source)
        .ok()
        .flatten()
}

/// Returns the pure dotted identifier spelling of a constructor callee such as
/// `Other` in `Other()` or `Outer.Inner` in `Outer.Inner()`. Nullable,
/// callable-reference, call, indexing, and parenthesized receivers fail closed.
pub(crate) fn kotlin_constructor_callee_name(
    node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    if node.kind() == "identifier" {
        let name = node_text(node, source)?.trim().to_string();
        return Ok((!name.is_empty()).then_some(name));
    }
    if node.kind() != "navigation_expression" {
        return Ok(None);
    }
    let text = node_text(node, source)?.trim();
    if text.contains('?') || text.contains("::") {
        return Ok(None);
    }
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    if children.len() != 2 || children[1].kind() != "identifier" {
        return Ok(None);
    }
    let Some(prefix) = kotlin_constructor_callee_name(children[0], source)? else {
        return Ok(None);
    };
    let member = node_text(children[1], source)?.trim().to_string();
    if member.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!("{prefix}.{member}")))
}

fn kotlin_full_declaration(node: Node<'_>, source: &str) -> Result<String> {
    Ok(node_text(node, source)?.trim().to_string())
}

fn kotlin_package_name(root: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut cursor = root.walk();
    let Some(package) = root
        .named_children(&mut cursor)
        .find(|node| node.kind() == "package_header")
    else {
        return Ok(None);
    };
    let package_name = kotlin_direct_child_by_kind(package, &["qualified_identifier"]);
    package_name
        .map(|node| node_text(node, source).map(|text| text.trim().to_string()))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

fn kotlin_property_name_node(node: Node<'_>) -> Option<Node<'_>> {
    let declaration = kotlin_direct_child_by_kind(node, &["variable_declaration"])?;
    let mut cursor = declaration.walk();
    declaration
        .named_children(&mut cursor)
        .find(|child| child.kind() == "identifier")
}

fn kotlin_type_alias_name_node(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == "identifier")
}

fn kotlin_direct_child_by_kind<'tree>(node: Node<'tree>, kinds: &[&str]) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| kinds.contains(&child.kind()))
}

fn is_kotlin_type_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "function_type"
            | "non_nullable_type"
            | "nullable_type"
            | "parenthesized_type"
            | "user_type"
    )
}

fn collect_kotlin_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    fn collect<'tree>(
        node: Node<'tree>,
        deadline: Option<&dyn DeadlineCheck>,
        nodes: &mut Vec<Node<'tree>>,
    ) -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("collecting Kotlin semantic symbols")?;
        }
        if is_kotlin_semantic_symbol_node(node) {
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

    use super::{build_kotlin_skeleton, find_kotlin_semantic_node};
    use crate::language::parse_document;

    #[test]
    fn builds_kotlin_skeletons_for_packages_types_members_properties_and_aliases() {
        let source = r#"
package com.example

typealias UserId = String

class Counter(val initial: Int) {
    val label: String = "counter"
    fun increment(amount: Int): Int = initial + amount
    class Nested {
        fun run() {}
    }
}

interface Renderer {
    fun render(value: String): String
}

object Config {
    val answer = 42
}
"#;
        let path = Path::new("Counter.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let skeleton = build_kotlin_skeleton(path, source, &document.tree, 5, &[], None).unwrap();

        assert_eq!(
            skeleton.available_paths,
            vec![
                "com::example::UserId",
                "com::example::Counter",
                "com::example::Counter::label",
                "com::example::Counter::increment",
                "com::example::Counter::Nested",
                "com::example::Counter::Nested::run",
                "com::example::Renderer",
                "com::example::Renderer::render",
                "com::example::Config",
                "com::example::Config::answer",
            ]
        );
        assert!(
            skeleton
                .skeleton
                .contains("class Counter(val initial: Int) ...")
        );
        assert!(
            skeleton
                .skeleton
                .contains("fun increment(amount: Int): Int ...")
        );
        assert!(skeleton.skeleton.contains("val label: String ..."));

        let increment = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "com::example::Counter::increment")
            .unwrap();
        assert_eq!(increment.parameters, vec!["amount: Int"]);
        assert_eq!(increment.return_type.as_deref(), Some("Int"));
        assert_eq!(
            increment.scope_path.as_deref(),
            Some("com::example::Counter")
        );

        let found = find_kotlin_semantic_node(
            path,
            &document.tree,
            source,
            "Counter.kt::com::example::Counter::increment",
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.kind(), "function_declaration");
    }

    #[test]
    fn records_type_alias_target_as_return_type() {
        let source = r#"
package demo

typealias Helper = Other
typealias Generic<T> = List<T>
typealias Nullable = Other?
"#;
        let path = Path::new("Aliases.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let skeleton = build_kotlin_skeleton(path, source, &document.tree, 5, &[], None).unwrap();

        let helper = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "demo::Helper")
            .unwrap();
        assert_eq!(helper.return_type.as_deref(), Some("Other"));

        let generic = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "demo::Generic")
            .unwrap();
        assert_eq!(generic.return_type.as_deref(), Some("List<T>"));

        let nullable = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "demo::Nullable")
            .unwrap();
        assert_eq!(nullable.return_type.as_deref(), Some("Other?"));
    }

    #[test]
    fn skips_kotlin_local_declarations_without_stable_file_semantic_paths() {
        let source = r#"
package demo

fun outer() {
    class Local
    fun nested() = 1
    val value = nested()
}
"#;
        let path = Path::new("Local.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let skeleton = build_kotlin_skeleton(path, source, &document.tree, 4, &[], None).unwrap();

        assert_eq!(skeleton.available_paths, vec!["demo::outer"]);
        assert!(!skeleton.skeleton.contains("class Local"));
        assert!(!skeleton.skeleton.contains("fun nested"));
    }

    #[test]
    fn namespaces_companion_members_under_the_companion_scope() {
        let source = r#"
package demo

class Config {
    fun instance(value: Int): Int = value
    companion object {
        fun helper(value: Int): Int = value
        val label = "x"
    }
}
"#;
        let path = Path::new("Companion.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let skeleton = build_kotlin_skeleton(path, source, &document.tree, 5, &[], None).unwrap();

        assert_eq!(
            skeleton.available_paths,
            vec![
                "demo::Config",
                "demo::Config::instance",
                "demo::Config::Companion::helper",
                "demo::Config::Companion::label",
            ]
        );
        let helper = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "demo::Config::Companion::helper")
            .unwrap();
        assert_eq!(
            helper.scope_path.as_deref(),
            Some("demo::Config::Companion")
        );
    }

    #[test]
    fn namespaces_named_companion_objects_and_keeps_members_canonical() {
        let source = r#"
package demo

class Config {
    companion object Factory {
        fun helper(value: Int): Int = value
        val label = "x"
    }
}
"#;
        let path = Path::new("Companion.kt");
        let document = parse_document(path, source).unwrap();
        assert!(!document.tree.root_node().has_error());
        let skeleton = build_kotlin_skeleton(path, source, &document.tree, 5, &[], None).unwrap();

        // The named companion object is indexed under its declared name, while
        // its members stay under the canonical `Type::Companion::` scope so
        // `Config.Companion.helper` and `Config.Factory.helper` share one ID.
        assert_eq!(
            skeleton.available_paths,
            vec![
                "demo::Config",
                "demo::Config::Factory",
                "demo::Config::Companion::helper",
                "demo::Config::Companion::label",
            ]
        );
        let companion = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path == "demo::Config::Factory")
            .unwrap();
        assert_eq!(companion.node_kind, "companion_object");
        assert_eq!(companion.scope_path.as_deref(), Some("demo::Config"));
    }
}
