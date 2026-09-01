use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tree_sitter::Node;

use super::{node_text, normalize_absolute_path, parse_document, parse_document_with_timeout};
use crate::deadline::DeadlineCheck;

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
        path, root, source, None,
    )?);
    Ok(dependencies)
}

pub(crate) fn java_local_file_dependency_paths_with_deadline(
    path: &Path,
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let normalized_path = normalize_absolute_path(path)?;
    let package_name = java_package_name_with_deadline(root, source, deadline)?;
    let mut dependencies = BTreeSet::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        check_local_file_dependency_deadline(deadline)?;
        match node.kind() {
            "import_declaration" => {
                if let Some(import_path) = java_explicit_type_import_path(node, source)? {
                    if let Some(source_path) =
                        resolve_unique_java_source_path_with_deadline(path, &import_path, deadline)?
                        && source_path != normalized_path
                    {
                        dependencies.insert(source_path);
                    }
                    continue;
                }
                if let Some(import_path) = java_explicit_static_member_import_path(node, source)? {
                    let Some((type_path, _)) = import_path.rsplit_once('.') else {
                        continue;
                    };
                    if let Some(source_path) =
                        resolve_unique_java_source_path_with_deadline(path, type_path, deadline)?
                        && source_path != normalized_path
                    {
                        dependencies.insert(source_path);
                    }
                    continue;
                }
                let Some((is_static, import_path)) = java_wildcard_import_path(node, source)?
                else {
                    continue;
                };
                if is_static {
                    if let Some(source_path) =
                        resolve_unique_java_source_path_with_deadline(path, &import_path, deadline)?
                        && source_path != normalized_path
                    {
                        dependencies.insert(source_path);
                    }
                } else {
                    dependencies.extend(
                        resolve_unique_java_package_source_paths_with_deadline(
                            path,
                            &import_path,
                            deadline,
                        )?
                        .into_iter()
                        .filter(|source_path| source_path != &normalized_path),
                    );
                }
            }
            "class_declaration" => {
                if let Some(superclass) = node.child_by_field_name("superclass")
                    && let Some(reference) = java_direct_superclass_reference(superclass, source)?
                    && let Some(source_path) = java_direct_type_reference_source_path_with_deadline(
                        path,
                        package_name.as_deref(),
                        &reference,
                        deadline,
                    )?
                    && source_path != normalized_path
                {
                    dependencies.insert(source_path);
                }
                if let Some(references) =
                    java_direct_interface_references_for_declaration_with_deadline(
                        node, source, deadline,
                    )?
                {
                    insert_java_direct_interface_dependencies(
                        path,
                        package_name.as_deref(),
                        &normalized_path,
                        references,
                        &mut dependencies,
                        deadline,
                    )?;
                }
            }
            "interface_declaration" => {
                if let Some(references) =
                    java_direct_interface_references_for_declaration_with_deadline(
                        node, source, deadline,
                    )?
                {
                    insert_java_direct_interface_dependencies(
                        path,
                        package_name.as_deref(),
                        &normalized_path,
                        references,
                        &mut dependencies,
                        deadline,
                    )?;
                }
            }
            _ => {}
        }
    }
    Ok(dependencies)
}

fn insert_java_direct_interface_dependencies(
    path: &Path,
    package_name: Option<&str>,
    normalized_path: &Path,
    references: Vec<JavaDirectSuperclassReference>,
    dependencies: &mut BTreeSet<PathBuf>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    for reference in references {
        check_local_file_dependency_deadline(deadline)?;
        if let Some(source_path) = java_direct_type_reference_source_path_with_deadline(
            path,
            package_name,
            &reference,
            deadline,
        )? && source_path != normalized_path
        {
            dependencies.insert(source_path);
        }
    }
    Ok(())
}

