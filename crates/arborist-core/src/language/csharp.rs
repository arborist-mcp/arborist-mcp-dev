use anyhow::Result;
use tree_sitter::Node;

use super::node_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileTypeAliasImport {
    pub(crate) scope_path: Option<String>,
    pub(crate) local_name: String,
    pub(crate) semantic_type_path: String,
    /// Raw top-level type-argument spellings of the alias target's final
    /// type segment, such as `["HelperA"]` for
    /// `using Alias = Demo.Derived<HelperA>;`; empty for non-generic alias
    /// targets.
    pub(crate) raw_generic_argument_spellings: Vec<String>,
    /// Raw top-level type-argument spellings of every dotted segment that
    /// precedes the final segment of the alias target, outermost first, such
    /// as `[["HelperA"]]` for
    /// `using Alias = Demo.Outer<HelperA>.Inner<HelperB>;`; empty when the
    /// target has no enclosing generic segments.
    pub(crate) raw_enclosing_generic_argument_spellings: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpGlobalTypeAliasImport {
    pub(crate) local_name: String,
    pub(crate) semantic_type_path: String,
    /// Raw top-level type-argument spellings of the alias target's final
    /// type segment, such as `["HelperA"]` for
    /// `global using GlobalAlias = Demo.Derived<HelperA>;`; empty for
    /// non-generic alias targets.
    pub(crate) raw_generic_argument_spellings: Vec<String>,
    /// Raw top-level type-argument spellings of every dotted segment that
    /// precedes the final segment of the alias target, outermost first; empty
    /// when the target has no enclosing generic segments.
    pub(crate) raw_enclosing_generic_argument_spellings: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileStaticTypeImport {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileNamespaceImport {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_namespace_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileBaseType {
    pub(crate) type_range: (usize, usize),
    pub(crate) semantic_base_type_path: String,
    pub(crate) is_global_qualified: bool,
    /// Raw top-level type-argument spellings from the base-list spelling,
    /// such as `["T"]` for `class Derived<T> : Box<T>`; empty for non-generic
    /// base spellings. Generic-inheritance resolution substitutes a
    /// constructed receiver's concrete arguments for these spellings to
    /// compose the base type's arguments.
    pub(crate) generic_argument_spellings: Vec<String>,
    /// Raw top-level type-argument spellings of every dotted segment that
    /// precedes the final segment of a nested generic base spelling,
    /// outermost first, such as `[["HelperA"]]` for
    /// `class Derived : Outer<HelperA>.Inner<HelperB>`; empty when the base
    /// spelling has no enclosing generic segments. Generic-inheritance
    /// resolution substitutes a constructed receiver's concrete arguments
    /// for these spellings to compose the enclosing base type's arguments.
    pub(crate) enclosing_generic_argument_spellings: Vec<Vec<String>>,
}

pub(crate) fn csharp_file_base_types(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpFileBaseType>> {
    fn collect(
        node: Node<'_>,
        source: &str,
        base_types: &mut Vec<CSharpFileBaseType>,
    ) -> Result<()> {
        if matches!(node.kind(), "class_declaration" | "record_declaration") {
            let mut cursor = node.walk();
            if let Some(base_list) = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "base_list")
            {
                let mut base_list_cursor = base_list.walk();
                if let Some(base_type) = base_list.named_children(&mut base_list_cursor).next() {
                    let base_type_text = node_text(base_type, source)?.trim();
                    let is_global_qualified = base_type_text.starts_with("global::");
                    if let Some(semantic_base_type_path) =
                        csharp_base_type_semantic_path(base_type_text)
                    {
                        base_types.push(CSharpFileBaseType {
                            type_range: (node.start_byte(), node.end_byte()),
                            semantic_base_type_path,
                            is_global_qualified,
                            generic_argument_spellings: csharp_generic_type_arguments(
                                base_type_text,
                            )
                            .unwrap_or_default(),
                            enclosing_generic_argument_spellings:
                                csharp_generic_type_arguments_per_segment(base_type_text)
                                    .map(|segments| {
                                        segments[..segments.len().saturating_sub(1)].to_vec()
                                    })
                                    .unwrap_or_default(),
                        });
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, source, base_types)?;
        }
        Ok(())
    }

    let mut base_types = Vec::new();
    collect(root, source, &mut base_types)?;
    Ok(base_types)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileInterfaceParent {
    pub(crate) declaration_range: (usize, usize),
    pub(crate) semantic_type_path: String,
    pub(crate) is_global_qualified: bool,
    /// Raw top-level type-argument spellings from the parent-interface
    /// spelling, such as `["T"]` for `interface IGeneric<T> : IBase<T>`;
    /// empty for non-generic parent spellings. Generic-inheritance
    /// resolution substitutes an interface receiver's concrete arguments for
    /// these spellings to compose the parent interface's arguments.
    pub(crate) generic_argument_spellings: Vec<String>,
}

/// Collects the direct parent interfaces of every interface declaration in a
/// C# source file. An `interface_declaration` may extend several interfaces
/// through its `base_list`; every `type` entry is recorded against the
/// declaring interface's byte range so the resolver can walk extends chains.
pub(crate) fn csharp_file_interface_parents(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpFileInterfaceParent>> {
    fn collect(
        node: Node<'_>,
        source: &str,
        parents: &mut Vec<CSharpFileInterfaceParent>,
    ) -> Result<()> {
        if node.kind() == "interface_declaration" {
            let mut cursor = node.walk();
            if let Some(base_list) = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "base_list")
            {
                let mut base_list_cursor = base_list.walk();
                for base_type in base_list.named_children(&mut base_list_cursor) {
                    // Concrete base type nodes (`identifier`, `generic_name`,
                    // `qualified_name`, `alias_qualified_name`, ...) normalize
                    // to a semantic path; argument lists and primary-constructor
                    // base types are rejected by the normalization.
                    let base_type_text = node_text(base_type, source)?.trim();
                    let is_global_qualified = base_type_text.starts_with("global::");
                    if let Some(semantic_type_path) =
                        csharp_generic_type_semantic_path(base_type_text)
                    {
                        parents.push(CSharpFileInterfaceParent {
                            declaration_range: (node.start_byte(), node.end_byte()),
                            semantic_type_path,
                            is_global_qualified,
                            generic_argument_spellings: csharp_generic_type_arguments(
                                base_type_text,
                            )
                            .unwrap_or_default(),
                        });
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, source, parents)?;
        }
        Ok(())
    }

    let mut parents = Vec::new();
    collect(root, source, &mut parents)?;
    Ok(parents)
}

pub(crate) fn csharp_file_type_alias_imports(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpFileTypeAliasImport>> {
    let mut imports = Vec::new();
    for (directive, scope_path) in csharp_scoped_using_directives(root, source)? {
        if let Some(import) =
            csharp_type_alias_import_from_directive(directive, scope_path.as_deref(), source)?
        {
            imports.push(import);
        }
    }
    Ok(imports)
}

fn csharp_type_alias_import_from_directive(
    directive: Node<'_>,
    scope_path: Option<&str>,
    source: &str,
) -> Result<Option<CSharpFileTypeAliasImport>> {
    let directive_text = node_text(directive, source)?.trim();
    if !is_file_type_alias_directive(directive_text) {
        return Ok(None);
    }
    let Some(name) = directive.child_by_field_name("name") else {
        return Ok(None);
    };
    let local_name = node_text(name, source)?.trim();
    if !is_safe_csharp_identifier(local_name) {
        return Ok(None);
    }
    let mut children_cursor = directive.walk();
    let Some(target) = directive
        .named_children(&mut children_cursor)
        .find(|child| *child != name)
    else {
        return Ok(None);
    };
    let target_path = node_text(target, source)?.trim();
    let Some(semantic_type_path) = csharp_generic_type_semantic_path(target_path) else {
        return Ok(None);
    };
    Ok(Some(CSharpFileTypeAliasImport {
        scope_path: scope_path.map(str::to_string),
        local_name: local_name.to_string(),
        semantic_type_path,
        raw_generic_argument_spellings: csharp_generic_type_arguments(target_path)
            .unwrap_or_default(),
        raw_enclosing_generic_argument_spellings: csharp_generic_type_arguments_per_segment(
            target_path,
        )
        .map(|segments| segments[..segments.len().saturating_sub(1)].to_vec())
        .unwrap_or_default(),
    }))
}

pub(crate) fn csharp_file_static_type_imports(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpFileStaticTypeImport>> {
    let mut imports = Vec::new();
    for (directive, scope_path) in csharp_scoped_using_directives(root, source)? {
        let directive_text = node_text(directive, source)?.trim();
        if !is_file_static_type_directive(directive_text)
            || directive.child_by_field_name("name").is_some()
        {
            continue;
        }
        let mut children_cursor = directive.walk();
        let Some(target) = directive.named_children(&mut children_cursor).next() else {
            continue;
        };
        let target_path = node_text(target, source)?.trim();
        let Some(semantic_type_path) = csharp_generic_type_semantic_path(target_path) else {
            continue;
        };
        imports.push(CSharpFileStaticTypeImport {
            scope_path,
            semantic_type_path,
        });
    }
    Ok(imports)
}

pub(crate) fn csharp_global_static_type_imports(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<String>> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for directive in root.named_children(&mut cursor) {
        if directive.kind() != "using_directive" {
            continue;
        }
        let directive_text = node_text(directive, source)?.trim();
        if !is_global_static_type_directive(directive_text)
            || directive.child_by_field_name("name").is_some()
        {
            continue;
        }
        let mut children_cursor = directive.walk();
        let Some(target) = directive.named_children(&mut children_cursor).next() else {
            continue;
        };
        let target_path = node_text(target, source)?.trim();
        if let Some(semantic_type_path) = csharp_generic_type_semantic_path(target_path) {
            imports.push(semantic_type_path);
        }
    }
    Ok(imports)
}

pub(crate) fn csharp_global_namespace_imports(root: Node<'_>, source: &str) -> Result<Vec<String>> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for directive in root.named_children(&mut cursor) {
        if directive.kind() != "using_directive" {
            continue;
        }
        let directive_text = node_text(directive, source)?.trim();
        if !is_global_namespace_directive(directive_text)
            || directive.child_by_field_name("name").is_some()
        {
            continue;
        }
        let mut children_cursor = directive.walk();
        let Some(target) = directive.named_children(&mut children_cursor).next() else {
            continue;
        };
        let target_path = node_text(target, source)?.trim();
        if let Some(semantic_namespace_path) = csharp_qualified_type_semantic_path(target_path) {
            imports.push(semantic_namespace_path);
        }
    }
    Ok(imports)
}

pub(crate) fn csharp_global_type_alias_imports(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpGlobalTypeAliasImport>> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for directive in root.named_children(&mut cursor) {
        if directive.kind() != "using_directive" {
            continue;
        }
        let directive_text = node_text(directive, source)?.trim();
        if !is_global_type_alias_directive(directive_text) {
            continue;
        }
        let Some(name) = directive.child_by_field_name("name") else {
            continue;
        };
        let local_name = node_text(name, source)?.trim();
        if !is_safe_csharp_identifier(local_name) {
            continue;
        }
        let mut children_cursor = directive.walk();
        let Some(target) = directive
            .named_children(&mut children_cursor)
            .find(|child| *child != name)
        else {
            continue;
        };
        let target_path = node_text(target, source)?.trim();
        if let Some(semantic_type_path) = csharp_generic_type_semantic_path(target_path) {
            imports.push(CSharpGlobalTypeAliasImport {
                local_name: local_name.to_string(),
                semantic_type_path,
                raw_generic_argument_spellings: csharp_generic_type_arguments(target_path)
                    .unwrap_or_default(),
                raw_enclosing_generic_argument_spellings:
                    csharp_generic_type_arguments_per_segment(target_path)
                        .map(|segments| segments[..segments.len().saturating_sub(1)].to_vec())
                        .unwrap_or_default(),
            });
        }
    }
    Ok(imports)
}

fn csharp_scoped_using_directives<'tree>(
    root: Node<'tree>,
    source: &str,
) -> Result<Vec<(Node<'tree>, Option<String>)>> {
    fn collect_namespace_using_directives<'tree>(
        namespace: Node<'tree>,
        parent_scope_path: Option<&str>,
        source: &str,
        directives: &mut Vec<(Node<'tree>, Option<String>)>,
    ) -> Result<()> {
        let Some(name) = namespace.child_by_field_name("name") else {
            return Ok(());
        };
        let namespace_name = node_text(name, source)?.trim();
        let Some(namespace_path) = csharp_qualified_type_semantic_path(namespace_name) else {
            return Ok(());
        };
        let scope_path = parent_scope_path
            .map(|parent_scope_path| format!("{parent_scope_path}::{namespace_path}"))
            .unwrap_or(namespace_path);
        let Some(body) = namespace.child_by_field_name("body") else {
            return Ok(());
        };
        let mut cursor = body.walk();
        for child in body.named_children(&mut cursor) {
            if child.kind() == "using_directive" {
                directives.push((child, Some(scope_path.clone())));
            } else if child.kind() == "namespace_declaration" {
                collect_namespace_using_directives(
                    child,
                    Some(scope_path.as_str()),
                    source,
                    directives,
                )?;
            }
        }
        Ok(())
    }

    let mut directives = Vec::new();
    let mut root_scope_path = None;
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        if child.kind() == "file_scoped_namespace_declaration" {
            let Some(name) = child.child_by_field_name("name") else {
                continue;
            };
            let namespace_name = node_text(name, source)?.trim();
            root_scope_path = csharp_qualified_type_semantic_path(namespace_name);
        } else if child.kind() == "using_directive" {
            directives.push((child, root_scope_path.clone()));
        } else if child.kind() == "namespace_declaration" {
            collect_namespace_using_directives(child, None, source, &mut directives)?;
        }
    }
    Ok(directives)
}

pub(crate) fn csharp_file_namespace_imports(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpFileNamespaceImport>> {
    let mut imports = Vec::new();
    for (directive, scope_path) in csharp_scoped_using_directives(root, source)? {
        let directive_text = node_text(directive, source)?.trim();
        if !is_file_namespace_directive(directive_text)
            || directive.child_by_field_name("name").is_some()
        {
            continue;
        }
        let mut children_cursor = directive.walk();
        let Some(target) = directive.named_children(&mut children_cursor).next() else {
            continue;
        };
        let target_path = node_text(target, source)?.trim();
        let Some(semantic_namespace_path) = csharp_qualified_type_semantic_path(target_path) else {
            continue;
        };
        imports.push(CSharpFileNamespaceImport {
            scope_path,
            semantic_namespace_path,
        });
    }
    Ok(imports)
}

fn is_file_type_alias_directive(directive_text: &str) -> bool {
    directive_text.starts_with("using ")
        && !directive_text.starts_with("global using ")
        && !directive_text
            .split_whitespace()
            .any(|token| token == "unsafe")
}

fn is_file_namespace_directive(directive_text: &str) -> bool {
    directive_text.starts_with("using ")
        && !directive_text.starts_with("global using ")
        && !directive_text.contains('=')
        && !directive_text
            .split_whitespace()
            .any(|token| matches!(token, "static" | "unsafe"))
}

fn is_file_static_type_directive(directive_text: &str) -> bool {
    directive_text.starts_with("using static ")
        && directive_text
            .split_whitespace()
            .filter(|token| *token == "static")
            .count()
            == 1
        && !directive_text.starts_with("global using ")
        && !directive_text
            .split_whitespace()
            .any(|token| token == "unsafe")
}

fn is_global_static_type_directive(directive_text: &str) -> bool {
    directive_text.starts_with("global using static ")
        && directive_text
            .split_whitespace()
            .filter(|token| *token == "static")
            .count()
            == 1
        && !directive_text
            .split_whitespace()
            .any(|token| token == "unsafe")
}

fn is_global_namespace_directive(directive_text: &str) -> bool {
    directive_text.starts_with("global using ")
        && !directive_text.starts_with("global using static ")
        && !directive_text.contains('=')
        && !directive_text
            .split_whitespace()
            .any(|token| token == "unsafe")
}

fn is_global_type_alias_directive(directive_text: &str) -> bool {
    directive_text.starts_with("global using ")
        && directive_text.contains('=')
        && !directive_text
            .split_whitespace()
            .any(|token| matches!(token, "static" | "unsafe"))
}

fn csharp_base_type_semantic_path(type_path: &str) -> Option<String> {
    csharp_generic_type_semantic_path(type_path)
}

pub(crate) fn csharp_generic_type_semantic_path(type_path: &str) -> Option<String> {
    csharp_qualified_type_semantic_path(&strip_csharp_generic_type_arguments(type_path)?)
}

fn strip_csharp_generic_type_arguments(type_path: &str) -> Option<String> {
    let mut normalized = String::with_capacity(type_path.len());
    let mut generic_argument_contents = Vec::new();
    let mut generic_just_closed = false;

    for character in type_path.chars() {
        match character {
            '<' if generic_just_closed => return None,
            '<' => {
                generic_argument_contents.push(false);
                generic_just_closed = false;
            }
            '>' => {
                let has_content = generic_argument_contents.pop()?;
                if !has_content {
                    return None;
                }
                if let Some(parent_has_content) = generic_argument_contents.last_mut() {
                    *parent_has_content = true;
                } else {
                    generic_just_closed = true;
                }
            }
            _ if generic_argument_contents.is_empty() => {
                if generic_just_closed && (character.is_ascii_alphanumeric() || character == '_') {
                    return None;
                }
                normalized.push(character);
                if character == '.' {
                    generic_just_closed = false;
                }
            }
            _ if character.is_ascii_alphanumeric() || character == '_' => {
                if let Some(has_content) = generic_argument_contents.last_mut() {
                    *has_content = true;
                }
            }
            _ => {}
        }
    }

    generic_argument_contents.is_empty().then_some(normalized)
}

/// Parses the top-level type-argument spellings of the last type segment of
/// a constructed generic spelling such as `Box<Helper>` (`["Helper"]`),
/// `Box<Dictionary<string, int>>` (`["Dictionary<string, int>"]`),
/// `Pair<A, B>` (`["A", "B"]`), or the nested `Outer<Helper>.Inner<Helper>`
/// (`["Helper"]`, the inner type's own arguments aligned with its own
/// type-parameter list). Non-generic spellings, empty argument lists, and
/// malformed or trailing lists return `None` and fail closed.
pub(crate) fn csharp_generic_type_arguments(type_path: &str) -> Option<Vec<String>> {
    let last_segment = csharp_last_type_segment(type_path)?;
    let open = last_segment.find('<')?;
    let mut arguments = Vec::new();
    let mut depth = 0usize;
    let mut current = String::new();
    for (index, character) in last_segment[open + 1..].char_indices() {
        match character {
            '<' => {
                depth += 1;
                current.push('<');
            }
            '>' => {
                if depth == 0 {
                    let argument = current.trim();
                    if argument.is_empty() {
                        return None;
                    }
                    arguments.push(argument.to_string());
                    let remainder = &last_segment[open + 1 + index + character.len_utf8()..];
                    if !remainder.is_empty() {
                        return None;
                    }
                    return Some(arguments);
                }
                depth -= 1;
                current.push('>');
            }
            ',' if depth == 0 => {
                let argument = current.trim();
                if argument.is_empty() {
                    return None;
                }
                arguments.push(argument.to_string());
                current.clear();
            }
            _ => current.push(character),
        }
    }
    None
}

/// Parses the top-level type-argument spellings of every dotted segment of a
/// constructed generic spelling such as `Outer<Helper>.Inner<Helper>`
/// (`[["Helper"], ["Helper"]]`), `Box<Helper>` (`[["Helper"]]`), or
/// `NonGenericOuter.GenInner<Helper>` (`[[], ["Helper"]]`). Non-generic
/// segments record empty lists. Malformed, empty, or trailing argument
/// lists return `None` and fail closed.
pub(crate) fn csharp_generic_type_arguments_per_segment(
    type_path: &str,
) -> Option<Vec<Vec<String>>> {
    let mut segments = Vec::new();
    let mut depth = 0usize;
    let mut last_start = 0usize;
    for (index, character) in type_path.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            '.' if depth == 0 => {
                if index == last_start {
                    return None;
                }
                segments.push(&type_path[last_start..index]);
                last_start = index + 1;
            }
            _ => {}
        }
    }
    if last_start == type_path.len() {
        return None;
    }
    segments.push(&type_path[last_start..]);
    let mut arguments = Vec::with_capacity(segments.len());
    for segment in segments {
        arguments.push(if segment.contains('<') {
            csharp_generic_type_arguments(segment)?
        } else {
            Vec::new()
        });
    }
    Some(arguments)
}

/// Splits a qualified type spelling on `.` outside any generic argument list
/// and returns the final segment, so `Outer<Helper>.Inner<Helper>` yields
/// `Inner<Helper>` and `Box<Helper>` yields itself. Empty final segments and
/// unbalanced argument lists return `None` and fail closed.
fn csharp_last_type_segment(type_path: &str) -> Option<&str> {
    let mut depth = 0usize;
    let mut last_start = 0usize;
    for (index, character) in type_path.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => depth = depth.checked_sub(1)?,
            '.' if depth == 0 => {
                if index == last_start {
                    return None;
                }
                last_start = index + 1;
            }
            _ => {}
        }
    }
    let last_segment = &type_path[last_start..];
    if last_segment.is_empty() {
        return None;
    }
    Some(last_segment)
}

fn csharp_qualified_type_semantic_path(type_path: &str) -> Option<String> {
    let type_path = type_path.strip_prefix("global::").unwrap_or(type_path);
    if type_path.is_empty()
        || type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return None;
    }
    Some(type_path.replace('.', "::"))
}

fn is_safe_csharp_identifier(identifier: &str) -> bool {
    let mut characters = identifier.chars();
    matches!(characters.next(), Some(character) if character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        csharp_file_base_types, csharp_file_interface_parents, csharp_file_namespace_imports,
        csharp_file_static_type_imports, csharp_file_type_alias_imports,
        csharp_global_namespace_imports, csharp_global_static_type_imports,
        csharp_global_type_alias_imports,
    };
    use crate::language::parse_document;

    #[test]
    fn normalizes_safe_csharp_generic_type_paths() {
        assert_eq!(
            super::csharp_generic_type_semantic_path("global::Demo.Outer<int>.Base<string>"),
            Some("Demo::Outer::Base".to_string())
        );
        assert_eq!(
            super::csharp_generic_type_semantic_path("Base<System.Collections.Generic.List<int>>"),
            Some("Base".to_string())
        );
        assert_eq!(super::csharp_generic_type_semantic_path("Base<>"), None);
        assert_eq!(
            super::csharp_generic_type_semantic_path("Base<int>[]"),
            None
        );
        assert_eq!(super::csharp_generic_type_semantic_path("Base<int"), None);
        assert_eq!(
            super::csharp_generic_type_semantic_path("Base<int>Suffix"),
            None
        );
    }

    #[test]
    fn collects_last_segment_generic_type_arguments() {
        assert_eq!(
            super::csharp_generic_type_arguments("Box<Helper>"),
            Some(vec!["Helper".to_string()])
        );
        assert_eq!(
            super::csharp_generic_type_arguments("Outer<Helper>.Inner<Helper>"),
            Some(vec!["Helper".to_string()])
        );
        assert_eq!(
            super::csharp_generic_type_arguments("Outer<Helper>.Inner<int>"),
            Some(vec!["int".to_string()])
        );
        assert_eq!(
            super::csharp_generic_type_arguments("NonGenericOuter.GenInner<Helper>"),
            Some(vec!["Helper".to_string()])
        );
        assert_eq!(
            super::csharp_generic_type_arguments("global::Demo.Outer<int>.Base<string>"),
            Some(vec!["string".to_string()])
        );
        assert_eq!(
            super::csharp_generic_type_arguments("Box<Dictionary<string, int>>"),
            Some(vec!["Dictionary<string, int>".to_string()])
        );
        assert_eq!(
            super::csharp_generic_type_arguments("Pair<A, B>"),
            Some(vec!["A".to_string(), "B".to_string()])
        );
        assert_eq!(super::csharp_generic_type_arguments("Base"), None);
        assert_eq!(
            super::csharp_generic_type_arguments("Outer<Helper>.Inner"),
            None
        );
        assert_eq!(super::csharp_generic_type_arguments("Base<>"), None);
        assert_eq!(super::csharp_generic_type_arguments("Base<int"), None);
        assert_eq!(super::csharp_generic_type_arguments("Base<int>[]"), None);
    }

    #[test]
    fn collects_per_segment_generic_type_arguments() {
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Outer<Helper>.Inner<Helper>"),
            Some(vec![vec!["Helper".to_string()], vec!["Helper".to_string()]])
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Box<Helper>"),
            Some(vec![vec!["Helper".to_string()]])
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("NonGenericOuter.GenInner<Helper>"),
            Some(vec![Vec::new(), vec!["Helper".to_string()]])
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Outer<Helper>.Plain"),
            Some(vec![vec!["Helper".to_string()], Vec::new()])
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Pair<A, B>.Nested<C>"),
            Some(vec![
                vec!["A".to_string(), "B".to_string()],
                vec!["C".to_string()]
            ])
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment(
                "global::Demo.Outer<int>.Base<string>"
            ),
            Some(vec![
                Vec::new(),
                vec!["int".to_string()],
                vec!["string".to_string()]
            ])
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Base"),
            Some(vec![Vec::new()])
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Base<int"),
            None
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Base<>"),
            None
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Base<int>[]"),
            None
        );
        assert_eq!(
            super::csharp_generic_type_arguments_per_segment("Outer.Inner"),
            Some(vec![Vec::new(), Vec::new()])
        );
    }

    #[test]
    fn collects_only_safe_csharp_base_types() {
        let source = r#"
class Base {}
class SimpleDerived : Base {}
class GlobalDerived : global::Demo.Base {}
class QualifiedDerived : Demo.Base {}
class GenericDerived : Base<int> {}
"#;
        let document = parse_document(Path::new("Derived.cs"), source).unwrap();
        let base_types = csharp_file_base_types(document.tree.root_node(), source).unwrap();

        assert_eq!(
            base_types
                .iter()
                .map(|base_type| {
                    (
                        base_type.semantic_base_type_path.as_str(),
                        base_type.is_global_qualified,
                    )
                })
                .collect::<Vec<_>>(),
            [
                ("Base", false),
                ("Demo::Base", true),
                ("Demo::Base", false),
                ("Base", false),
            ]
        );
    }

    #[test]
    fn collects_interface_parent_types() {
        let source = r#"
interface IBase {}
interface ISecond : IBase {}
interface IGeneric : IBase<int> {}
interface IQualified : Demo.IBase {}
interface IGlobal : global::Demo.IBase {}
interface IMultiple : IBase, ISecond {}
class NotAnInterface : IBase {}
"#;
        let document = parse_document(Path::new("Interfaces.cs"), source).unwrap();
        let parents = csharp_file_interface_parents(document.tree.root_node(), source).unwrap();

        assert_eq!(
            parents
                .iter()
                .map(|parent| {
                    (
                        parent.semantic_type_path.as_str(),
                        parent.is_global_qualified,
                    )
                })
                .collect::<Vec<_>>(),
            [
                ("IBase", false),
                ("IBase", false),
                ("Demo::IBase", false),
                ("Demo::IBase", true),
                ("IBase", false),
                ("ISecond", false),
            ]
        );
        // `class NotAnInterface : IBase {}` contributes no interface parents,
        // and `interface IBase {}` has no base list.
        assert_eq!(parents.len(), 6);
    }

    #[test]
    fn collects_only_safe_file_type_alias_imports() {
        let source = r#"
using HelperAlias = Demo.Utility.Helper;
using GlobalAlias = global::Demo.Utility.GlobalHelper;
using GenericAlias = Demo.Utility.GenericHelper<int>;
global using ProjectAlias = Demo.Utility.ProjectHelper;
using unsafe UnsafeAlias = Demo.Utility.UnsafeHelper;
using Demo.Utility;

namespace Demo.App;
class Caller {}
"#;
        let document = parse_document(Path::new("Caller.cs"), source).unwrap();
        let imports = csharp_file_type_alias_imports(document.tree.root_node(), source).unwrap();

        assert_eq!(
            imports,
            vec![
                super::CSharpFileTypeAliasImport {
                    scope_path: None,
                    local_name: "HelperAlias".to_string(),
                    semantic_type_path: "Demo::Utility::Helper".to_string(),
                    raw_generic_argument_spellings: Vec::new(),
                    raw_enclosing_generic_argument_spellings: vec![vec![], vec![]],
                },
                super::CSharpFileTypeAliasImport {
                    scope_path: None,
                    local_name: "GlobalAlias".to_string(),
                    semantic_type_path: "Demo::Utility::GlobalHelper".to_string(),
                    raw_generic_argument_spellings: Vec::new(),
                    raw_enclosing_generic_argument_spellings: vec![vec![], vec![]],
                },
                super::CSharpFileTypeAliasImport {
                    scope_path: None,
                    local_name: "GenericAlias".to_string(),
                    semantic_type_path: "Demo::Utility::GenericHelper".to_string(),
                    raw_generic_argument_spellings: vec!["int".to_string()],
                    raw_enclosing_generic_argument_spellings: vec![vec![], vec![]],
                },
            ]
        );

        assert_eq!(
            csharp_file_static_type_imports(document.tree.root_node(), source).unwrap(),
            Vec::<super::CSharpFileStaticTypeImport>::new()
        );
    }

    #[test]
    fn collects_only_safe_file_static_type_imports() {
        let source = r#"
using static Demo.Utility.Helper;
using static global::Demo.Utility.GlobalHelper;
global using static Demo.Utility.ProjectHelper;
using static unsafe Demo.Utility.UnsafeHelper;
using static static Demo.Utility.DuplicateStaticHelper;
using static Demo.Utility.GenericHelper<int>;
using static Demo.Utility.InvalidGenericHelper<>;
using Demo.Utility;

namespace Demo.App;
class Caller {}
"#;
        let document = parse_document(Path::new("Caller.cs"), source).unwrap();
        let imports = csharp_file_static_type_imports(document.tree.root_node(), source).unwrap();

        assert_eq!(
            imports,
            vec![
                super::CSharpFileStaticTypeImport {
                    scope_path: None,
                    semantic_type_path: "Demo::Utility::Helper".to_string(),
                },
                super::CSharpFileStaticTypeImport {
                    scope_path: None,
                    semantic_type_path: "Demo::Utility::GlobalHelper".to_string(),
                },
                super::CSharpFileStaticTypeImport {
                    scope_path: None,
                    semantic_type_path: "Demo::Utility::GenericHelper".to_string(),
                },
            ]
        );
    }

    #[test]
    fn collects_namespace_scoped_static_type_imports() {
        let block_scoped_source = r#"
namespace Demo.App {
    using static Demo.Utility.BlockHelpers;
    class Caller {}
}
"#;
        let document = parse_document(Path::new("BlockScoped.cs"), block_scoped_source).unwrap();
        assert_eq!(
            csharp_file_static_type_imports(document.tree.root_node(), block_scoped_source)
                .unwrap(),
            vec![super::CSharpFileStaticTypeImport {
                scope_path: Some("Demo::App".to_string()),
                semantic_type_path: "Demo::Utility::BlockHelpers".to_string(),
            }]
        );

        let file_scoped_source = r#"
namespace Demo.App;
using static Demo.Utility.FileHelpers;
class Caller {}
"#;
        let document = parse_document(Path::new("FileScoped.cs"), file_scoped_source).unwrap();
        assert_eq!(
            csharp_file_static_type_imports(document.tree.root_node(), file_scoped_source).unwrap(),
            vec![super::CSharpFileStaticTypeImport {
                scope_path: Some("Demo::App".to_string()),
                semantic_type_path: "Demo::Utility::FileHelpers".to_string(),
            }]
        );
    }

    #[test]
    fn collects_only_safe_root_global_static_type_imports() {
        let source = r#"
global using static Demo.Utility.Helper;
global using static global::Demo.Utility.GlobalHelper;
global using static Demo.Utility.GenericHelper<int>;
global using static unsafe Demo.Utility.UnsafeHelper;
global using static static Demo.Utility.DuplicateStaticHelper;
global using Demo.Utility;
global using HelperAlias = Demo.Utility.Helper;
using static Demo.Utility.FileHelper;

namespace Demo.App;
class Caller {}
"#;
        let document = parse_document(Path::new("Caller.cs"), source).unwrap();

        assert_eq!(
            csharp_global_static_type_imports(document.tree.root_node(), source).unwrap(),
            vec![
                "Demo::Utility::Helper".to_string(),
                "Demo::Utility::GlobalHelper".to_string(),
                "Demo::Utility::GenericHelper".to_string(),
            ]
        );
    }

    #[test]
    fn collects_only_safe_root_global_namespace_imports() {
        let source = r#"
global using Demo.Utility;
global using global::Demo.Shared;
global using static Demo.Utility.Helper;
global using Demo.Utility.Generic<int>;
global using unsafe Demo.Unsafe;
global using Alias = Demo.Utility.Helper;
using Demo.File;

namespace Demo.App;
class Caller {}
"#;
        let document = parse_document(Path::new("Caller.cs"), source).unwrap();

        assert_eq!(
            csharp_global_namespace_imports(document.tree.root_node(), source).unwrap(),
            vec!["Demo::Utility".to_string(), "Demo::Shared".to_string()]
        );
    }

    #[test]
    fn collects_only_safe_root_global_type_alias_imports() {
        let source = r#"
global using HelperAlias = Demo.Utility.Helper;
global using GlobalAlias = global::Demo.Utility.GlobalHelper;
global using static Demo.Utility.Helper;
global using Demo.Utility;
global using GenericAlias = Demo.Utility.Generic<int>;
global using unsafe UnsafeAlias = Demo.Utility.UnsafeHelper;
using FileAlias = Demo.Utility.FileHelper;

namespace Demo.App;
class Caller {}
"#;
        let document = parse_document(Path::new("Caller.cs"), source).unwrap();

        assert_eq!(
            csharp_global_type_alias_imports(document.tree.root_node(), source).unwrap(),
            vec![
                super::CSharpGlobalTypeAliasImport {
                    local_name: "HelperAlias".to_string(),
                    semantic_type_path: "Demo::Utility::Helper".to_string(),
                    raw_generic_argument_spellings: Vec::new(),
                    raw_enclosing_generic_argument_spellings: vec![vec![], vec![]],
                },
                super::CSharpGlobalTypeAliasImport {
                    local_name: "GlobalAlias".to_string(),
                    semantic_type_path: "Demo::Utility::GlobalHelper".to_string(),
                    raw_generic_argument_spellings: Vec::new(),
                    raw_enclosing_generic_argument_spellings: vec![vec![], vec![]],
                },
                super::CSharpGlobalTypeAliasImport {
                    local_name: "GenericAlias".to_string(),
                    semantic_type_path: "Demo::Utility::Generic".to_string(),
                    raw_generic_argument_spellings: vec!["int".to_string()],
                    raw_enclosing_generic_argument_spellings: vec![vec![], vec![]],
                },
            ]
        );
    }

    #[test]
    fn collects_only_safe_file_namespace_imports() {
        let source = r#"
using Demo.Utility;
using global::Demo.Shared;
using Alias = Demo.Utility.Helper;
using static Demo.Utility.Helper;
global using Demo.Project;
using unsafe Demo.Unsafe;

namespace Demo.App;
class Caller {}
"#;
        let document = parse_document(Path::new("Caller.cs"), source).unwrap();
        let imports = csharp_file_namespace_imports(document.tree.root_node(), source).unwrap();

        assert_eq!(
            imports,
            vec![
                super::CSharpFileNamespaceImport {
                    scope_path: None,
                    semantic_namespace_path: "Demo::Utility".to_string(),
                },
                super::CSharpFileNamespaceImport {
                    scope_path: None,
                    semantic_namespace_path: "Demo::Shared".to_string(),
                },
            ]
        );
    }

    #[test]
    fn collects_namespace_scoped_namespace_imports() {
        let block_scoped_source = r#"
namespace Demo.App {
    using Demo.Utility;
    class Caller {}
}
"#;
        let document = parse_document(Path::new("BlockScoped.cs"), block_scoped_source).unwrap();
        assert_eq!(
            csharp_file_namespace_imports(document.tree.root_node(), block_scoped_source).unwrap(),
            vec![super::CSharpFileNamespaceImport {
                scope_path: Some("Demo::App".to_string()),
                semantic_namespace_path: "Demo::Utility".to_string(),
            }]
        );

        let file_scoped_source = r#"
namespace Demo.App;
using Demo.Utility;
class Caller {}
"#;
        let document = parse_document(Path::new("FileScoped.cs"), file_scoped_source).unwrap();
        assert_eq!(
            csharp_file_namespace_imports(document.tree.root_node(), file_scoped_source).unwrap(),
            vec![super::CSharpFileNamespaceImport {
                scope_path: Some("Demo::App".to_string()),
                semantic_namespace_path: "Demo::Utility".to_string(),
            }]
        );
    }

    #[test]
    fn collects_namespace_scoped_type_alias_imports() {
        let block_scoped_source = r#"
namespace Demo.App {
    using HelperAlias = Demo.Utility.Helper;
    class Caller {}
}
"#;
        let document = parse_document(Path::new("BlockScoped.cs"), block_scoped_source).unwrap();
        assert_eq!(
            csharp_file_type_alias_imports(document.tree.root_node(), block_scoped_source).unwrap(),
            vec![super::CSharpFileTypeAliasImport {
                scope_path: Some("Demo::App".to_string()),
                local_name: "HelperAlias".to_string(),
                semantic_type_path: "Demo::Utility::Helper".to_string(),
                raw_generic_argument_spellings: Vec::new(),
                raw_enclosing_generic_argument_spellings: vec![vec![], vec![]],
            }]
        );

        let file_scoped_source = r#"
namespace Demo.App;
using HelperAlias = Demo.Utility.Helper;
class Caller {}
"#;
        let document = parse_document(Path::new("FileScoped.cs"), file_scoped_source).unwrap();
        assert_eq!(
            csharp_file_type_alias_imports(document.tree.root_node(), file_scoped_source).unwrap(),
            vec![super::CSharpFileTypeAliasImport {
                scope_path: Some("Demo::App".to_string()),
                local_name: "HelperAlias".to_string(),
                semantic_type_path: "Demo::Utility::Helper".to_string(),
                raw_generic_argument_spellings: Vec::new(),
                raw_enclosing_generic_argument_spellings: vec![vec![], vec![]],
            }]
        );
    }
}
