use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use tree_sitter::Node;

use crate::deadline::DeadlineCheck;
use crate::language::node_text;

const OVERLOAD_ALIAS_PHASE: &str = "collecting Python overload aliases";
const OVERLOAD_DECORATOR_PHASE: &str = "classifying Python overload decorators";

#[derive(Debug)]
struct PythonOverloadBinding {
    end_byte: usize,
    active: bool,
}

#[derive(Debug)]
struct PythonModuleBinding {
    name: String,
    effective_byte: usize,
    is_overload_import: bool,
    is_overload_module_import: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PythonOverloadScopeKind {
    Class,
    Function,
}

#[derive(Debug)]
struct PythonOverloadScope {
    start_byte: usize,
    end_byte: usize,
    bindings: BTreeMap<String, Vec<PythonOverloadBinding>>,
}

#[derive(Debug)]
pub(crate) struct PythonOverloadNames {
    bindings: BTreeMap<String, Vec<PythonOverloadBinding>>,
    scopes: Vec<PythonOverloadScope>,
}

impl PythonOverloadNames {
    fn binding_before(&self, name: &str, byte_offset: usize) -> Option<bool> {
        self.bindings.get(name).and_then(|bindings| {
            bindings
                .iter()
                .rev()
                .find(|binding| binding.end_byte <= byte_offset)
                .map(|binding| binding.active)
        })
    }

    fn binding_before_node(&self, name: &str, node: Node<'_>) -> Option<bool> {
        let byte_offset = node.start_byte();
        let mut scopes = self
            .scopes
            .iter()
            .filter(|scope| scope.start_byte <= byte_offset && byte_offset < scope.end_byte)
            .collect::<Vec<_>>();
        scopes.sort_by_key(|scope| scope.end_byte - scope.start_byte);

        for scope in scopes {
            if let Some(binding) = scope.bindings.get(name).and_then(|bindings| {
                bindings
                    .iter()
                    .rev()
                    .find(|binding| binding.end_byte <= byte_offset)
                    .map(|binding| binding.active)
            }) {
                return Some(binding);
            }
        }
        self.binding_before(name, byte_offset)
    }

    fn contains_before(&self, name: &str, node: Node<'_>) -> bool {
        self.binding_before_node(name, node) == Some(true)
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
    effective_byte: usize,
    bindings: &mut Vec<PythonModuleBinding>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(OVERLOAD_ALIAS_PHASE)?;
    }
    match node.kind() {
        "identifier" | "keyword_identifier" => {
            let name = node_text(node, source)?.trim();
            if name != "_" {
                bindings.push(PythonModuleBinding {
                    name: name.to_string(),
                    effective_byte,
                    is_overload_import: false,
                    is_overload_module_import: false,
                });
            }
        }
        "attribute" | "subscript" => {}
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                python_collect_pattern_bindings(child, source, effective_byte, bindings, deadline)?;
            }
        }
    }
    Ok(())
}

fn python_collect_import_bindings(
    statement: Node<'_>,
    source: &str,
    bindings: &mut Vec<PythonModuleBinding>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let module_is_typing = if statement.kind() == "import_from_statement" {
        statement
            .named_child(0)
            .map(|module| {
                node_text(module, source)
                    .map(|text| matches!(text.trim(), "typing" | "typing_extensions"))
            })
            .transpose()?
            .unwrap_or(false)
    } else {
        false
    };
    let mut cursor = statement.walk();
    let mut children = statement.named_children(&mut cursor);
    if statement.kind() == "import_from_statement" {
        let _ = children.next();
    }
    for child in children {
        if let Some(deadline) = deadline {
            deadline.check(OVERLOAD_ALIAS_PHASE)?;
        }
        if child.kind() == "wildcard_import" {
            bindings.push(PythonModuleBinding {
                name: "overload".to_string(),
                effective_byte: statement.end_byte(),
                is_overload_import: module_is_typing,
                is_overload_module_import: false,
            });
            continue;
        }
        let (binding, imported_name) = if child.kind() == "aliased_import" {
            let mut alias_cursor = child.walk();
            let aliases = child.named_children(&mut alias_cursor).collect::<Vec<_>>();
            (
                aliases
                    .last()
                    .map(|alias| node_text(*alias, source).map(str::to_string))
                    .transpose()?,
                aliases
                    .first()
                    .map(|imported| node_text(*imported, source).map(str::to_string))
                    .transpose()?,
            )
        } else if matches!(child.kind(), "dotted_name" | "identifier") {
            let imported_name = node_text(child, source)?.to_string();
            let binding_name = if statement.kind() == "import_statement" {
                imported_name
                    .split('.')
                    .next()
                    .unwrap_or_default()
                    .to_string()
            } else {
                imported_name.clone()
            };
            (Some(binding_name), Some(imported_name))
        } else {
            (None, None)
        };
        if let Some(binding) = binding {
            bindings.push(PythonModuleBinding {
                name: binding.trim().to_string(),
                effective_byte: statement.end_byte(),
                is_overload_import: module_is_typing
                    && imported_name
                        .as_deref()
                        .is_some_and(|name| name.trim() == "overload"),
                is_overload_module_import: statement.kind() == "import_statement"
                    && imported_name
                        .as_deref()
                        .is_some_and(|name| matches!(name.trim(), "typing" | "typing_extensions")),
            });
        }
    }
    Ok(())
}

