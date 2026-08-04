use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Node, Tree};

use super::semantic_parent_path;
use crate::deadline::DeadlineCheck;
use crate::language::{contains_node, node_text, normalize_path};
use crate::model::{SemanticSkeleton, SemanticSkeletonSymbol};

pub(crate) fn build_csharp_skeleton(
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

    for node in collect_csharp_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("collecting C# semantic symbols")?;
        }
        let Some(name) = csharp_symbol_name(node, source)? else {
            continue;
        };
        let Some(semantic_path) = csharp_semantic_path(tree.root_node(), node, source, &name)?
        else {
            continue;
        };
        let signature = csharp_signature(node, source).ok_or_else(|| {
            anyhow!("C# semantic symbol `{semantic_path}` has an empty signature")
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
                parameters: csharp_parameters(node, source),
                return_type: csharp_return_type(node, source),
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
                "ambiguous C# semantic path `{selector}`; duplicate declarations cannot be expanded safely"
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
            deadline.check("rendering C# semantic skeleton")?;
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
            let signature = csharp_signature(node, source).ok_or_else(|| {
                anyhow!("C# semantic symbol `{semantic_path}` has an empty signature")
            })?;
            skeleton_items.push(format!("{signature} ..."));
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("validating C# semantic skeleton")?;
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

pub(crate) fn find_csharp_semantic_node<'tree>(
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

    for node in collect_csharp_symbol_nodes(tree.root_node(), deadline)? {
        if let Some(deadline) = deadline {
            deadline.check("resolving C# semantic target")?;
        }
        let Some(name) = csharp_symbol_name(node, source)? else {
            continue;
        };
        if csharp_semantic_path(tree.root_node(), node, source, &name)?.as_deref()
            == Some(local_target)
        {
            matches.push(node);
        }
    }

    match matches.as_slice() {
        [] => Ok(None),
        [node] => Ok(Some(*node)),
        _ => bail!(
            "ambiguous C# semantic path `{target_path}`; duplicate declarations cannot be resolved safely"
        ),
    }
}

pub(crate) fn is_csharp_symbol_node(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
            | "method_declaration"
            | "constructor_declaration"
    )
}

