use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path, parse_document};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JavaLocalTypeImport {
    pub(crate) local_name: String,
    pub(crate) semantic_path: String,
    pub(crate) source_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JavaLocalStaticMemberImport {
    pub(crate) local_name: String,
    pub(crate) semantic_type_path: String,
    pub(crate) source_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum JavaDirectSuperclassReference {
    Simple(String),
    Qualified(String),
}

pub(crate) fn java_local_file_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let mut dependencies = java_local_explicit_type_imports(path, root, source)?
        .into_iter()
        .map(|import| import.source_path)
        .collect::<BTreeSet<_>>();
    dependencies.extend(
        java_local_explicit_static_member_imports(path, root, source)?
            .into_iter()
            .map(|import| import.source_path),
    );
    dependencies.extend(java_local_simple_superclass_dependency_paths(
        path, root, source,
    )?);
    dependencies.extend(java_local_direct_interface_dependency_paths(
        path, root, source,
    )?);
    dependencies.extend(java_local_wildcard_import_dependency_paths(
        path, root, source,
    )?);
    Ok(dependencies)
}

fn java_local_simple_superclass_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let normalized_path = normalize_absolute_path(path)?;
    let package_name = java_package_name(root, source)?;
    let mut dependencies = BTreeSet::new();
    let mut cursor = root.walk();
    for declaration in root
        .named_children(&mut cursor)
        .filter(|node| node.kind() == "class_declaration")
    {
        let Some(superclass) = declaration.child_by_field_name("superclass") else {
            continue;
        };
        let Some(superclass_reference) = java_direct_superclass_reference(superclass, source)?
        else {
            continue;
        };
        let source_path = java_direct_type_reference_source_path(
            path,
            package_name.as_deref(),
            &superclass_reference,
        );
        if let Some(source_path) = source_path
            && source_path != normalized_path
        {
            dependencies.insert(source_path);
        }
    }
    Ok(dependencies)
}

fn java_local_direct_interface_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let normalized_path = normalize_absolute_path(path)?;
    let package_name = java_package_name(root, source)?;
    let mut dependencies = BTreeSet::new();
    let mut cursor = root.walk();
    for declaration in root
        .named_children(&mut cursor)
        .filter(|node| matches!(node.kind(), "class_declaration" | "interface_declaration"))
    {
        let Some(interface_references) =
            java_direct_interface_references_for_declaration(declaration, source)?
        else {
            continue;
        };
        for interface_reference in interface_references {
            let source_path = java_direct_type_reference_source_path(
                path,
                package_name.as_deref(),
                &interface_reference,
            );
            if let Some(source_path) = source_path
                && source_path != normalized_path
            {
                dependencies.insert(source_path);
            }
        }
    }
    Ok(dependencies)
}

fn java_direct_type_reference_source_path(
    path: &Path,
    package_name: Option<&str>,
    reference: &JavaDirectSuperclassReference,
) -> Option<PathBuf> {
    match reference {
        JavaDirectSuperclassReference::Simple(type_name) => {
            if let Some(package_name) = package_name {
                let import_path = format!("{package_name}.{type_name}");
                resolve_unique_java_source_path(path, &import_path)
            } else {
                resolve_unique_java_default_package_source_path(path, type_name)
            }
        }
        JavaDirectSuperclassReference::Qualified(qualified_name) => {
            resolve_unique_java_qualified_superclass_source_path(path, package_name, qualified_name)
        }
    }
}

pub(crate) fn java_direct_superclass_reference(
    superclass: Node<'_>,
    source: &str,
) -> Result<Option<JavaDirectSuperclassReference>> {
    let Some(type_node) = superclass.named_child(0) else {
        return Ok(None);
    };
    java_direct_type_reference(type_node, source)
}

pub(crate) fn java_direct_interface_references_for_declaration(
    declaration: Node<'_>,
    source: &str,
) -> Result<Option<Vec<JavaDirectSuperclassReference>>> {
    let interfaces = match declaration.kind() {
        "class_declaration" => declaration.child_by_field_name("interfaces"),
        "interface_declaration" => {
            let mut cursor = declaration.walk();
            declaration
                .named_children(&mut cursor)
                .find(|child| child.kind() == "extends_interfaces")
        }
        _ => None,
    };
    let Some(interfaces) = interfaces else {
        return Ok(None);
    };
    java_direct_interface_references(interfaces, source)
}