fn python_collect_match_pattern_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut Vec<PythonModuleBinding>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(OVERLOAD_ALIAS_PHASE)?;
    }
    match node.kind() {
        "case_pattern" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if child.kind() == "dotted_name" {
                    let name = node_text(child, source)?.trim();
                    if !name.contains('.') && name != "_" {
                        bindings.push(PythonModuleBinding {
                            name: name.to_string(),
                            effective_byte: child.end_byte(),
                            is_overload_import: false,
                            is_overload_module_import: false,
                        });
                    }
                } else {
                    python_collect_match_pattern_bindings(child, source, bindings, deadline)?;
                }
            }
        }
        "as_pattern" => {
            let mut cursor = node.walk();
            let children = node.named_children(&mut cursor).collect::<Vec<_>>();
            if let Some(alias) = node.child_by_field_name("alias").or_else(|| {
                children
                    .iter()
                    .rev()
                    .copied()
                    .find(|child| matches!(child.kind(), "identifier" | "keyword_identifier"))
            }) {
                python_collect_pattern_bindings(
                    alias,
                    source,
                    alias.end_byte(),
                    bindings,
                    deadline,
                )?;
            }
            for child in children {
                if child.kind() == "case_pattern" {
                    python_collect_match_pattern_bindings(child, source, bindings, deadline)?;
                }
            }
        }
        "keyword_pattern" => {
            let mut cursor = node.walk();
            let mut children = node.named_children(&mut cursor);
            let _ = children.next();
            for child in children {
                if child.kind() == "dotted_name" {
                    let name = node_text(child, source)?.trim();
                    if !name.contains('.') && name != "_" {
                        bindings.push(PythonModuleBinding {
                            name: name.to_string(),
                            effective_byte: child.end_byte(),
                            is_overload_import: false,
                            is_overload_module_import: false,
                        });
                    }
                } else {
                    python_collect_match_pattern_bindings(child, source, bindings, deadline)?;
                }
            }
        }
        "splat_pattern" => {
            if let Some(name_node) = node.named_child(0) {
                let name = node_text(name_node, source)?.trim();
                if name != "_" {
                    bindings.push(PythonModuleBinding {
                        name: name.to_string(),
                        effective_byte: name_node.end_byte(),
                        is_overload_import: false,
                        is_overload_module_import: false,
                    });
                }
            }
        }
        _ => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                python_collect_match_pattern_bindings(child, source, bindings, deadline)?;
            }
        }
    }
    Ok(())
}

