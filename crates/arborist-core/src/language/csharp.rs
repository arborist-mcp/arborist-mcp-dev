use anyhow::Result;
use tree_sitter::Node;

use super::node_text;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CSharpFileTypeAliasImport {
    pub(crate) scope_path: Option<String>,
    pub(crate) local_name: String,
    pub(crate) semantic_type_path: String,
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
    let Some(semantic_type_path) = csharp_qualified_type_semantic_path(target_path) else {
        return Ok(None);
    };
    Ok(Some(CSharpFileTypeAliasImport {
        scope_path: scope_path.map(str::to_string),
        local_name: local_name.to_string(),
        semantic_type_path,
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
        let Some(semantic_type_path) = csharp_qualified_type_semantic_path(target_path) else {
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
        if let Some(semantic_type_path) = csharp_qualified_type_semantic_path(target_path) {
            imports.push(semantic_type_path);
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
        csharp_file_type_alias_imports, csharp_global_static_type_imports,
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
                    scope_path: None,
                    local_name: "HelperAlias".to_string(),
                    semantic_type_path: "Demo::Utility::Helper".to_string(),
                },
                super::CSharpFileTypeAliasImport {
                    scope_path: None,
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
                    scope_path: None,
                    semantic_type_path: "Demo::Utility::Helper".to_string(),
                },
                super::CSharpFileStaticTypeImport {
                    scope_path: None,
                    semantic_type_path: "Demo::Utility::GlobalHelper".to_string(),
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
            }]
        );
    }
}