pub(crate) fn csharp_symbol_name(node: Node<'_>, source: &str) -> Result<Option<String>> {
    node.child_by_field_name("name")
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

pub(crate) fn csharp_semantic_path(
    root: Node<'_>,
    node: Node<'_>,
    source: &str,
    name: &str,
) -> Result<Option<String>> {
    let mut parts = Vec::new();
    if let Some(namespace) = csharp_file_scoped_namespace_name(root, source)? {
        parts.extend(namespace.split('.').map(str::to_string));
    }

    let mut ancestors = Vec::new();
    let mut current = node.parent();
    while let Some(candidate) = current {
        if matches!(
            candidate.kind(),
            "namespace_declaration"
                | "class_declaration"
                | "struct_declaration"
                | "interface_declaration"
                | "enum_declaration"
                | "record_declaration"
        ) && let Some(ancestor_name) = csharp_symbol_name(candidate, source)?
        {
            ancestors.push((candidate.kind() == "namespace_declaration", ancestor_name));
        }
        current = candidate.parent();
    }
    ancestors.reverse();
    for (is_namespace, ancestor_name) in ancestors {
        if is_namespace {
            parts.extend(ancestor_name.split('.').map(str::to_string));
        } else {
            parts.push(ancestor_name);
        }
    }
    parts.push(name.to_string());
    Ok((!parts.is_empty()).then(|| parts.join("::")))
}

pub(crate) fn csharp_signature(node: Node<'_>, source: &str) -> Option<String> {
    let end_byte = node
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .unwrap_or(node.end_byte());
    let signature = source
        .get(node.start_byte()..end_byte)?
        .trim()
        .trim_end_matches(';')
        .trim();
    (!signature.is_empty()).then(|| signature.to_string())
}

pub(crate) fn csharp_parameters(node: Node<'_>, source: &str) -> Vec<String> {
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

pub(crate) fn csharp_return_type(node: Node<'_>, source: &str) -> Option<String> {
    node.child_by_field_name("returns")
        .and_then(|return_type| node_text(return_type, source).ok())
        .map(str::trim)
        .filter(|return_type| !return_type.is_empty())
        .map(str::to_string)
}

fn csharp_file_scoped_namespace_name(root: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut cursor = root.walk();
    let namespaces = root
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "file_scoped_namespace_declaration")
        .filter_map(|node| csharp_symbol_name(node, source).transpose())
        .collect::<Result<Vec<_>>>()
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    match namespaces.as_slice() {
        [] => Ok(None),
        [namespace] => Ok(Some(namespace.clone())),
        _ => Ok(None),
    }
}

fn collect_csharp_symbol_nodes<'tree>(
    root: Node<'tree>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<Node<'tree>>> {
    fn collect<'tree>(
        node: Node<'tree>,
        deadline: Option<&dyn DeadlineCheck>,
        nodes: &mut Vec<Node<'tree>>,
    ) -> Result<()> {
        if let Some(deadline) = deadline {
            deadline.check("collecting C# semantic symbols")?;
        }
        if is_csharp_symbol_node(node) {
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

    use super::{build_csharp_skeleton, find_csharp_semantic_node};
    use crate::language::parse_document;

    #[test]
    fn builds_csharp_skeletons_for_namespaces_types_methods_and_constructors() {
        let source = r#"
using System;

namespace Demo.Core {
    public class Outer {
        public class Nested {
            public Nested(int value) {}
            public string Render(string prefix) => prefix + value;
        }
    }

    public struct Point { public int X; }
    public interface IRenderer { string Render(); }
    public enum Kind { Basic }
    public record Entry(string Name);
}
"#;
        let path = Path::new("Sample.cs");
        let document = parse_document(path, source).unwrap();
        let skeleton = build_csharp_skeleton(path, source, &document.tree, 5, &[], None).unwrap();

        assert_eq!(
            skeleton.available_paths,
            vec![
                "Demo::Core::Outer",
                "Demo::Core::Outer::Nested",
                "Demo::Core::Outer::Nested::Nested",
                "Demo::Core::Outer::Nested::Render",
                "Demo::Core::Point",
                "Demo::Core::IRenderer",
                "Demo::Core::IRenderer::Render",
                "Demo::Core::Kind",
                "Demo::Core::Entry",
            ]
        );
        assert!(skeleton.skeleton.contains("public class Outer ..."));
        assert!(
            skeleton
                .skeleton
                .contains("public string Render(string prefix) ...")
        );

        let found = find_csharp_semantic_node(
            path,
            &document.tree,
            source,
            "Sample.cs::Demo::Core::Outer::Nested::Render",
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(found.kind(), "method_declaration");
    }

    #[test]
    fn supports_file_scoped_namespaces_depth_limits_and_qualified_expansion() {
        let source = r#"
using System;

namespace Demo.File;

public class Sample {
    public int Add(int left, int right) => left + right;
}
"#;
        let path = Path::new("FileScoped.cs");
        let document = parse_document(path, source).unwrap();
        let target = "FileScoped.cs::Demo::File::Sample".to_string();
        let skeleton =
            build_csharp_skeleton(path, source, &document.tree, 2, &[target], None).unwrap();

        assert_eq!(skeleton.available_paths, vec!["Demo::File::Sample"]);
        assert!(skeleton.skeleton.contains("public class Sample {"));
        assert!(
            skeleton
                .skeleton
                .contains("public int Add(int left, int right)")
        );
    }

    #[test]
    fn rejects_ambiguous_csharp_expansion_paths() {
        let source = r#"
public class Duplicate {
    public void Run() {}
    public void Run(int value) {}
}
"#;
        let path = Path::new("Duplicates.cs");
        let document = parse_document(path, source).unwrap();
        let error = build_csharp_skeleton(
            path,
            source,
            &document.tree,
            4,
            &["Duplicate::Run".to_string()],
            None,
        )
        .unwrap_err();

        assert!(error.to_string().contains("ambiguous C# semantic path"));
    }
}
