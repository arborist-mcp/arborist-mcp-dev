use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_elixir_skeleton(
    path: &Path,
    source: &str,
    tree: &Tree,
    depth_limit: usize,
    expand_nodes: &[String],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<SemanticSkeleton> {
    let normalized_file_path = normalize_path(path);
    let mut expand_set = expand_nodes.iter().cloned().collect::<BTreeSet<_>>();
    let mut declarations = Vec::new();
    collect_declarations(tree.root_node(), source, &[], deadline, &mut declarations)?;

    let mut by_path: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, symbol) in &declarations {
        *by_path.entry(&symbol.semantic_path).or_default() += 1;
    }
    for selector in &expand_set {
        let local_path = selector
            .strip_prefix(&format!("{normalized_file_path}::"))
            .unwrap_or(selector);
        if by_path.get(local_path).copied().unwrap_or_default() > 1 {
            bail!(
                "ambiguous Elixir semantic path `{selector}`; duplicate declarations cannot be expanded safely"
            );
        }
    }
    let file_prefix = format!("{normalized_file_path}::");
    let qualified_singletons = expand_set
        .iter()
        .filter_map(|selector| selector.strip_prefix(&file_prefix))
        .filter(|local_path| by_path.get(*local_path).copied() == Some(1))
        .map(str::to_string)
        .collect::<Vec<_>>();
    expand_set.extend(qualified_singletons);

    let mut available_paths = Vec::new();
    let mut available_symbols = Vec::new();
    let mut rendered = Vec::new();
    let mut rendered_nodes = Vec::new();
    for (node, symbol) in declarations {
        if let Some(deadline) = deadline {
            deadline.check("rendering Elixir semantic skeleton")?;
        }
        if symbol.semantic_path.matches('.').count() + 1 > depth_limit
            && !expand_set.contains(&symbol.semantic_path)
            && !expand_set.contains(&symbol.symbol_id)
        {
            continue;
        }
        available_paths.push(symbol.semantic_path.clone());
        if rendered_nodes
            .iter()
            .any(|parent| contains_node(*parent, node))
        {
            continue;
        }
        let signature = symbol.signature.as_deref().ok_or_else(|| {
            anyhow!(
                "Elixir semantic symbol `{}` has no signature",
                symbol.semantic_path
            )
        })?;
        rendered.push(
            if expand_set.contains(&symbol.semantic_path) || expand_set.contains(&symbol.symbol_id)
            {
                node_text(node, source)?.trim().to_string()
            } else {
                format!("{signature} ...")
            },
        );
        rendered_nodes.push(node);
        available_symbols.push(symbol);
    }

    let result = SemanticSkeleton {
        file: normalized_file_path,
        skeleton: rendered.join("\n\n"),
        available_paths,
        available_symbols,
    };
    result.validate_public_output()?;
    Ok(result)
}

pub(crate) fn find_elixir_semantic_node<'tree>(
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
    let mut declarations = Vec::new();
    collect_declarations(tree.root_node(), source, &[], deadline, &mut declarations)?;
    let matches = declarations
        .into_iter()
        .filter(|(_, symbol)| symbol.semantic_path == local_target)
        .map(|(node, _)| node)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous Elixir semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_elixir_symbol_node(node: Node<'_>) -> bool {
    node.kind() == "call"
        && node
            .child_by_field_name("target")
            .is_some_and(|target| target.kind() == "identifier")
        && node
            .child_by_field_name("arguments")
            .and_then(|arguments| arguments.named_child(0))
            .is_some_and(|head| head.kind() == "call")
}

pub(crate) fn elixir_semantic_path_for_node(
    node: Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if is_definition_call(candidate, source) {
            let Some((name, _)) = definition_head(candidate, source)? else {
                return Ok(None);
            };
            let mut modules = Vec::new();
            let mut parent = candidate.parent();
            while let Some(ancestor) = parent {
                if is_module_call(ancestor, source) {
                    let arguments = ancestor.child_by_field_name("arguments");
                    if let Some(module) = module_argument_name(arguments, source)? {
                        modules.push(module);
                    }
                }
                parent = ancestor.parent();
            }
            modules.reverse();
            modules.push(name);
            return Ok(Some(modules.join(".")));
        }
        current = candidate.parent();
    }
    Ok(None)
}