fn python_collect_nested_module_bindings(
    node: Node<'_>,
    source: &str,
    bindings: &mut Vec<PythonModuleBinding>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(OVERLOAD_ALIAS_PHASE)?;
    }
    match node.kind() {
        "assignment" | "augmented_assignment" => {
            if let Some(left) = node.child_by_field_name("left") {
                python_collect_pattern_bindings(left, source, node.end_byte(), bindings, deadline)?;
            }
        }
        "for_statement" => {
            if let Some(left) = node.child_by_field_name("left") {
                python_collect_pattern_bindings(left, source, left.end_byte(), bindings, deadline)?;
            }
        }
        "named_expression" => {
            if let Some(name) = node.child_by_field_name("name") {
                python_collect_pattern_bindings(name, source, node.end_byte(), bindings, deadline)?;
            }
        }
        "case_pattern" => {
            python_collect_match_pattern_bindings(node, source, bindings, deadline)?;
            return Ok(());
        }
        "delete_statement" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                if let Some(deadline) = deadline {
                    deadline.check(OVERLOAD_ALIAS_PHASE)?;
                }
                python_collect_pattern_bindings(
                    child,
                    source,
                    node.end_byte(),
                    bindings,
                    deadline,
                )?;
            }
        }
        "function_definition" | "class_definition" => {
            if let Some(name) = node.child_by_field_name("name") {
                bindings.push(PythonModuleBinding {
                    name: node_text(name, source)?.trim().to_string(),
                    effective_byte: node.end_byte(),
                    is_overload_import: false,
                    is_overload_module_import: false,
                });
            }
            return Ok(());
        }
        "decorated_definition" => {
            if let Some(definition) = node.child_by_field_name("definition")
                && let Some(name) = definition.child_by_field_name("name")
            {
                bindings.push(PythonModuleBinding {
                    name: node_text(name, source)?.trim().to_string(),
                    effective_byte: node.end_byte(),
                    is_overload_import: false,
                    is_overload_module_import: false,
                });
            }
            return Ok(());
        }
        "type_alias_statement" => {
            if let Some(left) = node.child_by_field_name("left") {
                python_collect_pattern_bindings(left, source, node.end_byte(), bindings, deadline)?;
            }
        }
        "import_statement" | "import_from_statement" => {
            python_collect_import_bindings(node, source, bindings, deadline)?;
            return Ok(());
        }
        "except_clause" | "as_pattern" => {
            if let Some(alias) = node.child_by_field_name("alias") {
                python_collect_pattern_bindings(
                    alias,
                    source,
                    alias.end_byte(),
                    bindings,
                    deadline,
                )?;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        python_collect_nested_module_bindings(child, source, bindings, deadline)?;
    }
    Ok(())
}

fn python_module_binding_events(
    statement: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<PythonModuleBinding>> {
    let mut bindings = Vec::new();
    python_collect_nested_module_bindings(statement, source, &mut bindings, deadline)?;
    bindings.sort_by_key(|binding| binding.effective_byte);
    Ok(bindings)
}

fn python_typing_overload_import_aliases(
    statement: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<String>> {
    if let Some(deadline) = deadline {
        deadline.check(OVERLOAD_ALIAS_PHASE)?;
    }
    if statement.kind() != "import_from_statement" {
        return Ok(BTreeSet::new());
    }
    let mut cursor = statement.walk();
    let mut children = statement.named_children(&mut cursor);
    let Some(module) = children.next() else {
        return Ok(BTreeSet::new());
    };
    if !matches!(
        node_text(module, source)?.trim(),
        "typing" | "typing_extensions"
    ) {
        return Ok(BTreeSet::new());
    }

    let mut aliases = BTreeSet::new();
    for imported in children {
        if let Some(deadline) = deadline {
            deadline.check(OVERLOAD_ALIAS_PHASE)?;
        }
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
            "wildcard_import" => {
                aliases.insert("overload".to_string());
            }
            _ => {}
        }
    }
    Ok(aliases)
}

fn python_collect_parameter_bindings(
    function: Node<'_>,
    source: &str,
    effective_byte: usize,
    bindings: &mut Vec<PythonModuleBinding>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let Some(parameters) = function.child_by_field_name("parameters") else {
        return Ok(());
    };
    let mut cursor = parameters.walk();
    for parameter in parameters.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check(OVERLOAD_ALIAS_PHASE)?;
        }
        let binding = match parameter.kind() {
            "identifier" | "keyword_identifier" | "tuple_pattern" => Some(parameter),
            "default_parameter" | "typed_default_parameter" => {
                parameter.child_by_field_name("name")
            }
            "typed_parameter" | "list_splat_pattern" | "dictionary_splat_pattern" => {
                parameter.named_child(0)
            }
            _ => None,
        };
        if let Some(binding) = binding {
            python_collect_pattern_bindings(binding, source, effective_byte, bindings, deadline)?;
        }
    }
    Ok(())
}

