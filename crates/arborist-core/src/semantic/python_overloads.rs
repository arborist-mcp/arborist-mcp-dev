use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::node_text;

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