pub(crate) fn elixir_declaration_node_for_node<'tree>(
    node: Node<'tree>,
    source: &str,
) -> Option<Node<'tree>> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if is_definition_call(candidate, source)
            && definition_head(candidate, source).ok().flatten().is_some()
        {
            return Some(candidate);
        }
        current = candidate.parent();
    }
    None
}

fn collect_declarations<'tree>(
    node: Node<'tree>,
    source: &str,
    modules: &[String],
    deadline: Option<&dyn DeadlineCheck>,
    declarations: &mut Vec<(Node<'tree>, SemanticSkeletonSymbol)>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("collecting Elixir semantic symbols")?;
    }
    if is_module_call(node, source) {
        if let (Some(module), Some(body)) = (
            module_argument_name(node.child_by_field_name("arguments"), source)?,
            node.child_by_field_name("do_block"),
        ) {
            let mut nested_modules = modules.to_vec();
            nested_modules.push(module);
            collect_declarations(body, source, &nested_modules, deadline, declarations)?;
        }
        return Ok(());
    }
    if is_definition_call(node, source)
        && let Some(symbol) = definition_symbol(node, source, modules)?
    {
        declarations.push((node, symbol));
        return Ok(());
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_declarations(child, source, modules, deadline, declarations)?;
    }
    Ok(())
}

fn definition_symbol(
    node: Node<'_>,
    source: &str,
    modules: &[String],
) -> Result<Option<SemanticSkeletonSymbol>> {
    let Some((name, parameters)) = definition_head(node, source)? else {
        return Ok(None);
    };
    let mut path = modules.to_vec();
    path.push(name);
    let semantic_path = path.join(".");
    let signature_end = node
        .child_by_field_name("do_block")
        .map(|body| body.start_byte())
        .unwrap_or(node.end_byte());
    let signature = source
        .get(node.start_byte()..signature_end)
        .map(str::trim)
        .filter(|signature| !signature.is_empty())
        .ok_or_else(|| {
            anyhow!("Elixir semantic symbol `{semantic_path}` has an empty signature")
        })?;
    Ok(Some(SemanticSkeletonSymbol {
        symbol_id: semantic_path.clone(),
        semantic_path: semantic_path.clone(),
        scope_path: semantic_parent_path(&semantic_path),
        node_kind: node.kind().to_string(),
        byte_range: (node.start_byte(), node.end_byte()),
        signature: Some(signature.to_string()),
        parameters,
        return_type: None,
        docstring: None,
    }))
}

fn definition_head(node: Node<'_>, source: &str) -> Result<Option<(String, Vec<String>)>> {
    let Some(arguments) = node.child_by_field_name("arguments") else {
        return Ok(None);
    };
    let Some(head) = arguments.named_child(0) else {
        return Ok(None);
    };
    if head.kind() != "call" {
        return Ok(None);
    }
    let Some(target) = head.child_by_field_name("target") else {
        return Ok(None);
    };
    let Some(name) = simple_target_name(target, source)? else {
        return Ok(None);
    };
    let parameters = head
        .child_by_field_name("arguments")
        .map(|arguments| {
            let mut cursor = arguments.walk();
            arguments
                .named_children(&mut cursor)
                .filter_map(|parameter| {
                    if parameter.kind() == "keywords" {
                        None
                    } else {
                        node_text(parameter, source)
                            .ok()
                            .map(|text| text.trim().to_string())
                    }
                })
                .filter(|parameter| !parameter.is_empty())
                .collect()
        })
        .unwrap_or_default();
    Ok(Some((name, parameters)))
}