pub(crate) fn java_direct_interface_references(
    interfaces: Node<'_>,
    source: &str,
) -> Result<Option<Vec<JavaDirectSuperclassReference>>> {
    let mut cursor = interfaces.walk();
    let Some(type_list) = interfaces
        .named_children(&mut cursor)
        .find(|child| child.kind() == "type_list")
    else {
        return Ok(None);
    };
    let mut cursor = type_list.walk();
    let mut references = Vec::new();
    for type_node in type_list.named_children(&mut cursor) {
        let Some(reference) = java_direct_type_reference(type_node, source)? else {
            return Ok(None);
        };
        references.push(reference);
    }
    Ok((!references.is_empty()).then_some(references))
}

fn java_direct_type_reference(
    type_node: Node<'_>,
    source: &str,
) -> Result<Option<JavaDirectSuperclassReference>> {
    match type_node.kind() {
        "type_identifier" => java_simple_superclass_reference(type_node, source),
        "scoped_type_identifier" => java_qualified_superclass_reference(type_node, source),
        "generic_type" => {
            let mut cursor = type_node.walk();
            let children = type_node.named_children(&mut cursor).collect::<Vec<_>>();
            let [base_type, type_arguments] = children.as_slice() else {
                return Ok(None);
            };
            if type_arguments.kind() != "type_arguments" {
                return Ok(None);
            }
            match base_type.kind() {
                "type_identifier" => java_simple_superclass_reference(*base_type, source),
                "scoped_type_identifier" => java_qualified_superclass_reference(*base_type, source),
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

fn java_simple_superclass_reference(
    type_node: Node<'_>,
    source: &str,
) -> Result<Option<JavaDirectSuperclassReference>> {
    let reference = node_text(type_node, source)?.trim().to_string();
    Ok((!reference.is_empty()).then_some(JavaDirectSuperclassReference::Simple(reference)))
}

fn java_qualified_superclass_reference(
    type_node: Node<'_>,
    source: &str,
) -> Result<Option<JavaDirectSuperclassReference>> {
    let reference = node_text(type_node, source)?.trim().to_string();
    Ok(
        (!reference.is_empty() && is_safe_java_qualified_name(&reference))
            .then_some(JavaDirectSuperclassReference::Qualified(reference)),
    )
}

pub(crate) fn java_local_explicit_type_imports(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<Vec<JavaLocalTypeImport>> {
    let normalized_path = normalize_absolute_path(path)?;
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "import_declaration" {
            continue;
        }
        let Some(import_path) = java_explicit_type_import_path(node, source)? else {
            continue;
        };
        let Some(source_path) = resolve_unique_java_source_path(path, &import_path) else {
            continue;
        };
        if source_path == normalized_path {
            continue;
        }
        let Some(local_name) = import_path
            .rsplit('.')
            .next()
            .filter(|name| !name.is_empty())
        else {
            continue;
        };
        imports.push(JavaLocalTypeImport {
            local_name: local_name.to_string(),
            semantic_path: import_path.replace('.', "::"),
            source_path,
        });
    }
    Ok(imports)
}

pub(crate) fn java_local_explicit_static_member_imports(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<Vec<JavaLocalStaticMemberImport>> {
    let normalized_path = normalize_absolute_path(path)?;
    let mut imports = Vec::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "import_declaration" {
            continue;
        }
        let Some(import_path) = java_explicit_static_member_import_path(node, source)? else {
            continue;
        };
        let Some((type_path, local_name)) = import_path.rsplit_once('.') else {
            continue;
        };
        if type_path.is_empty() || local_name.is_empty() {
            continue;
        }
        let Some(source_path) = resolve_unique_java_source_path(path, type_path) else {
            continue;
        };
        if source_path == normalized_path {
            continue;
        }
        imports.push(JavaLocalStaticMemberImport {
            local_name: local_name.to_string(),
            semantic_type_path: type_path.replace('.', "::"),
            source_path,
        });
    }
    Ok(imports)
}

fn java_local_wildcard_import_dependency_paths(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Result<BTreeSet<PathBuf>> {
    let normalized_path = normalize_absolute_path(path)?;
    let mut dependencies = BTreeSet::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        if node.kind() != "import_declaration" {
            continue;
        }
        let Some((is_static, import_path)) = java_wildcard_import_path(node, source)? else {
            continue;
        };
        if is_static {
            if let Some(source_path) = resolve_unique_java_source_path(path, &import_path)
                && source_path != normalized_path
            {
                dependencies.insert(source_path);
            }
            continue;
        }
        dependencies.extend(
            resolve_unique_java_package_source_paths(path, &import_path)
                .into_iter()
                .filter(|source_path| source_path != &normalized_path),
        );
    }
    Ok(dependencies)
}

fn java_wildcard_import_path(node: Node<'_>, source: &str) -> Result<Option<(bool, String)>> {
    let text = node_text(node, source)?.trim();
    let is_static = text.split_whitespace().nth(1) == Some("static");
    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    if !children.iter().any(|child| child.kind() == "asterisk") {
        return Ok(None);
    }
    let Some(name) = children
        .into_iter()
        .find(|child| matches!(child.kind(), "identifier" | "scoped_identifier"))
    else {
        return Ok(None);
    };
    let import_path = node_text(name, source)?.trim().to_string();
    if import_path.is_empty() || !is_safe_java_qualified_name(&import_path) {
        return Ok(None);
    }
    Ok(Some((is_static, import_path)))
}

fn java_explicit_type_import_path(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some((is_static, import_path)) = java_explicit_import_path(node, source)? else {
        return Ok(None);
    };
    Ok((!is_static).then_some(import_path))
}

fn java_explicit_static_member_import_path(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let Some((is_static, import_path)) = java_explicit_import_path(node, source)? else {
        return Ok(None);
    };
    Ok(is_static.then_some(import_path))
}

fn java_explicit_import_path(node: Node<'_>, source: &str) -> Result<Option<(bool, String)>> {
    let text = node_text(node, source)?.trim();
    let is_static = text.split_whitespace().nth(1) == Some("static");

    let mut cursor = node.walk();
    let children = node.named_children(&mut cursor).collect::<Vec<_>>();
    if children.iter().any(|child| child.kind() == "asterisk") {
        return Ok(None);
    }
    let Some(name) = children
        .into_iter()
        .find(|child| matches!(child.kind(), "identifier" | "scoped_identifier"))
    else {
        return Ok(None);
    };
    let import_path = node_text(name, source)?.trim().to_string();
    if import_path.is_empty() || !is_safe_java_qualified_name(&import_path) {
        return Ok(None);
    }
    Ok(Some((is_static, import_path)))
}

fn java_package_name(root: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut cursor = root.walk();
    let Some(package) = root
        .named_children(&mut cursor)
        .find(|node| node.kind() == "package_declaration")
    else {
        return Ok(None);
    };
    let mut cursor = package.walk();
    package
        .named_children(&mut cursor)
        .find(|node| matches!(node.kind(), "identifier" | "scoped_identifier"))
        .map(|name| node_text(name, source).map(str::trim).map(str::to_string))
        .transpose()
        .map(|name| name.filter(|name| !name.is_empty()))
}

fn resolve_unique_java_qualified_superclass_source_path(
    path: &Path,
    package_name: Option<&str>,
    qualified_name: &str,
) -> Option<PathBuf> {
    let mut source_type_paths = BTreeSet::from([qualified_name.to_string()]);
    if let Some(package_name) = package_name
        && !package_name.is_empty()
    {
        source_type_paths.insert(format!("{package_name}.{qualified_name}"));
    }

    let mut candidates = BTreeSet::new();
    for source_type_path in source_type_paths {
        let segments = source_type_path.split('.').collect::<Vec<_>>();
        for end in 1..=segments.len() {
            let candidate_path = segments[..end].join(".");
            if let Some(candidate) = resolve_unique_java_source_path(path, &candidate_path) {
                candidates.insert(candidate);
            }
        }
    }
    (candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten()
}

fn resolve_unique_java_package_source_paths(path: &Path, package_name: &str) -> BTreeSet<PathBuf> {
    let segments = package_name.split('.').collect::<Vec<_>>();
    let mut package_directories = BTreeSet::new();
    let mut source_root = match path.parent() {
        Some(parent) => parent.to_path_buf(),
        None => return BTreeSet::new(),
    };

    loop {
        let mut candidate = source_root.clone();
        for segment in &segments {
            candidate.push(segment);
        }
        if candidate.is_dir()
            && java_source_files_in_package_directory(&candidate, package_name)
                .next()
                .is_some()
            && let Ok(candidate) = normalize_absolute_path(&candidate)
        {
            package_directories.insert(candidate);
        }
        if !source_root.pop() {
            break;
        }
    }

    if package_directories.len() != 1 {
        return BTreeSet::new();
    }
    let directory = package_directories.into_iter().next().unwrap();
    java_source_files_in_package_directory(&directory, package_name)
        .filter_map(|candidate| normalize_absolute_path(&candidate).ok())
        .collect()
}

fn java_source_files_in_package_directory<'a>(
    directory: &'a Path,
    package_name: &'a str,
) -> impl Iterator<Item = PathBuf> + 'a {
    fs::read_dir(directory)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|candidate| {
            candidate.is_file()
                && candidate
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("java"))
        })
        .filter(move |candidate| candidate_declares_package(candidate, package_name))
}

fn resolve_unique_java_default_package_source_path(
    path: &Path,
    type_name: &str,
) -> Option<PathBuf> {
    let mut candidates = BTreeSet::new();
    let mut source_root = path.parent()?.to_path_buf();
    loop {
        let candidate = source_root.join(format!("{type_name}.java"));
        if candidate.is_file() && candidate_declares_default_package(&candidate) {
            candidates.insert(normalize_absolute_path(&candidate).ok()?);
        }
        if !source_root.pop() {
            break;
        }
    }
    (candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten()
}

fn candidate_declares_default_package(candidate: &Path) -> bool {
    let Ok(source) = fs::read_to_string(candidate) else {
        return false;
    };
    let Ok(document) = parse_document(candidate, &source) else {
        return false;
    };
    let mut cursor = document.tree.root_node().walk();
    !document
        .tree
        .root_node()
        .named_children(&mut cursor)
        .any(|node| node.kind() == "package_declaration")
}

fn resolve_unique_java_source_path(path: &Path, import_path: &str) -> Option<PathBuf> {
    let segments = import_path.split('.').collect::<Vec<_>>();
    let mut candidates = BTreeSet::new();
    let mut source_root = path.parent()?.to_path_buf();
    loop {
        // Nested type and static-member imports such as `pkg.outer.Outer.Inner`
        // or `pkg.outer.Outer.Inner.method` name a member declared inside the
        // outermost remaining prefix's source file, so progressively strip
        // trailing segments until a prefix maps to a `.java` file.
        let mut prefix_len = segments.len();
        while prefix_len > 0 {
            let mut candidate = source_root.clone();
            for segment in &segments[..prefix_len] {
                candidate.push(segment);
            }
            candidate.set_extension("java");
            let prefix_path = segments[..prefix_len].join(".");
            if candidate.is_file() && candidate_declares_import_package(&candidate, &prefix_path) {
                candidates.insert(normalize_absolute_path(&candidate).ok()?);
                break;
            }
            prefix_len -= 1;
        }

        if !source_root.pop() {
            break;
        }
    }
    (candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten()
}

fn candidate_declares_import_package(candidate: &Path, import_path: &str) -> bool {
    let Some((expected_package, _)) = import_path.rsplit_once('.') else {
        return false;
    };
    candidate_declares_package(candidate, expected_package)
}

fn candidate_declares_package(candidate: &Path, expected_package: &str) -> bool {
    let Ok(source) = fs::read_to_string(candidate) else {
        return false;
    };
    let Ok(document) = parse_document(candidate, &source) else {
        return false;
    };
    let mut cursor = document.tree.root_node().walk();
    let Some(package) = document
        .tree
        .root_node()
        .named_children(&mut cursor)
        .find(|node| node.kind() == "package_declaration")
    else {
        return false;
    };
    let mut cursor = package.walk();
    let Some(name) = package
        .named_children(&mut cursor)
        .find(|node| matches!(node.kind(), "identifier" | "scoped_identifier"))
    else {
        return false;
    };
    node_text(name, &source)
        .map(|name| name.trim() == expected_package)
        .unwrap_or(false)
}

fn is_safe_java_qualified_name(name: &str) -> bool {
    name.split('.').all(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(['/', '\\'])
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        java_local_explicit_static_member_imports, java_local_explicit_type_imports,
        java_local_file_dependency_paths,
    };
    use crate::language::parse_document;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn resolves_unique_explicit_java_imports_from_ancestor_source_roots() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.java");
        let helper_path = root.join("src/com/example/Helper.java");
        let widget_path = root.join("src/com/example/types/Widget.java");
        let static_helper_path = root.join("src/com/example/StaticHelper.java");
        let mismatched_helper_path = root.join("src/com/example/com/example/Helper.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(widget_path.parent().unwrap()).unwrap();
        fs::create_dir_all(mismatched_helper_path.parent().unwrap()).unwrap();
        fs::write(&helper_path, "package com.example; class Helper {}\n").unwrap();
        fs::write(&widget_path, "package com.example.types; class Widget {}\n").unwrap();
        fs::write(
            &static_helper_path,
            "package com.example; class StaticHelper {}\n",
        )
        .unwrap();
        fs::write(
            &mismatched_helper_path,
            "package unrelated; class Helper {}\n",
        )
        .unwrap();
        let source = "package com.example;\nimport com.example.Helper;\nimport com.example.types.Widget;\nimport static com.example.StaticHelper.utility;\nimport static com.example.StaticHelper.*;\nimport com.example.Missing;\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();

        assert_eq!(
            dependencies,
            [
                helper_path.clone(),
                static_helper_path.clone(),
                widget_path.clone(),
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(
            java_local_explicit_type_imports(&source_path, document.tree.root_node(), source)
                .unwrap(),
            vec![
                super::JavaLocalTypeImport {
                    local_name: "Helper".to_string(),
                    semantic_path: "com::example::Helper".to_string(),
                    source_path: helper_path,
                },
                super::JavaLocalTypeImport {
                    local_name: "Widget".to_string(),
                    semantic_path: "com::example::types::Widget".to_string(),
                    source_path: widget_path,
                },
            ]
        );
        assert_eq!(
            java_local_explicit_static_member_imports(
                &source_path,
                document.tree.root_node(),
                source,
            )
            .unwrap(),
            vec![super::JavaLocalStaticMemberImport {
                local_name: "utility".to_string(),
                semantic_type_path: "com::example::StaticHelper".to_string(),
                source_path: static_helper_path,
            }]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_unique_nested_type_and_static_member_imports_from_ancestor_source_roots() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.java");
        let outer_path = root.join("src/com/example/Outer.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(
            &outer_path,
            "package com.example; class Outer { static class Inner { static int utility(int value) { return value; } } }
",
        )
        .unwrap();
        let source = "package com.example;
import com.example.Outer.Inner;
import static com.example.Outer.Inner.utility;
";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();

        assert_eq!(dependencies, [outer_path.clone()].into_iter().collect());
        assert_eq!(
            java_local_explicit_type_imports(&source_path, document.tree.root_node(), source)
                .unwrap(),
            vec![super::JavaLocalTypeImport {
                local_name: "Inner".to_string(),
                semantic_path: "com::example::Outer::Inner".to_string(),
                source_path: outer_path.clone(),
            }]
        );
        assert_eq!(
            java_local_explicit_static_member_imports(
                &source_path,
                document.tree.root_node(),
                source,
            )
            .unwrap(),
            vec![super::JavaLocalStaticMemberImport {
                local_name: "utility".to_string(),
                semantic_type_path: "com::example::Outer::Inner".to_string(),
                source_path: outer_path,
            }]
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_unique_same_package_simple_superclasses_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Child.java");
        let base_path = root.join("src/com/example/Base.java");
        let unrelated_path = root.join("src/com/example/Unrelated.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.example; class Child extends Base {} class Generic extends Holder<String> {} class Qualified extends other.Base {}
",
        )
        .unwrap();
        fs::write(
            &base_path,
            "package com.example; class Base {}
",
        )
        .unwrap();
        fs::write(
            &unrelated_path,
            "package com.example; class Unrelated {}
",
        )
        .unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [base_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_unique_same_package_outer_superclasses_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Child.java");
        let outer_path = root.join("src/com/example/Outer.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.example; class Child extends Outer.Base {}
",
        )
        .unwrap();
        fs::write(
            &outer_path,
            "package com.example; class Outer { static class Base {} }
",
        )
        .unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [outer_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_unique_simple_generic_superclasses_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Child.java");
        let base_path = root.join("src/com/example/Base.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.example; class Child extends Base<String> {} class Qualified extends other.Base<String> {}
",
        )
        .unwrap();
        fs::write(
            &base_path,
            "package com.example; class Base<T> {}
",
        )
        .unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [base_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_unique_qualified_superclasses_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/child/Child.java");
        let base_path = root.join("src/com/base/Base.java");
        let holder_path = root.join("src/com/base/Holder.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(base_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.child; class Child extends com.base.Base {} class Generic extends com.base.Holder<String> {}
",
        )
        .unwrap();
        fs::write(
            &base_path,
            "package com.base; class Base {}
",
        )
        .unwrap();
        fs::write(
            &holder_path,
            "package com.base; class Holder<T> {}
",
        )
        .unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [base_path, holder_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_unique_java_wildcard_package_imports_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/app/Main.java");
        let helper_path = root.join("src/com/example/Helper.java");
        let other_path = root.join("src/com/example/Other.java");
        let unrelated_path = root.join("src/com/other/Unrelated.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(helper_path.parent().unwrap()).unwrap();
        fs::create_dir_all(unrelated_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.app;\nimport com.example.*;\nclass Main {}\n",
        )
        .unwrap();
        fs::write(&helper_path, "package com.example; class Helper {}\n").unwrap();
        fs::write(&other_path, "package com.example; class Other {}\n").unwrap();
        fs::write(&unrelated_path, "package com.other; class Unrelated {}\n").unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(
            dependencies,
            [helper_path, other_path].into_iter().collect()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_unique_java_static_wildcard_imports_as_dependencies() {
        let root = temporary_dir();
        let source_path = root.join("src/com/app/Main.java");
        let helper_path = root.join("src/com/example/Helper.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(helper_path.parent().unwrap()).unwrap();
        fs::write(
            &source_path,
            "package com.app;\nimport static com.example.Helper.*;\nclass Main {}\n",
        )
        .unwrap();
        fs::write(
            &helper_path,
            "package com.example; class Helper { static int value() { return 1; } }\n",
        )
        .unwrap();
        let source = fs::read_to_string(&source_path).unwrap();
        let document = parse_document(&source_path, &source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), &source)
                .unwrap();

        assert_eq!(dependencies, [helper_path].into_iter().collect());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_ambiguous_java_wildcard_package_imports() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.java");
        let first_helper = root.join("src/com/example/Helper.java");
        let second_helper = root.join("src/com/example/com/example/Helper.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::create_dir_all(second_helper.parent().unwrap()).unwrap();
        fs::write(&first_helper, "package com.example; class Helper {}\n").unwrap();
        fs::write(&second_helper, "package com.example; class Helper {}\n").unwrap();
        let source = "package com.example;\nimport com.example.*;\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).unwrap();

        let dependencies =
            java_local_file_dependency_paths(&source_path, document.tree.root_node(), source)
                .unwrap();

        assert!(dependencies.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    fn temporary_dir() -> PathBuf {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let directory = std::env::temp_dir().join(format!("arborist-java-language-{suffix}"));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }
}