fn python_collect_scope_declarations(
    node: Node<'_>,
    source: &str,
    global_names: &mut BTreeSet<String>,
    nonlocal_names: &mut BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(OVERLOAD_ALIAS_PHASE)?;
    }
    if matches!(node.kind(), "function_definition" | "class_definition") {
        return Ok(());
    }
    if matches!(node.kind(), "global_statement" | "nonlocal_statement") {
        let target = if node.kind() == "global_statement" {
            global_names
        } else {
            nonlocal_names
        };
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "identifier" {
                target.insert(node_text(child, source)?.trim().to_string());
            }
        }
        return Ok(());
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        python_collect_scope_declarations(child, source, global_names, nonlocal_names, deadline)?;
    }
    Ok(())
}

fn python_scope_binding_map(
    scope: Node<'_>,
    source: &str,
    kind: PythonOverloadScopeKind,
    overload_aliases: &BTreeSet<String>,
    module_aliases: &BTreeSet<String>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeMap<String, Vec<PythonOverloadBinding>>> {
    let Some(body) = scope.child_by_field_name("body") else {
        return Ok(BTreeMap::new());
    };
    let events = python_module_binding_events(body, source, deadline)?;
    let mut bindings = BTreeMap::new();
    let mut tracked_names = overload_aliases.clone();
    let mut tracked_modules = module_aliases.clone();
    let mut global_names = BTreeSet::new();
    let mut nonlocal_names = BTreeSet::new();
    if kind == PythonOverloadScopeKind::Function {
        python_collect_scope_declarations(
            body,
            source,
            &mut global_names,
            &mut nonlocal_names,
            deadline,
        )?;
    }

    if kind == PythonOverloadScopeKind::Function {
        for event in &events {
            if global_names.contains(&event.name) || nonlocal_names.contains(&event.name) {
                continue;
            }
            python_add_overload_binding(
                &mut bindings,
                event.name.clone(),
                body.start_byte(),
                false,
            );
            if tracked_modules.contains(&event.name) || event.is_overload_module_import {
                python_add_overload_binding(
                    &mut bindings,
                    format!("{}.overload", event.name),
                    body.start_byte(),
                    false,
                );
            }
        }
        let mut parameters = Vec::new();
        python_collect_parameter_bindings(
            scope,
            source,
            body.start_byte(),
            &mut parameters,
            deadline,
        )?;
        for parameter in parameters {
            python_add_overload_binding(&mut bindings, parameter.name, body.start_byte(), false);
        }
    }

    for event in events {
        if let Some(deadline) = deadline {
            deadline.check(OVERLOAD_ALIAS_PHASE)?;
        }
        if event.is_overload_import {
            tracked_names.insert(event.name.clone());
        }
        if event.is_overload_module_import {
            tracked_modules.insert(event.name.clone());
        }
        if event.name == "overload" || tracked_names.contains(&event.name) {
            python_add_overload_binding(
                &mut bindings,
                event.name.clone(),
                event.effective_byte,
                event.is_overload_import,
            );
        }
        if tracked_modules.contains(&event.name) {
            python_add_overload_binding(
                &mut bindings,
                format!("{}.overload", event.name),
                event.effective_byte,
                event.is_overload_module_import,
            );
        }
    }
    Ok(bindings)
}

fn python_collect_overload_scopes(
    node: Node<'_>,
    source: &str,
    overload_aliases: &BTreeSet<String>,
    module_aliases: &BTreeSet<String>,
    scopes: &mut Vec<PythonOverloadScope>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(OVERLOAD_ALIAS_PHASE)?;
    }
    if matches!(node.kind(), "class_definition" | "function_definition") {
        let kind = if node.kind() == "class_definition" {
            PythonOverloadScopeKind::Class
        } else {
            PythonOverloadScopeKind::Function
        };
        if let Some(body) = node.child_by_field_name("body") {
            scopes.push(PythonOverloadScope {
                start_byte: body.start_byte(),
                end_byte: body.end_byte(),
                bindings: python_scope_binding_map(
                    node,
                    source,
                    kind,
                    overload_aliases,
                    module_aliases,
                    deadline,
                )?,
            });
        }
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        python_collect_overload_scopes(
            child,
            source,
            overload_aliases,
            module_aliases,
            scopes,
            deadline,
        )?;
    }
    Ok(())
}