fn module_argument_name(arguments: Option<Node<'_>>, source: &str) -> Result<Option<String>> {
    let Some(arguments) = arguments else {
        return Ok(None);
    };
    let Some(argument) = arguments.named_child(0) else {
        return Ok(None);
    };
    if argument.kind() != "alias" {
        return Ok(None);
    }
    simple_target_name(argument, source)
}

fn simple_target_name(target: Node<'_>, source: &str) -> Result<Option<String>> {
    let text = node_text(target, source)?.trim();
    if text.is_empty()
        || !text.split('.').all(|part| {
            !part.is_empty() && part.chars().all(|ch| ch == '_' || ch.is_alphanumeric())
        })
    {
        return Ok(None);
    }
    Ok(Some(text.to_string()))
}

fn is_module_call(node: Node<'_>, source: &str) -> bool {
    node.kind() == "call"
        && node
            .child_by_field_name("target")
            .is_some_and(|target| target.kind() == "identifier")
        && node.child_by_field_name("do_block").is_some()
        && node.child_by_field_name("arguments").is_some_and(|_| {
            node.child_by_field_name("target")
                .and_then(|target| node_text(target, source).ok())
                .is_some_and(|text| text.trim() == "defmodule")
        })
}

fn is_definition_call(node: Node<'_>, source: &str) -> bool {
    node.kind() == "call"
        && node.child_by_field_name("target").is_some_and(|target| {
            target.kind() == "identifier"
                && node_text(target, source)
                    .ok()
                    .is_some_and(|text| matches!(text.trim(), "def" | "defp"))
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{build_elixir_skeleton, find_elixir_semantic_node};
    use crate::language::parse_document;

    #[test]
    fn builds_elixir_skeleton_for_named_modules_and_functions() {
        let source = "defmodule Outer do\n  defmodule Inner do\n    def compute(value, extra \\\\ 1), do: value + extra\n    defp helper(), do: :ok\n  end\nend\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_elixir_skeleton(path, source, &document.tree, 10, &[], None).unwrap();
        assert_eq!(
            skeleton.available_paths,
            vec!["Outer.Inner.compute", "Outer.Inner.helper"]
        );
        let compute = skeleton
            .available_symbols
            .iter()
            .find(|symbol| symbol.semantic_path.ends_with("compute"))
            .unwrap();
        assert_eq!(compute.parameters, vec!["value", "extra \\\\ 1"]);
        assert_eq!(compute.scope_path.as_deref(), Some("Outer.Inner"));
        assert!(skeleton.skeleton.contains("def compute(value, extra"));
    }

    #[test]
    fn duplicate_elixir_paths_fail_closed_when_expanded() {
        let source = "defmodule Demo do\n  def run(), do: :one\n  def run(), do: :two\nend\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let error =
            build_elixir_skeleton(path, source, &document.tree, 10, &["Demo.run".into()], None)
                .expect_err("expanded duplicate declarations must remain ambiguous");
        assert!(error.to_string().contains("ambiguous Elixir semantic path"));
        assert!(find_elixir_semantic_node(path, &document.tree, source, "Demo.run", None).is_err());
    }

    #[test]
    fn ordinary_nested_calls_are_not_declarations() {
        let source = "defmodule Demo do\n  def run(value), do: helper(value)\n  defp helper(value), do: value\nend\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_elixir_skeleton(path, source, &document.tree, 10, &[], None).unwrap();
        assert_eq!(skeleton.available_paths, vec!["Demo.run", "Demo.helper"]);
        assert_eq!(
            find_elixir_semantic_node(path, &document.tree, source, "helper", None).unwrap(),
            None
        );
    }

    #[test]
    fn malformed_elixir_does_not_panic_during_skeleton_collection() {
        let source = "defmodule Broken do\n  def compute(\n";
        let path = Path::new("sample.ex");
        let document = parse_document(path, source).unwrap();
        let result = build_elixir_skeleton(path, source, &document.tree, 10, &[], None);
        assert!(result.is_ok());
    }
}
