use anyhow::Result;
use tree_sitter::Node;

use super::node_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileTypeAliasImport {
    pub(crate) local_name: String,
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileStaticTypeImport {
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileNamespaceImport {
    pub(crate) semantic_namespace_path: String,
}

pub(crate) fn csharp_file_type_alias_imports(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpFileTypeAliasImport>> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for directive in root.named_children(&mut cursor) {
        if directive.kind() != "using_directive" {
            continue;
        }
        let directive_text = node_text(directive, source)?.trim();
        if !is_file_type_alias_directive(directive_text) {
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
        let Some(semantic_type_path) = csharp_qualified_type_semantic_path(target_path) else {
            continue;
        };
        imports.push(CSharpFileTypeAliasImport {
            local_name: local_name.to_string(),
            semantic_type_path,
        });
    }
    Ok(imports)
}

pub(crate) fn csharp_file_static_type_imports(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpFileStaticTypeImport>> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for directive in root.named_children(&mut cursor) {
        if directive.kind() != "using_directive" {
            continue;
        }
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
        let Some(semantic_type_path) = csharp_qualified_type_semantic_path(target_path) else {
            continue;
        };
        imports.push(CSharpFileStaticTypeImport { semantic_type_path });
    }
    Ok(imports)
}

pub(crate) fn csharp_file_namespace_imports(
    root: Node<'_>,
    source: &str,
) -> Result<Vec<CSharpFileNamespaceImport>> {
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for directive in root.named_children(&mut cursor) {
        if directive.kind() != "using_directive" {
            continue;
        }
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
        csharp_file_namespace_imports, csharp_file_static_type_imports,
        csharp_file_type_alias_imports,
    };
    use crate::language::parse_document;

    #[test]
    fn collects_only_safe_file_type_alias_imports() {
        let source = r#"
using HelperAlias = Demo.Utility.Helper;
using GlobalAlias = global::Demo.Utility.GlobalHelper;
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
                    local_name: "HelperAlias".to_string(),
                    semantic_type_path: "Demo::Utility::Helper".to_string(),
                },
                super::CSharpFileTypeAliasImport {
                    local_name: "GlobalAlias".to_string(),
                    semantic_type_path: "Demo::Utility::GlobalHelper".to_string(),
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
                    semantic_type_path: "Demo::Utility::Helper".to_string(),
                },
                super::CSharpFileStaticTypeImport {
                    semantic_type_path: "Demo::Utility::GlobalHelper".to_string(),
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
                    semantic_namespace_path: "Demo::Utility".to_string(),
                },
                super::CSharpFileNamespaceImport {
                    semantic_namespace_path: "Demo::Shared".to_string(),
                },
            ]
        );
    }
}