fn check_local_file_dependency_deadline(deadline: Option<&dyn DeadlineCheck>) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("extracting local file dependencies")?;
    }
    Ok(())
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
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    let normalized_path = normalize_absolute_path(path)?;
    let mut dependencies = BTreeSet::new();
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        check_local_file_dependency_deadline(deadline)?;
        if node.kind() != "import_declaration" {
            continue;
        }
        let Some((is_static, import_path)) = java_wildcard_import_path(node, source)? else {
            continue;
        };
        if is_static {
            if let Some(source_path) =
                resolve_unique_java_source_path_with_deadline(path, &import_path, deadline)?
                && source_path != normalized_path
            {
                dependencies.insert(source_path);
            }
            continue;
        }
        dependencies.extend(
            resolve_unique_java_package_source_paths_with_deadline(path, &import_path, deadline)?
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

fn java_package_name_with_deadline(
    root: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<String>> {
    let mut cursor = root.walk();
    for node in root.named_children(&mut cursor) {
        check_local_file_dependency_deadline(deadline)?;
        if node.kind() != "package_declaration" {
            continue;
        }
        let mut package_cursor = node.walk();
        for child in node.named_children(&mut package_cursor) {
            check_local_file_dependency_deadline(deadline)?;
            if matches!(child.kind(), "identifier" | "scoped_identifier") {
                let name = node_text(child, source)?.trim().to_string();
                return Ok((!name.is_empty()).then_some(name));
            }
        }
        return Ok(None);
    }
    Ok(None)
}

fn java_direct_interface_references_for_declaration_with_deadline(
    declaration: Node<'_>,
    source: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<Vec<JavaDirectSuperclassReference>>> {
    let interfaces = match declaration.kind() {
        "class_declaration" => declaration.child_by_field_name("interfaces"),
        "interface_declaration" => {
            let mut cursor = declaration.walk();
            let mut interfaces = None;
            for child in declaration.named_children(&mut cursor) {
                check_local_file_dependency_deadline(deadline)?;
                if child.kind() == "extends_interfaces" {
                    interfaces = Some(child);
                    break;
                }
            }
            interfaces
        }
        _ => None,
    };
    let Some(interfaces) = interfaces else {
        return Ok(None);
    };
    let mut cursor = interfaces.walk();
    let mut type_list = None;
    for child in interfaces.named_children(&mut cursor) {
        check_local_file_dependency_deadline(deadline)?;
        if child.kind() == "type_list" {
            type_list = Some(child);
            break;
        }
    }
    let Some(type_list) = type_list else {
        return Ok(None);
    };
    let mut cursor = type_list.walk();
    let mut references = Vec::new();
    for type_node in type_list.named_children(&mut cursor) {
        check_local_file_dependency_deadline(deadline)?;
        let Some(reference) = java_direct_type_reference(type_node, source)? else {
            return Ok(None);
        };
        references.push(reference);
    }
    Ok((!references.is_empty()).then_some(references))
}

fn java_direct_type_reference_source_path_with_deadline(
    path: &Path,
    package_name: Option<&str>,
    reference: &JavaDirectSuperclassReference,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    match reference {
        JavaDirectSuperclassReference::Simple(type_name) => {
            if let Some(package_name) = package_name {
                let import_path = format!("{package_name}.{type_name}");
                resolve_unique_java_source_path_with_deadline(path, &import_path, deadline)
            } else {
                resolve_unique_java_default_package_source_path_with_deadline(
                    path, type_name, deadline,
                )
            }
        }
        JavaDirectSuperclassReference::Qualified(qualified_name) => {
            resolve_unique_java_qualified_superclass_source_path_with_deadline(
                path,
                package_name,
                qualified_name,
                deadline,
            )
        }
    }
}

fn resolve_unique_java_qualified_superclass_source_path_with_deadline(
    path: &Path,
    package_name: Option<&str>,
    qualified_name: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let mut source_type_paths = BTreeSet::from([qualified_name.to_string()]);
    if let Some(package_name) = package_name
        && !package_name.is_empty()
    {
        source_type_paths.insert(format!("{package_name}.{qualified_name}"));
    }

    let mut candidates = BTreeSet::new();
    for source_type_path in source_type_paths {
        check_local_file_dependency_deadline(deadline)?;
        let segments = source_type_path.split('.').collect::<Vec<_>>();
        for end in 1..=segments.len() {
            check_local_file_dependency_deadline(deadline)?;
            let candidate_path = segments[..end].join(".");
            if let Some(candidate) =
                resolve_unique_java_source_path_with_deadline(path, &candidate_path, deadline)?
            {
                candidates.insert(candidate);
            }
        }
    }
    Ok((candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten())
}

fn resolve_unique_java_package_source_paths_with_deadline(
    path: &Path,
    package_name: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<BTreeSet<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let segments = package_name.split('.').collect::<Vec<_>>();
    let mut package_directories = BTreeSet::new();
    let Some(mut source_root) = path.parent().map(Path::to_path_buf) else {
        return Ok(BTreeSet::new());
    };

    loop {
        check_local_file_dependency_deadline(deadline)?;
        let mut candidate = source_root.clone();
        for segment in &segments {
            candidate.push(segment);
        }
        if candidate.is_dir()
            && java_package_directory_contains_source_file_with_deadline(
                &candidate,
                package_name,
                deadline,
            )?
            && let Ok(candidate) = normalize_absolute_path(&candidate)
        {
            package_directories.insert(candidate);
        }
        if !source_root.pop() {
            break;
        }
    }

    if package_directories.len() != 1 {
        return Ok(BTreeSet::new());
    }
    let directory = package_directories.into_iter().next().unwrap();
    Ok(
        java_source_files_in_package_directory_with_deadline(&directory, package_name, deadline)?
            .into_iter()
            .filter_map(|candidate| normalize_absolute_path(&candidate).ok())
            .collect(),
    )
}

fn java_package_directory_contains_source_file_with_deadline(
    directory: &Path,
    package_name: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<bool> {
    check_local_file_dependency_deadline(deadline)?;
    for entry in fs::read_dir(directory).ok().into_iter().flatten().flatten() {
        check_local_file_dependency_deadline(deadline)?;
        let candidate = entry.path();
        if candidate.is_file()
            && candidate
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("java"))
            && candidate_declares_package_with_deadline(&candidate, package_name, deadline)?
        {
            return Ok(true);
        }
    }
    check_local_file_dependency_deadline(deadline)?;
    Ok(false)
}

fn java_source_files_in_package_directory_with_deadline(
    directory: &Path,
    package_name: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let mut source_paths = Vec::new();
    for entry in fs::read_dir(directory).ok().into_iter().flatten().flatten() {
        check_local_file_dependency_deadline(deadline)?;
        let candidate = entry.path();
        if candidate.is_file()
            && candidate
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("java"))
            && candidate_declares_package_with_deadline(&candidate, package_name, deadline)?
        {
            source_paths.push(candidate);
        }
    }
    check_local_file_dependency_deadline(deadline)?;
    Ok(source_paths)
}

fn resolve_unique_java_default_package_source_path_with_deadline(
    path: &Path,
    type_name: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let mut candidates = BTreeSet::new();
    let Some(mut source_root) = path.parent().map(Path::to_path_buf) else {
        return Ok(None);
    };
    loop {
        check_local_file_dependency_deadline(deadline)?;
        let candidate = source_root.join(format!("{type_name}.java"));
        if candidate.is_file()
            && candidate_declares_default_package_with_deadline(&candidate, deadline)?
            && let Ok(candidate) = normalize_absolute_path(&candidate)
        {
            candidates.insert(candidate);
        }
        if !source_root.pop() {
            break;
        }
    }
    Ok((candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten())
}

fn resolve_unique_java_source_path_with_deadline(
    path: &Path,
    import_path: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<PathBuf>> {
    check_local_file_dependency_deadline(deadline)?;
    let segments = import_path.split('.').collect::<Vec<_>>();
    let mut candidates = BTreeSet::new();
    let Some(mut source_root) = path.parent().map(Path::to_path_buf) else {
        return Ok(None);
    };
    loop {
        check_local_file_dependency_deadline(deadline)?;
        let mut prefix_len = segments.len();
        while prefix_len > 0 {
            check_local_file_dependency_deadline(deadline)?;
            let mut candidate = source_root.clone();
            for segment in &segments[..prefix_len] {
                candidate.push(segment);
            }
            candidate.set_extension("java");
            let prefix_path = segments[..prefix_len].join(".");
            if candidate.is_file()
                && candidate_declares_import_package_with_deadline(
                    &candidate,
                    &prefix_path,
                    deadline,
                )?
                && let Ok(candidate) = normalize_absolute_path(&candidate)
            {
                candidates.insert(candidate);
                break;
            }
            prefix_len -= 1;
        }
        if !source_root.pop() {
            break;
        }
    }
    Ok((candidates.len() == 1)
        .then(|| candidates.into_iter().next())
        .flatten())
}

fn candidate_declares_default_package_with_deadline(
    candidate: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<bool> {
    let Some(document) = read_java_dependency_candidate_with_deadline(candidate, deadline)? else {
        return Ok(false);
    };
    let mut cursor = document.document.tree.root_node().walk();
    for node in document
        .document
        .tree
        .root_node()
        .named_children(&mut cursor)
    {
        check_local_file_dependency_deadline(deadline)?;
        if node.kind() == "package_declaration" {
            return Ok(false);
        }
    }
    Ok(true)
}

fn candidate_declares_import_package_with_deadline(
    candidate: &Path,
    import_path: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<bool> {
    let Some((expected_package, _)) = import_path.rsplit_once('.') else {
        return Ok(false);
    };
    candidate_declares_package_with_deadline(candidate, expected_package, deadline)
}

fn candidate_declares_package_with_deadline(
    candidate: &Path,
    expected_package: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<bool> {
    let Some(document) = read_java_dependency_candidate_with_deadline(candidate, deadline)? else {
        return Ok(false);
    };
    let mut cursor = document.document.tree.root_node().walk();
    let mut package = None;
    for node in document
        .document
        .tree
        .root_node()
        .named_children(&mut cursor)
    {
        check_local_file_dependency_deadline(deadline)?;
        if node.kind() == "package_declaration" {
            package = Some(node);
            break;
        }
    }
    let Some(package) = package else {
        return Ok(false);
    };
    let mut cursor = package.walk();
    for node in package.named_children(&mut cursor) {
        check_local_file_dependency_deadline(deadline)?;
        if matches!(node.kind(), "identifier" | "scoped_identifier") {
            return Ok(node_text(node, &document.source)
                .map(|name| name.trim() == expected_package)
                .unwrap_or(false));
        }
    }
    Ok(false)
}

struct JavaDependencyCandidate {
    source: String,
    document: super::ParsedDocument,
}

fn read_java_dependency_candidate_with_deadline(
    candidate: &Path,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<JavaDependencyCandidate>> {
    check_local_file_dependency_deadline(deadline)?;
    let Ok(source) = fs::read_to_string(candidate) else {
        return Ok(None);
    };
    check_local_file_dependency_deadline(deadline)?;
    let document = if let Some(deadline) = deadline {
        match deadline.remaining_timeout_micros("parsing Java local file dependency candidates")? {
            Some(timeout_micros) => {
                parse_document_with_timeout(candidate, &source, timeout_micros)?
            }
            None => match parse_document(candidate, &source) {
                Ok(document) => document,
                Err(_) => return Ok(None),
            },
        }
    } else {
        match parse_document(candidate, &source) {
            Ok(document) => document,
            Err(_) => return Ok(None),
        }
    };
    Ok(Some(JavaDependencyCandidate { source, document }))
}

fn is_safe_java_qualified_name(name: &str) -> bool {
    name.split('.').all(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(['/', '\\'])
    })
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use anyhow::bail;

    use super::{
        java_local_explicit_static_member_imports, java_local_explicit_type_imports,
        java_local_file_dependency_paths, java_local_file_dependency_paths_with_deadline,
        java_package_directory_contains_source_file_with_deadline,
    };
    use crate::deadline::DeadlineCheck;
    use crate::language::parse_document;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct RejectAfterChecks {
        checks: Cell<usize>,
        reject_after: usize,
    }

    impl DeadlineCheck for RejectAfterChecks {
        fn check(&self, phase: &str) -> anyhow::Result<()> {
            assert_eq!(phase, "extracting local file dependencies");
            let checks = self.checks.get();
            self.checks.set(checks + 1);
            if checks >= self.reject_after {
                bail!("test deadline expired during {phase}")
            }
            Ok(())
        }
    }

    #[test]
    fn package_directory_scan_checks_deadline_after_failed_directory_read() {
        let root = temporary_dir();
        let missing_directory = root.join("missing");
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 1,
        };

        let error = java_package_directory_contains_source_file_with_deadline(
            &missing_directory,
            "com.example",
            Some(&deadline),
        )
        .expect_err("deadline should stop after a failed Java directory read");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during extracting local file dependencies"),
            "{error:#}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_directory_scan_checks_deadline_after_opening_empty_directory() {
        let root = temporary_dir();
        let empty_directory = root.join("empty");
        fs::create_dir_all(&empty_directory).unwrap();
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 1,
        };

        let error = java_package_directory_contains_source_file_with_deadline(
            &empty_directory,
            "com.example",
            Some(&deadline),
        )
        .expect_err("deadline should stop after opening an empty Java directory");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during extracting local file dependencies"),
            "{error:#}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn local_file_dependency_extraction_honors_deadline_during_directory_scan() {
        let root = temporary_dir();
        let source_path = root.join("src/com/example/Main.java");
        let candidate_path = root.join("src/com/example/Helper.java");
        fs::create_dir_all(source_path.parent().unwrap()).unwrap();
        fs::write(&candidate_path, "package com.example; class Helper {}\n").unwrap();
        let source = "package com.example;\nimport com.example.*;\n";
        fs::write(&source_path, source).unwrap();
        let document = parse_document(&source_path, source).expect("Java source should parse");
        let deadline = RejectAfterChecks {
            checks: Cell::new(0),
            reject_after: 5,
        };

        let error = java_local_file_dependency_paths_with_deadline(
            &source_path,
            document.tree.root_node(),
            source,
            Some(&deadline),
        )
        .expect_err("dependency extraction should stop while resolving source candidates");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during extracting local file dependencies")
        );
        assert!(deadline.checks.get() >= 6);
        let _ = fs::remove_dir_all(root);
    }

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