pub(crate) fn python_overload_names(
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<PythonOverloadNames> {
    let mut names = BTreeMap::new();
    let mut tracked_names = BTreeSet::new();
    let mut tracked_module_aliases =
        BTreeSet::from(["typing".to_string(), "typing_extensions".to_string()]);
    for module in &tracked_module_aliases {
        python_add_overload_binding(&mut names, format!("{module}.overload"), 0, true);
    }
    let mut cursor = root.walk();
    for statement in root.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check(OVERLOAD_ALIAS_PHASE)?;
        }
        let aliases = python_typing_overload_import_aliases(statement, source, deadline)?;
        let bindings = python_module_binding_events(statement, source, deadline)?;
        for alias in &aliases {
            if let Some(deadline) = deadline {
                deadline.check(OVERLOAD_ALIAS_PHASE)?;
            }
            tracked_names.insert(alias.clone());
        }
        for binding in &bindings {
            if binding.is_overload_module_import {
                tracked_module_aliases.insert(binding.name.clone());
            }
        }
        for binding in bindings {
            if binding.is_overload_import {
                tracked_names.insert(binding.name.clone());
            }
            if binding.name == "overload" || tracked_names.contains(&binding.name) {
                python_add_overload_binding(
                    &mut names,
                    binding.name.clone(),
                    binding.effective_byte,
                    binding.is_overload_import,
                );
            }
            if tracked_module_aliases.contains(&binding.name) {
                python_add_overload_binding(
                    &mut names,
                    format!("{}.overload", binding.name),
                    binding.effective_byte,
                    binding.is_overload_module_import,
                );
            }
        }
    }
    let mut scopes = Vec::new();
    python_collect_overload_scopes(
        root,
        source,
        &tracked_names,
        &tracked_module_aliases,
        &mut scopes,
        deadline,
    )?;
    Ok(PythonOverloadNames {
        bindings: names,
        scopes,
    })
}

pub(crate) fn python_is_overload(
    node: Node<'_>,
    source: &str,
    overload_names: &PythonOverloadNames,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<bool> {
    if let Some(deadline) = deadline {
        deadline.check(OVERLOAD_DECORATOR_PHASE)?;
    }
    let Some(parent) = node
        .parent()
        .filter(|parent| parent.kind() == "decorated_definition")
    else {
        return Ok(false);
    };

    let mut cursor = parent.walk();
    for child in parent.named_children(&mut cursor) {
        if let Some(deadline) = deadline {
            deadline.check(OVERLOAD_DECORATOR_PHASE)?;
        }
        if child.kind() != "decorator" {
            continue;
        }
        let is_overload = node_text(child, source).ok().is_some_and(|text| {
            let decorator = text
                .trim()
                .strip_prefix('@')
                .unwrap_or_default()
                .trim_start();
            let name = decorator.split(['(', ' ', '\t']).next().unwrap_or_default();
            (name == "overload"
                && overload_names
                    .binding_before_node(name, node)
                    .unwrap_or(true))
                || overload_names.contains_before(name, node)
        });
        if is_overload {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::path::Path;

    use anyhow::{Result, bail};
    use tree_sitter::Node;

    use super::{
        python_is_overload, python_module_binding_events, python_overload_names,
        python_typing_overload_import_aliases,
    };
    use crate::deadline::DeadlineCheck;
    use crate::language::parse_document;

    fn find_definition<'tree>(node: Node<'tree>, source: &str, name: &str) -> Option<Node<'tree>> {
        if matches!(node.kind(), "class_definition" | "function_definition")
            && node
                .child_by_field_name("name")
                .and_then(|name_node| name_node.utf8_text(source.as_bytes()).ok())
                == Some(name)
        {
            return Some(node);
        }
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if let Some(found) = find_definition(child, source, name) {
                return Some(found);
            }
        }
        None
    }

    struct RejectDeadlineChecks;

    impl DeadlineCheck for RejectDeadlineChecks {
        fn check(&self, phase: &str) -> Result<()> {
            bail!("deadline check reached {phase}")
        }
    }

    struct RejectAfterChecks {
        allowed_checks: usize,
        checks: Cell<usize>,
    }

    impl RejectAfterChecks {
        fn new(allowed_checks: usize) -> Self {
            Self {
                allowed_checks,
                checks: Cell::new(0),
            }
        }
    }

    impl DeadlineCheck for RejectAfterChecks {
        fn check(&self, phase: &str) -> Result<()> {
            let checks = self.checks.get();
            self.checks.set(checks + 1);
            if checks >= self.allowed_checks {
                bail!("deadline check reached {phase}");
            }
            Ok(())
        }
    }

    #[test]
    fn overload_decorators_respect_class_and_function_scope_rebindings() {
        let source = r#"from typing import overload

class ShadowedClass:
    overload = custom_overload

    @overload
    def shadowed(self, key: str) -> str: ...

class RestoredClass:
    overload = custom_overload
    from typing import overload

    @overload
    def restored_class(self, key: str) -> str: ...

class QualifiedShadowedClass:
    typing = custom_typing

    @typing.overload
    def qualified(self, key: str) -> str: ...

def parameter_shadow(overload):
    @overload
    def inner(key: str) -> str: ...


def local_assignment_shadow():
    @overload
    def before(key: str) -> str: ...

    overload = custom_overload

    @overload
    def after(key: str) -> str: ...

def local_import_restore():
    from typing import overload as local_overload

    @local_overload
    def restored(key: str) -> str: ...

def global_rebinding():
    global overload

    @overload
    def global_before(key: str) -> str: ...

    overload = custom_overload

    @overload
    def global_after(key: str) -> str: ...
"#;
        let document = parse_document(Path::new("sample.py"), source).unwrap();
        let overload_names =
            python_overload_names(document.tree.root_node(), source, None).unwrap();

        for (name, expected) in [
            ("shadowed", false),
            ("restored_class", true),
            ("qualified", false),
            ("inner", false),
            ("before", false),
            ("after", false),
            ("restored", true),
            ("global_before", true),
            ("global_after", false),
        ] {
            let node = find_definition(document.tree.root_node(), source, name)
                .unwrap_or_else(|| panic!("definition {name} should be present"));
            assert_eq!(
                python_is_overload(node, source, &overload_names, None).unwrap(),
                expected,
                "unexpected overload classification for {name}"
            );
        }
    }

    #[test]
    fn overload_decorator_scan_checks_deadlines_before_traversing_decorators() {
        let source = "@decorator\ndef function(): ...\n";
        let document = parse_document(Path::new("sample.py"), source).unwrap();
        let decorated_definition = document
            .tree
            .root_node()
            .named_child(0)
            .expect("decorated definition should be present");
        let function = decorated_definition
            .child_by_field_name("definition")
            .expect("decorated definition should contain a function");
        let overload_names =
            python_overload_names(document.tree.root_node(), source, None).unwrap();

        let error = python_is_overload(
            function,
            source,
            &overload_names,
            Some(&RejectDeadlineChecks),
        )
        .expect_err("decorator traversal must check the deadline");

        assert!(
            error
                .to_string()
                .contains("classifying Python overload decorators")
        );
    }

    #[test]
    fn overload_alias_binding_scan_checks_deadlines_while_descending_patterns() {
        let source = "typed_overload, ignored = values\n";
        let document = parse_document(Path::new("sample.py"), source).unwrap();
        let statement = document
            .tree
            .root_node()
            .named_child(0)
            .expect("assignment statement should be present");
        let deadline = RejectAfterChecks::new(2);

        let error = python_module_binding_events(statement, source, Some(&deadline))
            .expect_err("nested pattern collection must check the deadline");

        assert!(
            error
                .to_string()
                .contains("collecting Python overload aliases")
        );
        assert_eq!(deadline.checks.get(), 3);
    }

    #[test]
    fn overload_alias_import_scan_checks_deadlines_for_each_import() {
        let source = "from typing import Any, overload as typed_overload\n";
        let document = parse_document(Path::new("sample.py"), source).unwrap();
        let statement = document
            .tree
            .root_node()
            .named_child(0)
            .expect("import statement should be present");
        let deadline = RejectAfterChecks::new(1);

        let error = python_typing_overload_import_aliases(statement, source, Some(&deadline))
            .expect_err("each imported name must check the deadline");

        assert!(
            error
                .to_string()
                .contains("collecting Python overload aliases")
        );
        assert_eq!(deadline.checks.get(), 2);
    }

    #[test]
    fn overload_alias_import_scan_checks_deadlines_for_wildcard_import() {
        let source = "from typing import *\n";
        let document = parse_document(Path::new("sample.py"), source).unwrap();
        let statement = document
            .tree
            .root_node()
            .named_child(0)
            .expect("import statement should be present");
        let deadline = RejectAfterChecks::new(1);

        let error = python_typing_overload_import_aliases(statement, source, Some(&deadline))
            .expect_err("wildcard import scanning must check the deadline");

        assert!(
            error
                .to_string()
                .contains("collecting Python overload aliases")
        );
        assert_eq!(deadline.checks.get(), 2);
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
