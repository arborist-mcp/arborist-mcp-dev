use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{
    csharp_file_base_types, csharp_file_interface_parents, csharp_file_namespace_imports,
    csharp_file_static_type_imports, csharp_file_type_alias_imports,
    csharp_generic_type_semantic_path, csharp_global_namespace_imports,
    csharp_global_static_type_imports, csharp_global_type_alias_imports, detect_language,
    node_text, normalize_path, parse_document, parse_document_with_timeout, read_source,
};
use crate::model::LanguageId;
use crate::semantic::csharp::is_csharp_symbol_node;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpTypeAliasBinding {
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpStaticTypeImportBinding {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_type_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpNamespaceImportBinding {
    pub(crate) scope_path: Option<String>,
    pub(crate) semantic_namespace_path: String,
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpReceiverTypeBindings {
    types_by_name: BTreeMap<String, String>,
    /// Single-level array-typed receivers such as `Helper[] items` record the
    /// element component type so an element access (`items[0].helper(...)`)
    /// can dispatch on the element type; primitive, multi-dimensional,
    /// jagged, and malformed array spellings leave the map empty and fail
    /// closed.
    array_component_types: BTreeMap<String, String>,
    /// `var` locals bound from an element-access initializer such as
    /// `var first = items[0]` record the base spelling, call arity, and
    /// element-access depth so the local resolves to the base array's element
    /// component type; factory-call bases carry the call's argument count,
    /// and a jagged initializer such as `var first = matrix[0][0]` records a
    /// depth of two. Unsupported initializer shapes leave the map empty and
    /// fail closed.
    element_access_initializers_by_name: BTreeMap<String, (String, usize, usize)>,
    /// Names of members declared `static` on a type. Only member bindings
    /// (per-type fields, properties, and events) populate this set; local
    /// receiver bindings leave it empty so instance dispatch never consults
    /// static-ness.
    static_member_names: BTreeSet<String>,
}

impl CSharpReceiverTypeBindings {
    /// Returns whether `name` is bound as a local receiver. Callers use this
    /// to distinguish "not bound" (a receiver may be a same-named type
    /// instead) from "bound but unusable" (fail closed).
    pub(in crate::symbol_dependency) fn contains(&self, name: &str) -> bool {
        self.types_by_name.contains_key(name)
            || self.element_access_initializers_by_name.contains_key(name)
    }

    /// Returns the raw binding value for a bound name, including empty values
    /// (untyped `var` locals, lambda parameters) and factory markers.
    pub(in crate::symbol_dependency) fn raw_for(&self, name: &str) -> Option<&str> {
        self.types_by_name.get(name).map(String::as_str)
    }

    /// Returns the declared type spelling for a uniquely bound name. Names
    /// bound without a usable declared type (`var` locals, lambda parameters,
    /// `foreach` variables, local functions, and type parameters) and names
    /// bound to a marker-prefixed initializer (a factory call or a
    /// field/property-access chain) return `None`.
    pub(in crate::symbol_dependency) fn type_for(&self, name: &str) -> Option<String> {
        self.types_by_name
            .get(name)
            .filter(|type_name| !type_name.is_empty() && !type_name.starts_with('@'))
            .cloned()
    }

    /// Returns the declared element component type for a uniquely bound
    /// single-level array-typed name such as `Helper[] items`, which resolves
    /// to the element type `Helper` when a chain accesses an element. Names
    /// without a resolvable array component (non-array, primitive, and
    /// multi-dimensional or jagged array spellings) return `None`.
    pub(in crate::symbol_dependency) fn array_component_for(&self, name: &str) -> Option<String> {
        self.array_component_types
            .get(name)
            .filter(|type_name| !type_name.is_empty())
            .cloned()
    }

    /// Returns the array-typed base spelling, call arity, and element-access
    /// depth for a `var` local bound from an element-access initializer such
    /// as `var first = items[0]` or `var first = matrix[0][0]`, which
    /// resolves to the base array's element component type; factory-call
    /// bases carry the call's argument count, and the depth counts how many
    /// element-component layers the initializer strips. Names without an
    /// element-access initializer return `None`.
    pub(in crate::symbol_dependency) fn element_access_base_for(
        &self,
        name: &str,
    ) -> Option<(String, usize, usize)> {
        self.element_access_initializers_by_name
            .get(name)
            .filter(|(base_name, _, _)| !base_name.is_empty())
            .cloned()
    }

    /// Records `name` as a member declared `static`. Used by the per-type
    /// member-binding collector so static type-qualified member roots can
    /// require a static member and fail closed for instance members reached
    /// through a type name.
    pub(in crate::symbol_dependency) fn insert_static_member(&mut self, name: &str) {
        self.static_member_names.insert(name.to_string());
    }

    /// Returns whether `name` is a member declared `static`. Names absent
    /// from a type's member bindings and names never recorded as static
    /// (instance members and events) return `false`.
    pub(in crate::symbol_dependency) fn is_static_member(&self, name: &str) -> bool {
        self.static_member_names.contains(name)
    }
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpImportContext {
    type_alias_bindings: BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_type_alias_names: BTreeSet<(Option<String>, String)>,
    base_type_bindings_by_range: BTreeMap<(usize, usize), CSharpBaseTypeBinding>,
    interface_parent_bindings_by_range: BTreeMap<(usize, usize), Vec<CSharpInterfaceParentBinding>>,
    receiver_type_bindings_by_range: BTreeMap<(usize, usize), CSharpReceiverTypeBindings>,
    member_type_bindings_by_range: BTreeMap<(usize, usize), CSharpReceiverTypeBindings>,
    type_parameter_names_by_range: BTreeMap<(usize, usize), Vec<String>>,
    static_type_import_bindings: Vec<CSharpStaticTypeImportBinding>,
    namespace_import_bindings: Vec<CSharpNamespaceImportBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpBaseTypeBinding {
    pub(crate) semantic_type_path: String,
    pub(crate) is_global_qualified: bool,
    pub(crate) alias_name: Option<String>,
    pub(crate) namespace_import_paths: Vec<String>,
    /// Top-level type-argument spellings of a constructed generic binding
    /// such as `Box<Helper>` (`["Helper"]`); empty for non-generic bindings.
    /// Member-chain resolution substitutes a generic type's type parameters
    /// with these spellings when a hop's declared type references one.
    pub(crate) generic_arguments: Vec<String>,
    /// Top-level type-argument spellings for each generic segment of the
    /// spelling that precedes the final segment, outermost first; empty when
    /// the spelling has no enclosing generic segments.
    /// `Outer<Helper>.Inner<Helper>` records `[["Helper"]]` so member declared
    /// types that reference the outer type parameter (`T[] outerItems` on
    /// `Inner<U>`) substitute `T` with `Helper`.
    pub(crate) enclosing_generic_arguments: Vec<Vec<String>>,
    /// Raw top-level type-argument spellings written in the base list, such
    /// as `["T"]` for `class Derived<T> : Box<T>`; empty for non-generic
    /// base spellings and for bindings constructed from usage spellings.
    /// Generic-inheritance walks substitute the current type's parameters
    /// with its concrete arguments into these spellings to compose the base
    /// type's concrete arguments.
    pub(crate) raw_generic_argument_spellings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::symbol_dependency) struct CSharpInterfaceParentBinding {
    pub(crate) semantic_type_path: String,
    pub(crate) is_global_qualified: bool,
    /// Raw top-level type-argument spellings written in the interface's base
    /// list, such as `["T"]` for `interface IGeneric<T> : IBase<T>`; empty
    /// for non-generic parent spellings. Interface member-hop resolution
    /// substitutes the interface receiver's concrete arguments for these
    /// spellings to compose the parent interface's arguments.
    pub(crate) raw_generic_argument_spellings: Vec<String>,
}

/// Outcome of resolving an interface declaration's parent list. `None` means
/// the interface has no `base_list`; `Blocked` means a parent spelling failed
/// to normalize or resolve in the interface's file scope; `Parents` carries
/// the resolved parent bindings.
#[derive(Debug, Clone)]
pub(in crate::symbol_dependency) enum CSharpInterfaceParents {
    None,
    Blocked,
    Parents(Vec<CSharpBaseTypeBinding>),
}

#[derive(Debug, Clone, Default)]
pub(in crate::symbol_dependency) struct CSharpGlobalImportContext {
    type_alias_bindings: BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_type_alias_names: BTreeSet<(Option<String>, String)>,
    static_type_import_bindings: Vec<CSharpStaticTypeImportBinding>,
    namespace_import_bindings: Vec<CSharpNamespaceImportBinding>,
}

pub(in crate::symbol_dependency) fn csharp_global_import_context_for_files_with_overrides_and_deadline(
    source_file_paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpGlobalImportContext> {
    let mut type_alias_bindings = BTreeMap::new();
    let mut ambiguous_type_alias_names = BTreeSet::new();
    let mut static_type_import_bindings = Vec::new();
    let mut namespace_import_bindings = Vec::new();
    let mut visited_paths = BTreeSet::new();

    for source_file_path in source_file_paths {
        if let Some(deadline) = deadline {
            deadline.check("reading C# global import context")?;
        }
        let normalized_file_path = normalize_path(source_file_path);
        if !visited_paths.insert(normalized_file_path.clone()) {
            continue;
        }
        let path = Path::new(&normalized_file_path);
        if detect_language(path).ok() != Some(LanguageId::CSharp) {
            continue;
        }
        let source = file_overrides
            .and_then(|overrides| overrides.get(&normalized_file_path))
            .cloned()
            .map(Ok)
            .unwrap_or_else(|| read_source(path))?;
        if let Some(deadline) = deadline {
            deadline.check("parsing C# global import context")?;
        }
        let document = if let Some(deadline) = deadline {
            parse_document_with_timeout(
                path,
                &source,
                deadline.remaining_timeout_micros("parsing C# global import context")?,
            )?
        } else {
            parse_document(path, &source)?
        };
        let root = document.tree.root_node();
        if root.has_error() {
            continue;
        }
        for (local_name, semantic_type_path) in csharp_global_type_alias_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global type alias bindings")?;
            }
            insert_unique_csharp_type_alias_binding(
                &mut type_alias_bindings,
                &mut ambiguous_type_alias_names,
                None,
                local_name,
                CSharpTypeAliasBinding { semantic_type_path },
            );
        }
        for semantic_type_path in csharp_global_static_type_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global static import bindings")?;
            }
            static_type_import_bindings.push(CSharpStaticTypeImportBinding {
                scope_path: None,
                semantic_type_path,
            });
        }
        for semantic_namespace_path in csharp_global_namespace_imports(root, &source)? {
            if let Some(deadline) = deadline {
                deadline.check("extracting C# global namespace import bindings")?;
            }
            namespace_import_bindings.push(CSharpNamespaceImportBinding {
                scope_path: None,
                semantic_namespace_path,
            });
        }
    }

    Ok(CSharpGlobalImportContext {
        type_alias_bindings,
        ambiguous_type_alias_names,
        static_type_import_bindings,
        namespace_import_bindings,
    })
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_type_alias_binding_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Option<(String, CSharpTypeAliasBinding)> {
    let (local_type_name, method_name) = reference_name.split_once('.')?;
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return None;
    }
    let binding = context
        .type_alias_bindings
        .get(&(None, local_type_name.to_string()))?
        .clone();
    Some((method_name.to_string(), binding))
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_nested_type_alias_binding_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Option<(String, String, CSharpTypeAliasBinding)> {
    let (local_type_name, nested_reference) = reference_name.split_once('.')?;
    let (nested_type_path, method_name) = nested_reference.rsplit_once('.')?;
    if local_type_name.is_empty()
        || nested_type_path.is_empty()
        || method_name.is_empty()
        || nested_type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return None;
    }
    let binding = context
        .type_alias_bindings
        .get(&(None, local_type_name.to_string()))?
        .clone();
    Some((
        nested_type_path.replace('.', "::"),
        method_name.to_string(),
        binding,
    ))
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_base_type_alias(
    local_type_name: &str,
    context: &CSharpGlobalImportContext,
) -> Option<CSharpTypeAliasBinding> {
    if local_type_name.is_empty() {
        return None;
    }
    context
        .type_alias_bindings
        .get(&(None, local_type_name.to_string()))
        .cloned()
}

pub(in crate::symbol_dependency) fn csharp_global_base_type_alias_is_ambiguous(
    local_type_name: &str,
    context: &CSharpGlobalImportContext,
) -> bool {
    !local_type_name.is_empty()
        && context
            .ambiguous_type_alias_names
            .contains(&(None, local_type_name.to_string()))
}

pub(in crate::symbol_dependency) fn csharp_global_base_namespace_import_paths(
    context: &CSharpGlobalImportContext,
) -> Vec<String> {
    context
        .namespace_import_bindings
        .iter()
        .map(|binding| binding.semantic_namespace_path.clone())
        .collect()
}

pub(in crate::symbol_dependency) fn csharp_global_type_alias_name_is_ambiguous(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> bool {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return false;
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return false;
    }
    context
        .ambiguous_type_alias_names
        .contains(&(None, local_type_name.to_string()))
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_static_type_imports_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Vec<CSharpStaticTypeImportBinding> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Vec::new();
    }
    context.static_type_import_bindings.clone()
}

fn csharp_import_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpImportContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::CSharp) {
        return Ok(CSharpImportContext::default());
    }

    if let Some(deadline) = deadline {
        deadline.check("reading C# import context")?;
    }
    let source = file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))?;
    if let Some(deadline) = deadline {
        deadline.check("parsing C# import context")?;
    }
    let document = if let Some(deadline) = deadline {
        parse_document_with_timeout(
            path,
            &source,
            deadline.remaining_timeout_micros("parsing C# import context")?,
        )?
    } else {
        parse_document(path, &source)?
    };
    let root = document.tree.root_node();
    if root.has_error() {
        return Ok(CSharpImportContext::default());
    }

    let mut type_alias_bindings = BTreeMap::new();
    let mut ambiguous_alias_names = BTreeSet::new();
    let mut base_type_bindings_by_range = BTreeMap::new();
    let mut interface_parent_bindings_by_range: BTreeMap<
        (usize, usize),
        Vec<CSharpInterfaceParentBinding>,
    > = BTreeMap::new();
    for import in csharp_file_type_alias_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# type alias bindings")?;
        }
        insert_unique_csharp_type_alias_binding(
            &mut type_alias_bindings,
            &mut ambiguous_alias_names,
            import.scope_path,
            import.local_name,
            CSharpTypeAliasBinding {
                semantic_type_path: import.semantic_type_path,
            },
        );
    }
    for base_type in csharp_file_base_types(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# base type bindings")?;
        }
        if base_type_bindings_by_range
            .insert(
                base_type.type_range,
                CSharpBaseTypeBinding {
                    semantic_type_path: base_type.semantic_base_type_path,
                    is_global_qualified: base_type.is_global_qualified,
                    alias_name: None,
                    namespace_import_paths: Vec::new(),
                    generic_arguments: Vec::new(),
                    raw_generic_argument_spellings: base_type.generic_argument_spellings.clone(),
                    enclosing_generic_arguments: Vec::new(),
                },
            )
            .is_some()
        {
            return Ok(CSharpImportContext::default());
        }
    }
    for interface_parent in csharp_file_interface_parents(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# interface parent bindings")?;
        }
        interface_parent_bindings_by_range
            .entry(interface_parent.declaration_range)
            .or_default()
            .push(CSharpInterfaceParentBinding {
                semantic_type_path: interface_parent.semantic_type_path,
                is_global_qualified: interface_parent.is_global_qualified,
                raw_generic_argument_spellings: interface_parent.generic_argument_spellings.clone(),
            });
    }
    let mut static_type_import_bindings = Vec::new();
    for import in csharp_file_static_type_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# static type import bindings")?;
        }
        static_type_import_bindings.push(CSharpStaticTypeImportBinding {
            scope_path: import.scope_path,
            semantic_type_path: import.semantic_type_path,
        });
    }
    let mut namespace_import_bindings = Vec::new();
    for import in csharp_file_namespace_imports(root, &source)? {
        if let Some(deadline) = deadline {
            deadline.check("extracting C# namespace import bindings")?;
        }
        namespace_import_bindings.push(CSharpNamespaceImportBinding {
            scope_path: import.scope_path,
            semantic_namespace_path: import.semantic_namespace_path,
        });
    }
    let mut receiver_type_bindings_by_range = BTreeMap::new();
    collect_csharp_receiver_type_bindings(root, &source, &mut receiver_type_bindings_by_range)?;
    let mut member_type_bindings_by_range = BTreeMap::new();
    collect_csharp_member_type_bindings_by_range(
        root,
        &source,
        &mut member_type_bindings_by_range,
    )?;
    let mut type_parameter_names_by_range = BTreeMap::new();
    collect_csharp_type_parameter_names_by_range(
        root,
        &source,
        &mut type_parameter_names_by_range,
    )?;
    Ok(CSharpImportContext {
        type_alias_bindings,
        ambiguous_type_alias_names: ambiguous_alias_names,
        base_type_bindings_by_range,
        interface_parent_bindings_by_range,
        receiver_type_bindings_by_range,
        member_type_bindings_by_range,
        type_parameter_names_by_range,
        static_type_import_bindings,
        namespace_import_bindings,
    })
}

fn collect_csharp_receiver_type_bindings(
    node: tree_sitter::Node<'_>,
    source: &str,
    bindings_by_range: &mut BTreeMap<(usize, usize), CSharpReceiverTypeBindings>,
) -> Result<()> {
    if matches!(
        node.kind(),
        "method_declaration" | "constructor_declaration"
    ) && is_csharp_symbol_node(node)
    {
        bindings_by_range.insert(
            (node.start_byte(), node.end_byte()),
            csharp_receiver_type_bindings_for_node(node, source)?,
        );
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_csharp_receiver_type_bindings(child, source, bindings_by_range)?;
    }
    Ok(())
}

fn collect_csharp_member_type_bindings_by_range(
    node: tree_sitter::Node<'_>,
    source: &str,
    bindings_by_range: &mut BTreeMap<(usize, usize), CSharpReceiverTypeBindings>,
) -> Result<()> {
    if is_csharp_type_declaration(node) {
        let mut bindings = CSharpReceiverTypeBindings::default();
        collect_csharp_type_member_bindings(node, node, source, &mut bindings)?;
        bindings_by_range.insert((node.start_byte(), node.end_byte()), bindings);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_csharp_member_type_bindings_by_range(child, source, bindings_by_range)?;
    }
    Ok(())
}

/// Collects the ordered type-parameter names of each generic type
/// declaration (such as `T` in `class Box<T>`) keyed by the declaration's
/// byte range, so member-chain resolution can substitute a constructed
/// receiver's type arguments for the type parameters in member declared
/// types. Non-generic type declarations record an empty name list.
fn collect_csharp_type_parameter_names_by_range(
    node: tree_sitter::Node<'_>,
    source: &str,
    names_by_range: &mut BTreeMap<(usize, usize), Vec<String>>,
) -> Result<()> {
    if is_csharp_type_declaration(node) {
        let mut names = Vec::new();
        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            if child.kind() == "type_parameter_list" {
                let mut parameter_cursor = child.walk();
                for parameter in child.named_children(&mut parameter_cursor) {
                    if parameter.kind() == "type_parameter"
                        && let Some(name) = parameter.child_by_field_name("name")
                    {
                        let name = node_text(name, source)?.trim();
                        if !name.is_empty() {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
        names_by_range.insert((node.start_byte(), node.end_byte()), names);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_csharp_type_parameter_names_by_range(child, source, names_by_range)?;
    }
    Ok(())
}

/// Collects locally bound receiver names with their declared type spellings
/// for a function. Parameters, typed locals, and enclosing-type fields and
/// properties carry their declared type, and `var` foreach variables infer
/// the collection's element type when it names a bound array-typed value;
/// `var` locals, lambda parameters, unresolvable `foreach` variables, local
/// functions, and type parameters are bound with an empty type so they still
/// suppress static type interpretation while failing closed for instance
/// dispatch. This mirrors the extractor's binding rules so the resolver
/// classifies `receiver.method(...)` facts the same way the extractor
/// recorded them.
fn csharp_receiver_type_bindings_for_node(
    function: tree_sitter::Node<'_>,
    source: &str,
) -> Result<CSharpReceiverTypeBindings> {
    let mut bindings = CSharpReceiverTypeBindings::default();
    collect_csharp_enclosing_type_bindings(function, source, &mut bindings)?;
    collect_csharp_function_bindings(function, source, &mut bindings)?;
    Ok(bindings)
}

fn collect_csharp_enclosing_type_bindings(
    function: tree_sitter::Node<'_>,
    source: &str,
    bindings: &mut CSharpReceiverTypeBindings,
) -> Result<()> {
    let mut current = function.parent();
    while let Some(node) = current {
        if is_csharp_type_declaration(node) {
            return collect_csharp_type_member_bindings(node, node, source, bindings);
        }
        current = node.parent();
    }
    Ok(())
}

/// Collects the direct member bindings (fields, properties, and events) of a
/// type declaration. Nested type declarations, methods, and constructors stop
/// the walk so only the type's own members are bound.
fn collect_csharp_type_member_bindings(
    node: tree_sitter::Node<'_>,
    root: tree_sitter::Node<'_>,
    source: &str,
    bindings: &mut CSharpReceiverTypeBindings,
) -> Result<()> {
    if node != root && is_csharp_symbol_node(node) {
        return Ok(());
    }
    if node.kind() == "field_declaration" {
        let is_static = csharp_member_declaration_is_static(node, source);
        let mut declaration_cursor = node.walk();
        for declaration in node
            .named_children(&mut declaration_cursor)
            .filter(|child| child.kind() == "variable_declaration")
        {
            let type_name = csharp_declared_type_name(declaration, source)?;
            let mut declarator_cursor = declaration.walk();
            for declarator in declaration
                .named_children(&mut declarator_cursor)
                .filter(|child| child.kind() == "variable_declarator")
            {
                let Some(name) = declarator.child_by_field_name("name") else {
                    continue;
                };
                let name = node_text(name, source)?.trim();
                if is_static {
                    bindings.insert_static_member(name);
                }
                csharp_insert_receiver_binding(bindings, name, type_name.clone());
            }
        }
    }
    if matches!(node.kind(), "property_declaration" | "event_declaration")
        && let Some(name) = node.child_by_field_name("name")
    {
        let name = node_text(name, source)?.trim();
        // Static type-qualified roots require a static field or property;
        // static events are not value members and stay untracked so they fail
        // closed for `var` initializers while still serving as instance
        // member-chain hops.
        if node.kind() == "property_declaration"
            && csharp_member_declaration_is_static(node, source)
        {
            bindings.insert_static_member(name);
        }
        csharp_insert_receiver_binding(bindings, name, csharp_declared_type_name(node, source)?);
    }
    if node.kind() == "parameter_list"
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "record_declaration")
    {
        // A record primary-constructor parameter list such as
        // `record Base(Group Holder)` declares each positional parameter as an
        // init-only property of the record, so it binds the same member name
        // and declared type as an explicit property declaration for inherited
        // bare-root and member-chain resolution.
        let mut cursor = node.walk();
        for parameter in node
            .named_children(&mut cursor)
            .filter(|child| child.kind() == "parameter")
        {
            let Some(name) = parameter.child_by_field_name("name") else {
                continue;
            };
            let name = node_text(name, source)?.trim();
            if name.is_empty() {
                continue;
            }
            csharp_insert_receiver_binding(
                bindings,
                name,
                csharp_declared_type_name(parameter, source)?,
            );
        }
    }
    if node.kind() == "type_parameter"
        && let Some(name) = node.child_by_field_name("name")
    {
        let name = node_text(name, source)?.trim();
        csharp_insert_receiver_binding(bindings, name, None);
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_csharp_type_member_bindings(child, root, source, bindings)?;
    }
    Ok(())
}

/// Returns whether a field or property declaration is declared `static`.
/// The modifier appears in the declaration's source text before the member
/// type, so a whitespace token scan is sufficient and matches how indexed
/// symbols record static-ness in their signature.
fn csharp_member_declaration_is_static(node: tree_sitter::Node<'_>, source: &str) -> bool {
    source
        .get(node.start_byte()..node.end_byte())
        .is_some_and(|text| text.split_whitespace().any(|part| part == "static"))
}

fn collect_csharp_function_bindings(
    function: tree_sitter::Node<'_>,
    source: &str,
    bindings: &mut CSharpReceiverTypeBindings,
) -> Result<()> {
    fn declarator_name(node: tree_sitter::Node<'_>, source: &str) -> Result<Option<String>> {
        let name = if let Some(name) = node.child_by_field_name("name") {
            name
        } else {
            let mut cursor = node.walk();
            let Some(declarator) = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "variable_declarator")
            else {
                return Ok(None);
            };
            let Some(name) = declarator.child_by_field_name("name") else {
                return Ok(None);
            };
            name
        };
        let name = node_text(name, source)?.trim();
        Ok((!name.is_empty()).then(|| name.to_string()))
    }

    fn collect(
        node: tree_sitter::Node<'_>,
        source: &str,
        bindings: &mut CSharpReceiverTypeBindings,
    ) -> Result<()> {
        if is_csharp_symbol_node(node) && node.parent().is_some() {
            return Ok(());
        }
        if node.kind() == "implicit_parameter" {
            let name = node_text(node, source)?.trim();
            csharp_insert_receiver_binding(bindings, name, None);
        }
        if matches!(
            node.kind(),
            "parameter" | "catch_declaration" | "declaration_expression" | "declaration_pattern"
        ) && let Some(name) = declarator_name(node, source)?
        {
            csharp_insert_receiver_binding(
                bindings,
                &name,
                csharp_declared_type_name(node, source)?,
            );
        }
        if node.kind() == "local_function_statement"
            && let Some(name) = declarator_name(node, source)?
        {
            csharp_insert_receiver_binding(bindings, &name, None);
        }
        if node.kind() == "variable_declarator"
            && let Some(name) = node.child_by_field_name("name")
        {
            let name = node_text(name, source)?.trim();
            let type_name = match node.parent() {
                Some(parent) if parent.kind() == "variable_declaration" => {
                    match csharp_declared_type_name(parent, source)? {
                        Some(type_name) => Some(type_name),
                        // A `var` local infers its receiver type from a
                        // constructor initializer such as
                        // `var helper = new Helper()`, from a factory call
                        // initializer such as `var helper = MakeHelper()`,
                        // from a field/property-access initializer such as
                        // `var helper = this.holder.helper`, or from an
                        // element-access initializer such as
                        // `var first = items[0]` whose base array's element
                        // component type pins the receiver; other
                        // initializers bind an empty type and fail closed.
                        None => {
                            if let Some(type_name) =
                                csharp_var_initializer_type_binding(node, source, bindings)?
                            {
                                Some(type_name)
                            } else if let Some((base_spelling, call_arity, depth)) =
                                csharp_initializer_element_access_from_declarator(node, source)?
                            {
                                insert_csharp_element_access_initializer(
                                    bindings,
                                    name,
                                    base_spelling,
                                    call_arity,
                                    depth,
                                );
                                None
                            } else {
                                None
                            }
                        }
                    }
                }
                _ => None,
            };
            csharp_insert_receiver_binding(bindings, name, type_name);
        }
        if node.kind() == "foreach_statement"
            && let Some(left) = node.child_by_field_name("left")
            && left.kind() == "identifier"
        {
            let name = node_text(left, source)?.trim();
            if name.is_empty() {
                return Ok(());
            }
            // An `await foreach` loop iterates an async stream whose element
            // type is capability-gated; bind an empty type and fail closed
            // instead of inferring from a synchronous array/factory/member
            // collection. An explicitly typed foreach variable such as
            // `foreach (Helper row in matrix)` binds the declared type, and a
            // `var` foreach variable infers its element type from the
            // collection expression when it names a bound array-typed value,
            // so `foreach (var item in items)` over `Helper[]` binds `item`
            // to `Helper` and a jagged `Helper[][]` collection binds the
            // nested `Helper[]` component. Unresolvable collections (factory
            // calls, generic collections, primitives, and unknown names) bind
            // an empty type and fail closed.
            let type_name = if csharp_foreach_statement_is_await(node) {
                None
            } else if let Some(type_name) = csharp_declared_type_name(node, source)? {
                Some(type_name)
            } else {
                match node.child_by_field_name("right") {
                    Some(right) => csharp_foreach_collection_element_type(right, source, bindings)?,
                    None => None,
                }
            };
            csharp_insert_receiver_binding(bindings, name, type_name);
        }

        let mut cursor = node.walk();
        for child in node.named_children(&mut cursor) {
            collect(child, source, bindings)?;
        }
        Ok(())
    }

    let mut cursor = function.walk();
    for child in function.named_children(&mut cursor) {
        collect(child, source, bindings)?;
    }
    Ok(())
}

/// Returns whether a `foreach_statement` is an `await foreach` async stream
/// iteration whose element type is capability-gated and fails closed.
fn csharp_foreach_statement_is_await(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|child| child.kind() == "await")
}

/// Returns the inner expression of a `parenthesized_expression` such as
/// `(MakeFactory())` or `(group)`, recovering the anonymous keyword token for
/// `(this)`; malformed or empty parentheses return `None` and fail closed.
/// Mirrors the extractor's parenthesized unwrapping.
fn csharp_parenthesized_inner_expression(
    node: tree_sitter::Node<'_>,
) -> Option<tree_sitter::Node<'_>> {
    let inner = {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).next()
    };
    match inner {
        Some(inner) => Some(inner),
        None => {
            let mut cursor = node.walk();
            node.children(&mut cursor)
                .find(|child| !matches!(child.kind(), "(" | ")"))
        }
    }
}

/// Infers a receiver type binding for `var` locals. Constructor initializers
/// such as `var helper = new Helper()` or `var helper = new Outer.Inner()`
/// bind the constructed type; invocation initializers such as
/// `var helper = MakeHelper()` bind a factory marker the resolver expands to
/// the factory method's declared return type; identifier and
/// member-access initializers such as `var helper = this.holder.helper` bind
/// a chain marker the resolver walks to the final member's declared type.
/// Other initializers, target-typed creations, array creations, and malformed
/// spellings return `None` and fail closed. Mirrors the extractor's `var`
/// binding rules so the resolver classifies initializers the same way the
/// extractor recorded them.
fn csharp_var_initializer_type_binding(
    declarator: tree_sitter::Node<'_>,
    source: &str,
    bindings: &CSharpReceiverTypeBindings,
) -> Result<Option<String>> {
    let Some(initializer) = csharp_declarator_initializer(declarator) else {
        return Ok(None);
    };
    csharp_initializer_type_binding(initializer, source, bindings)
}

/// Records a `var` local whose initializer is an element access such as
/// `var first = items[0]`, returning the array-typed base spelling and call
/// arity, and element-access depth so the local resolves to the base array's
/// element component type. Plain-identifier bases such as `items`, `local`,
/// or a bare enclosing-class field name and member-access bases such as
/// `this.fieldItems` or `group.holder.fieldItems` record the spelling with
/// arity zero and depth one; factory-call bases such as `makeItems()` or
/// `Util.makeItems()` record the reference with a trailing `()` marker and
/// the call's argument count, and a jagged base such as `matrix[0][0]`
/// records depth two. Null-conditional element-access roots such as
/// `var first = group?.items[0]`, `var first = group?.GetItems()[0]`, or
/// `var first = (new Group()).GetItems()?[0]` record the same base spellings
/// as their plain dotted forms so the local dispatches on the same array
/// element component type. Unsupported initializer shapes record nothing and
/// fail closed. Mirrors the extractor's element-access binding rules.
fn csharp_initializer_element_access_from_declarator(
    declarator: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<(String, usize, usize)>> {
    let Some(initializer) = csharp_declarator_initializer(declarator) else {
        return Ok(None);
    };
    let initializer = match initializer.kind() {
        "parenthesized_expression" => {
            let Some(inner) = csharp_parenthesized_inner_expression(initializer) else {
                return Ok(None);
            };
            inner
        }
        _ => initializer,
    };
    match initializer.kind() {
        "element_access_expression" => csharp_element_access_base_binding(initializer, source),
        "conditional_access_expression" => {
            // A null-conditional element access such as `group?.items?[0]`,
            // `group?.GetItems()?[0]`, or `(new Group()).GetItems()?[0]`
            // models the `?[0]` hop as an element binding on the conditional
            // access; the condition expression is the array being indexed, so
            // its base binding is recorded at element-access depth one.
            if !csharp_conditional_access_has_element_binding(initializer) {
                return Ok(None);
            }
            let Some(condition) = initializer.child_by_field_name("condition") else {
                return Ok(None);
            };
            csharp_element_access_array_binding(condition, source)
        }
        _ => Ok(None),
    }
}

/// Returns whether a conditional access carries an element binding such as
/// the `?[0]` hop of `group?.items?[0]` or `(new Group()).GetItems()?[0]`,
/// which indexes the condition expression as an array.
fn csharp_conditional_access_has_element_binding(node: tree_sitter::Node<'_>) -> bool {
    if node.kind() != "conditional_access_expression" {
        return false;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| child.kind() == "element_binding_expression")
}

/// Resolves the base spelling, call arity, and element-access depth of an
/// element-access initializer root such as `items[0]` (depth one),
/// `matrix[0][0]` (depth two), `makeItems()[0]`, `(this.fieldItems)[0]`,
/// `group?.items[0]`, or `group?.GetItems()[0]`. A nested element-access
/// base recurses and increments the depth, and a parenthesized base array
/// unwraps to the same shape as the unparenthesized form. Unsupported base
/// shapes return `None` and fail closed.
fn csharp_element_access_base_binding(
    element_access: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<(String, usize, usize)>> {
    let Some(array) = element_access.child_by_field_name("expression") else {
        return Ok(None);
    };
    csharp_element_access_array_binding(array, source)
}

/// Resolves the base spelling, call arity, and element-access depth of an
/// array expression being indexed by an element access or a null-conditional
/// element binding, such as `items`, `this.fieldItems`, `group?.items`,
/// `makeItems()`, `group.GetItems()`, `group?.GetItems()`,
/// `(new Group()).GetItems()`, or `matrix[0]` (depth two). A nested
/// element-access base recurses and increments the depth, and a
/// parenthesized base array unwraps to the same shape as the unparenthesized
/// form. Null-conditional member hops (`?.`) spell the same as plain dotted
/// hops so the resolver dispatches the same member or factory. Unsupported
/// base shapes return `None` and fail closed.
fn csharp_element_access_array_binding(
    array: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<(String, usize, usize)>> {
    let mut array = array;
    // A parenthesized base array such as `(Util.makeItems())` in
    // `var first = (Util.makeItems())[0]` unwraps to the same base shape as
    // the unparenthesized form before dispatch.
    while array.kind() == "parenthesized_expression" {
        let Some(inner) = csharp_parenthesized_inner_expression(array) else {
            return Ok(None);
        };
        array = inner;
    }
    if array.kind() == "element_access_expression" {
        // A jagged element access such as `matrix[0][0]` nests element-access
        // nodes; recurse into the inner access and count one more layer.
        let Some(inner_array) = array.child_by_field_name("expression") else {
            return Ok(None);
        };
        let Some((base_spelling, call_arity, depth)) =
            csharp_element_access_array_binding(inner_array, source)?
        else {
            return Ok(None);
        };
        return Ok(Some((base_spelling, call_arity, depth + 1)));
    }
    let (base_spelling, call_arity) = match array.kind() {
        "identifier" | "member_access_expression" => {
            // A member-access base containing a null-conditional hop such as
            // `group?.holder.items` spells the same chain as the plain dotted
            // form so the resolver walks the same member hops.
            let base_name = node_text(array, source)?.trim().replace("?.", ".");
            if base_name.is_empty() {
                return Ok(None);
            }
            (base_name.to_string(), 0)
        }
        "conditional_access_expression" => {
            // A null-conditional member base such as `group?.items` spells the
            // same chain as the plain dotted form so the resolver walks the
            // same member hops; an element-binding conditional (`?[0]`) is the
            // indexed array handled by the caller, not a base spelling.
            if csharp_chain_member_hop_parts(array).is_none() {
                return Ok(None);
            }
            let base_name = node_text(array, source)?.trim().replace("?.", ".");
            if base_name.is_empty() {
                return Ok(None);
            }
            (base_name.to_string(), 0)
        }
        "invocation_expression" => {
            let Some(function) = array.child_by_field_name("function") else {
                return Ok(None);
            };
            let spelling = match function.kind() {
                "identifier" | "member_access_expression" => {
                    node_text(function, source)?.trim().replace("?.", ".")
                }
                "conditional_access_expression" => {
                    // A null-conditional factory hop such as
                    // `group?.GetItems()` spells the same callee as the plain
                    // dotted form so the resolver dispatches the factory on
                    // the same receiver.
                    if csharp_chain_member_hop_parts(function).is_none() {
                        return Ok(None);
                    }
                    node_text(function, source)?.trim().replace("?.", ".")
                }
                _ => return Ok(None),
            };
            if spelling.is_empty() {
                return Ok(None);
            }
            let Some(arguments) = array.child_by_field_name("arguments") else {
                return Ok(None);
            };
            let mut cursor = arguments.walk();
            let arity = arguments.named_children(&mut cursor).count();
            (format!("{spelling}()"), arity)
        }
        _ => return Ok(None),
    };
    if base_spelling.is_empty() {
        return Ok(None);
    }
    Ok(Some((base_spelling, call_arity, 1)))
}

/// Infers a receiver type binding from an initializer expression, unwrapping
/// parenthesized initializers such as `(MakeFactory())`, `(new Helper())`, or
/// `(MakeHelper()).entry` to the same shape as the unparenthesized form.
/// Mirrors the extractor's `var` initializer binding rules.
fn csharp_initializer_type_binding(
    initializer: tree_sitter::Node<'_>,
    source: &str,
    bindings: &CSharpReceiverTypeBindings,
) -> Result<Option<String>> {
    match initializer.kind() {
        "parenthesized_expression" => {
            let Some(inner) = csharp_parenthesized_inner_expression(initializer) else {
                return Ok(None);
            };
            csharp_initializer_type_binding(inner, source, bindings)
        }
        "object_creation_expression" => {
            let Some(type_node) = initializer.child_by_field_name("type") else {
                return Ok(None);
            };
            let type_name = node_text(type_node, source)?.trim();
            if type_name.is_empty() || type_name == "var" {
                return Ok(None);
            }
            Ok(Some(type_name.to_string()))
        }
        "invocation_expression" => csharp_factory_marker_from_initializer(initializer, source),
        "member_access_expression" | "conditional_access_expression" | "identifier" => {
            let Some(chain) = csharp_initializer_chain_spelling(initializer, source, bindings)?
            else {
                return Ok(None);
            };
            Ok(Some(format!("@init:{chain}")))
        }
        _ => Ok(None),
    }
}

/// Returns the receiver and member-name nodes of a member-access or
/// null-conditional member hop such as `group.items`, `group?.items`, or the
/// `?.inner()` function of `group?.inner()`. Null-conditional hops dispatch
/// the same instance member on the condition's type, so they spell the same
/// as plain member access. Mirrors the extractor's hop helper so resolver
/// bindings stay aligned with extracted references.
fn csharp_chain_member_hop_parts(
    node: tree_sitter::Node<'_>,
) -> Option<(tree_sitter::Node<'_>, tree_sitter::Node<'_>)> {
    match node.kind() {
        "member_access_expression" => {
            let receiver = node.child_by_field_name("expression")?;
            let name = node.child_by_field_name("name")?;
            Some((receiver, name))
        }
        "conditional_access_expression" => {
            let receiver = node.child_by_field_name("condition")?;
            let mut cursor = node.walk();
            let member_binding = node
                .named_children(&mut cursor)
                .find(|child| child.kind() == "member_binding_expression")?;
            let name = member_binding.child_by_field_name("name")?;
            Some((receiver, name))
        }
        _ => None,
    }
}

/// Builds a field/property-access chain spelling such as `helper`,
/// `this.holder.helper`, `holder.helper`, `Holder().helper`, or
/// `Util.STATIC_HELPER`, or `MakeHelper().entry` for a `var` local initialized
/// from an identifier or member-access expression. The spelling mirrors the
/// extractor's member-chain spelling so the resolver can walk the chain to the
/// final member's declared type. A chain whose leading receiver is an unbound
/// identifier is kept as a static type-qualified candidate, and a leading bare
/// method call (`MakeHelper()` in `MakeHelper().entry`) is kept so the resolver
/// can dispatch it as a factory method; untyped bound receivers and
/// unsupported initializer shapes return `None` and fail closed.
fn csharp_initializer_chain_spelling(
    initializer: tree_sitter::Node<'_>,
    source: &str,
    bindings: &CSharpReceiverTypeBindings,
) -> Result<Option<String>> {
    let mut segments = Vec::new();
    let mut current = initializer;
    loop {
        match current.kind() {
            "member_access_expression" | "conditional_access_expression" => {
                let Some((receiver, name)) = csharp_chain_member_hop_parts(current) else {
                    return Ok(None);
                };
                let name = node_text(name, source)?.trim();
                if name.is_empty() {
                    return Ok(None);
                }
                segments.push(name.to_string());
                current = receiver;
            }
            "invocation_expression" => {
                let Some(function) = current.child_by_field_name("function") else {
                    return Ok(None);
                };
                let Some(arguments) = current.child_by_field_name("arguments") else {
                    return Ok(None);
                };
                let mut cursor = arguments.walk();
                let arity = arguments.named_children(&mut cursor).count();
                // A bare-call root such as `MakeHelper()` in
                // `MakeHelper().entry` records the call as the leading chain
                // segment; the resolver dispatches it as a factory method on
                // the enclosing type or a static-imported type.
                if !matches!(
                    function.kind(),
                    "member_access_expression" | "conditional_access_expression"
                ) {
                    if function.kind() != "identifier" {
                        return Ok(None);
                    }
                    let name = node_text(function, source)?.trim();
                    if name.is_empty() {
                        return Ok(None);
                    }
                    if arity == 0 {
                        segments.push(format!("{name}()"));
                    } else {
                        segments.push(format!("{name}({arity})"));
                    }
                    break;
                }
                let Some((receiver, name)) = csharp_chain_member_hop_parts(function) else {
                    return Ok(None);
                };
                let name = node_text(name, source)?.trim();
                if name.is_empty() {
                    return Ok(None);
                }
                if arity == 0 {
                    segments.push(format!("{name}()"));
                } else {
                    segments.push(format!("{name}({arity})"));
                }
                current = receiver;
            }
            "identifier" => {
                let base = node_text(current, source)?.trim();
                // A locally bound receiver keeps an instance chain only when
                // its declared type is usable; untyped bindings (`var`
                // locals, lambda parameters) fail closed. An unbound
                // identifier is kept as a static type-qualified candidate so
                // the resolver can look the member up on the resolved type.
                if base.is_empty() || bindings.raw_for(base).is_some_and(|raw| raw.is_empty()) {
                    return Ok(None);
                }
                segments.push(base.to_string());
                break;
            }
            "this" => {
                segments.push("this".to_string());
                break;
            }
            "base" => {
                segments.push("base".to_string());
                break;
            }
            "alias_qualified_name" => {
                // A `global::`-qualified root such as
                // `global::Demo.Util.STATIC_HELPER` records the alias-qualified
                // spelling (`global::Demo`) as the leading segment; the
                // resolver re-joins the dotted type path when resolving the
                // static member.
                let spelling = node_text(current, source)?.trim();
                if spelling.is_empty() {
                    return Ok(None);
                }
                segments.push(spelling.to_string());
                break;
            }
            "object_creation_expression" => {
                let Some(spelling) = csharp_constructor_type_spelling(current, source)? else {
                    return Ok(None);
                };
                segments.push(spelling);
                break;
            }
            "parenthesized_expression" => {
                // A parenthesized segment such as `(MakeFactory())` in
                // `(MakeFactory()).entry` or `(group).inner().helper` unwraps
                // to its inner expression and keeps walking the chain so
                // parentheses never break initializer-chain inference.
                let Some(inner) = csharp_parenthesized_inner_expression(current) else {
                    return Ok(None);
                };
                current = inner;
            }
            _ => return Ok(None),
        }
    }
    segments.reverse();
    Ok(Some(segments.join(".")))
}

/// Builds a constructed-type spelling such as `Holder()` or `Outer.Inner()`
/// for an `object_creation_expression` chain root. Anonymous or malformed
/// creations produce no spelling and fail closed. Mirrors the extractor's
/// constructor spelling rules.
fn csharp_constructor_type_spelling(
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let text = node_text(node, source)?.trim();
    let Some(type_name) = text.strip_prefix("new") else {
        return Ok(None);
    };
    let type_name = type_name.trim_start();
    if type_name.is_empty() || type_name.contains("::") {
        return Ok(None);
    }
    let Some(type_name) = strip_csharp_constructor_suffix(type_name) else {
        return Ok(None);
    };
    let type_name = type_name.trim();
    if type_name.is_empty() || type_name == "var" {
        return Ok(None);
    }
    let Some(semantic_type_path) = csharp_generic_type_semantic_path(type_name) else {
        return Ok(None);
    };
    Ok(Some(format!("{}()", semantic_type_path.replace("::", "."))))
}

/// Strips a trailing object-initializer body (`{ ... }`) and constructor
/// argument list (`(...)`) from a constructed type spelling, leaving the bare
/// type path such as `Helper` or `Outer.Inner`. Mirrors the extractor's
/// constructor suffix rules.
fn strip_csharp_constructor_suffix(type_name: &str) -> Option<&str> {
    let mut trimmed = type_name.trim_end();
    if trimmed.ends_with('}') {
        trimmed = strip_csharp_balanced_suffix(trimmed, '{', '}')?;
        trimmed = trimmed.trim_end();
    }
    if trimmed.ends_with(')') {
        trimmed = strip_csharp_balanced_suffix(trimmed, '(', ')')?;
        trimmed = trimmed.trim_end();
    }
    Some(trimmed)
}

/// Strips a balanced trailing `open`/`close` pair from `text`, returning the
/// text before it. Unbalanced or absent pairs return `None`. Mirrors the
/// extractor's constructor suffix helper.
fn strip_csharp_balanced_suffix(text: &str, open: char, close: char) -> Option<&str> {
    let mut depth = 0usize;
    for (index, character) in text.char_indices().rev() {
        if character == close {
            depth += 1;
        } else if character == open {
            depth = depth.checked_sub(1)?;
            if depth == 0 {
                return Some(&text[..index]);
            }
        }
    }
    None
}

/// Builds a factory marker such as `@factory:MakeHelper(0)`,
/// `@factory:this.MakeHelper(0)`, or `@factory:Factories.MakeHelper(1)` for a
/// `var` local initialized from an invocation. The spelling mirrors the
/// extractor's reference spellings so the resolver can expand the marker to
/// the factory method's declared return type. Unsupported initializer shapes
/// return `None` and fail closed. Mirrors the extractor's factory marker rules.
fn csharp_factory_marker_from_initializer(
    initializer: tree_sitter::Node<'_>,
    source: &str,
) -> Result<Option<String>> {
    let Some(function) = initializer.child_by_field_name("function") else {
        return Ok(None);
    };
    let spelling = match function.kind() {
        "identifier" => node_text(function, source)?.trim().to_string(),
        "member_access_expression" | "conditional_access_expression" => {
            let Some((expression, name)) = csharp_chain_member_hop_parts(function) else {
                return Ok(None);
            };
            let expression_text = node_text(expression, source)?.trim();
            let name_text = node_text(name, source)?.trim();
            if expression_text.is_empty() || name_text.is_empty() {
                return Ok(None);
            }
            format!("{expression_text}.{name_text}")
        }
        _ => return Ok(None),
    };
    // A null-conditional factory hop such as `group?.GetSingle()` or
    // `new Group().GetMaybe()?.inner()` dispatches the same factory member as
    // the plain dotted spelling, so the marker normalizes `?.` hops to `.`
    // and the resolver expands the chain as if the access were unconditional.
    let spelling = spelling.replace("?.", ".");
    if spelling.is_empty() {
        return Ok(None);
    }
    let Some(arguments) = initializer.child_by_field_name("arguments") else {
        return Ok(None);
    };
    let mut cursor = arguments.walk();
    let arity = arguments.named_children(&mut cursor).count();
    Ok(Some(format!("@factory:{spelling}({arity})")))
}

/// Returns the initializer expression of a `variable_declarator` such as
/// `helper = new Helper()`, `helper = MakeHelper()`, or `helper = helper`.
/// The grammar does not name the `= expression` child, so the last named
/// child that is not the declared name or a tuple/indexer suffix is the
/// initializer; a bare identifier initializer (`var helper = helper`) is kept
/// because only the declared-name child is skipped. Mirrors the extractor's
/// initializer rules.
fn csharp_declarator_initializer<'a>(
    declarator: tree_sitter::Node<'a>,
) -> Option<tree_sitter::Node<'a>> {
    let declared_name = declarator.child_by_field_name("name");
    let mut cursor = declarator.walk();
    let mut initializer = None;
    for child in declarator.named_children(&mut cursor) {
        if matches!(child.kind(), "tuple_pattern" | "bracketed_argument_list")
            || declared_name.is_some_and(|name| name == child)
        {
            continue;
        }
        initializer = Some(child);
    }
    initializer
}

fn csharp_insert_receiver_binding(
    bindings: &mut CSharpReceiverTypeBindings,
    name: &str,
    type_name: Option<String>,
) {
    if name.is_empty() {
        return;
    }
    let name = name.to_string();
    match &type_name {
        Some(type_name) => {
            // A single-level array spelling also records the element component
            // type so an element-access receiver such as `items[0]` can
            // dispatch on the element type; non-array spellings clear any
            // stale component so a shadowed or re-declared name never keeps an
            // outdated array binding.
            match csharp_array_type_component_name(type_name) {
                Some(component) => {
                    bindings
                        .array_component_types
                        .insert(name.clone(), component);
                    bindings.types_by_name.insert(name, type_name.clone());
                }
                None => {
                    bindings.array_component_types.remove(&name);
                    bindings.types_by_name.insert(name, type_name.clone());
                }
            }
        }
        None => {
            bindings.array_component_types.remove(&name);
            bindings.types_by_name.insert(name, String::new());
        }
    }
}

/// Records the element-access initializer base spelling, call arity, and
/// element-access depth for a `var` local bound from an element access such
/// as `var first = items[0]`, `var first = makeItems()[0]`, or
/// `var first = matrix[0][0]` (depth two). Repeated declarations overwrite
/// the recorded base so the latest binding wins, matching the other C#
/// local-binding rules.
fn insert_csharp_element_access_initializer(
    bindings: &mut CSharpReceiverTypeBindings,
    name: &str,
    base_spelling: String,
    call_arity: usize,
    depth: usize,
) {
    if name.is_empty() {
        return;
    }
    bindings
        .element_access_initializers_by_name
        .insert(name.to_string(), (base_spelling, call_arity, depth));
}

/// Extracts the element component type from an array spelling such as
/// `Helper[]`, `Helper []`, `Box<Helper>[]`, `Helper[,]`, or `Helper[,,]`,
/// keeping the raw component spelling so trace-time resolution can resolve it
/// in the receiver's own scope. A multi-dimensional array's element component
/// type is the same base type as a single-level array, so `grid[0, 0]` over
/// `Helper[,]` dispatches on `Helper`; jagged (`Helper[][]`), primitive,
/// `var`, `void`, `global::`-qualified, and malformed spellings return `None`
/// and fail closed.
pub(in crate::symbol_dependency) fn csharp_array_type_component_name(text: &str) -> Option<String> {
    let name = text.trim();
    let open = name.find('[')?;
    if open == 0 || !name[open..].trim_end().ends_with(']') {
        return None;
    }
    let bracket = name[open..].trim();
    // A single-level array is `[]`; a multi-dimensional array is `[,]`,
    // `[,,]`, and so on, whose element component type is the same base type.
    // Jagged (`[][]`) and otherwise malformed brackets still fail closed.
    let is_single = bracket == "[]";
    let is_multidimensional = bracket.starts_with("[,")
        && bracket.ends_with(']')
        && bracket[1..bracket.len() - 1]
            .chars()
            .all(|character| character == ',');
    if !is_single && !is_multidimensional {
        return None;
    }
    let component = name[..open].trim();
    if component.is_empty()
        || component.contains(['[', ']', '(', ')', ':', ','])
        || matches!(
            component,
            "bool"
                | "byte"
                | "sbyte"
                | "short"
                | "ushort"
                | "int"
                | "uint"
                | "long"
                | "ulong"
                | "nint"
                | "nuint"
                | "float"
                | "double"
                | "decimal"
                | "char"
                | "string"
                | "object"
                | "void"
                | "var"
        )
    {
        return None;
    }
    Some(component.to_string())
}

/// Strips one element-component layer from an array type spelling such as
/// `Helper[]` (`Helper`), `Helper[,]` (`Helper`), or `Helper[][]`
/// (`Helper[]`), keeping the raw remaining spelling so trace-time resolution
/// can resolve it in the declaring scope. Malformed or non-array spellings
/// return `None` and fail closed. Unlike `csharp_array_type_component_name`,
/// an intermediate jagged component (`Helper[]` from `Helper[][]`) is kept so
/// callers can strip further layers.
fn csharp_array_element_component_spelling(text: &str) -> Option<&str> {
    let name = text.trim();
    let close = name.rfind(']')?;
    let open = name[..close].rfind('[')?;
    let bracket = &name[open..=close];
    let is_single = bracket == "[]";
    let is_multidimensional = bracket.starts_with("[,")
        && bracket.ends_with(']')
        && bracket[1..bracket.len() - 1]
            .chars()
            .all(|character| character == ',');
    if !is_single && !is_multidimensional {
        return None;
    }
    let component = name[..open].trim();
    if component.is_empty() {
        return None;
    }
    Some(component)
}

/// Strips `depth` element-component layers from an array type spelling such
/// as `Helper[]` (depth one -> `Helper`), `Helper[,]` (depth one -> `Helper`),
/// or `Helper[][]` (depth two -> `Helper`), keeping the raw final spelling so
/// trace-time resolution can resolve it in the declaring scope. A depth of
/// zero returns the spelling unchanged. Primitive, `var`, `void`,
/// `global::`-qualified, malformed, and non-array spellings, and depths
/// beyond the array's layer count return `None` and fail closed.
pub(in crate::symbol_dependency) fn csharp_array_component_spelling_at_depth(
    text: &str,
    depth: usize,
) -> Option<String> {
    if depth == 0 {
        let name = text.trim();
        return if name.is_empty() {
            None
        } else {
            Some(name.to_string())
        };
    }
    let mut current = text.trim();
    for _ in 0..depth {
        current = csharp_array_element_component_spelling(current)?;
    }
    Some(current.to_string())
}

/// Returns whether an array component spelling is a C# primitive, `void`, or
/// `var`, which never names a user-defined receiver type and therefore fails
/// closed for instance dispatch.
fn csharp_array_component_spelling_is_primitive(component: &str) -> bool {
    matches!(
        component,
        "bool"
            | "byte"
            | "sbyte"
            | "short"
            | "ushort"
            | "int"
            | "uint"
            | "long"
            | "ulong"
            | "nint"
            | "nuint"
            | "float"
            | "double"
            | "decimal"
            | "char"
            | "string"
            | "object"
            | "void"
            | "var"
    )
}

/// Infers the element type of a `var` foreach variable from its collection
/// expression, such as `items` in `foreach (var item in items)`, with
/// parenthesized collections such as `(items)` unwrapped to the same shape
/// as the unparenthesized form: a bound
/// array-typed identifier or `this.`-rooted field access yields the raw
/// element component spelling (`Helper[]` -> `Helper`, `Helper[,]` ->
/// `Helper`, `Helper[][]` -> `Helper[]`), a factory-returned array collection
/// such as `makeItems()` binds a factory-element marker the resolver expands
/// to the factory return array's element component type, a member-access
/// collection such as `group.items` binds a chain-element marker the resolver
/// expands to the terminal array member's element component type, a
/// null-conditional member collection such as `group?.items` spells the same
/// chain and binds the same marker, a `var`
/// local collection bound from a factory call or field/property chain (such
/// as `var items = makeItems()` or `var items = this.fieldItems`) keeps the
/// corresponding element marker, and primitive, non-array, and unresolvable
/// collections return `None` and fail closed.
fn csharp_foreach_collection_element_type(
    right: tree_sitter::Node<'_>,
    source: &str,
    bindings: &CSharpReceiverTypeBindings,
) -> Result<Option<String>> {
    // A parenthesized collection such as `foreach (var item in (items))`
    // unwraps to the same shape as the unparenthesized form; nested or
    // malformed parentheses fail closed.
    if right.kind() == "parenthesized_expression" {
        let Some(inner) = csharp_parenthesized_inner_expression(right) else {
            return Ok(None);
        };
        return csharp_foreach_collection_element_type(inner, source, bindings);
    }
    if right.kind() == "invocation_expression" {
        // A factory-returned array collection such as
        // `foreach (var item in makeItems())` binds the loop variable to a
        // factory-element marker so the resolver dispatches on the factory
        // return array's element component type; non-array factory returns,
        // primitives, and unresolvable factories fail closed.
        let Some(marker) = csharp_factory_marker_from_initializer(right, source)? else {
            return Ok(None);
        };
        let Some(spelling) = marker.strip_prefix("@factory:") else {
            return Ok(None);
        };
        return Ok(Some(format!("@factory-element:{spelling}")));
    }
    let spelling = match right.kind() {
        "identifier" => node_text(right, source)?.trim().to_string(),
        "member_access_expression" => {
            let text = node_text(right, source)?.trim();
            if let Some(member) = text.strip_prefix("this.") {
                // A `this.`-rooted member access keeps its member spelling;
                // a single field hop resolves directly below while deeper
                // chains fall back to a chain-element marker.
                member.to_string()
            } else {
                // A bound-receiver or static-rooted member chain such as
                // `group.items` binds a chain-element marker the resolver
                // expands to the terminal array member's element component
                // type.
                return Ok(Some(format!("@chain-element:{text}")));
            }
        }
        "conditional_access_expression" => {
            // A null-conditional member collection such as `group?.items` or
            // `this.holder?.items` spells the same chain as the plain dotted
            // form so the loop variable binds a chain-element marker the
            // resolver walks to the terminal array member's element component
            // type.
            let text = node_text(right, source)?.trim().replace("?.", ".");
            if text.is_empty() {
                return Ok(None);
            }
            if let Some(member) = text.strip_prefix("this.") {
                member.to_string()
            } else {
                return Ok(Some(format!("@chain-element:{text}")));
            }
        }
        _ => return Ok(None),
    };
    if spelling.is_empty() {
        return Ok(None);
    }
    let Some(declared_type) = bindings.raw_for(&spelling) else {
        if bindings.contains(&spelling) {
            // An element-access-initializer local such as `var first =
            // items[0]` is bound to an element value, not a collection; fail
            // closed.
            return Ok(None);
        }
        // A `this.`-rooted deeper chain such as `this.holder.items` is not
        // directly bound; resolve it through the chain-element marker.
        return Ok(Some(format!("@chain-element:this.{spelling}")));
    };
    // A `var` local collection bound from a factory call such as
    // `var items = makeItems()` keeps its factory marker so the loop
    // variable dispatches on the factory return array's element component
    // type.
    if let Some(factory_call) = declared_type.strip_prefix("@factory:") {
        return Ok(Some(format!("@factory-element:{factory_call}")));
    }
    // A `var` local collection bound from a field/property-access chain
    // such as `var items = this.fieldItems` or `var items = holder.items`
    // keeps a chain-element marker the resolver walks to the terminal array
    // member's element component type; a bare-name chain such as
    // `var items = fieldItems` resolves the bound member's declared type
    // directly below.
    if let Some(chain) = declared_type.strip_prefix("@init:") {
        if chain.contains('.') {
            return Ok(Some(format!("@chain-element:{chain}")));
        }
        let Some(chain_binding) = bindings.raw_for(chain) else {
            return Ok(None);
        };
        if chain_binding.starts_with('@') {
            return Ok(None);
        }
        let Some(element_type) = csharp_array_element_component_spelling(chain_binding) else {
            return Ok(None);
        };
        if csharp_array_component_spelling_is_primitive(element_type) {
            return Ok(None);
        }
        return Ok(Some(element_type.to_string()));
    }
    if declared_type.is_empty() {
        return Ok(None);
    }
    let Some(element_type) = csharp_array_element_component_spelling(declared_type) else {
        return Ok(None);
    };
    if csharp_array_component_spelling_is_primitive(element_type) {
        return Ok(None);
    }
    Ok(Some(element_type.to_string()))
}

fn csharp_declared_type_name(node: tree_sitter::Node<'_>, source: &str) -> Result<Option<String>> {
    let Some(type_node) = node.child_by_field_name("type") else {
        return Ok(None);
    };
    let type_name = node_text(type_node, source)?.trim();
    if type_name.is_empty() || type_name == "var" {
        return Ok(None);
    }
    Ok(Some(type_name.to_string()))
}

fn is_csharp_type_declaration(node: tree_sitter::Node<'_>) -> bool {
    matches!(
        node.kind(),
        "class_declaration"
            | "struct_declaration"
            | "interface_declaration"
            | "enum_declaration"
            | "record_declaration"
    )
}

fn insert_unique_csharp_type_alias_binding(
    bindings: &mut BTreeMap<(Option<String>, String), CSharpTypeAliasBinding>,
    ambiguous_names: &mut BTreeSet<(Option<String>, String)>,
    scope_path: Option<String>,
    local_name: String,
    binding: CSharpTypeAliasBinding,
) {
    let key = (scope_path, local_name);
    if ambiguous_names.contains(&key) {
        return;
    }
    if bindings.insert(key.clone(), binding).is_some() {
        bindings.remove(&key);
        ambiguous_names.insert(key);
    }
}

pub(in crate::symbol_dependency) fn resolve_csharp_base_type_binding_for_reference(
    source_file_path: &str,
    source_type_range: (usize, usize),
    source_namespace_path: Option<&str>,
    global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(binding) = context
        .base_type_bindings_by_range
        .get(&source_type_range)
        .cloned()
    else {
        return Ok(None);
    };
    Ok(resolve_csharp_base_type_binding_parts(
        &context,
        global_import_context,
        binding,
        source_namespace_path,
    ))
}

/// Resolves the direct parent interfaces of an interface declaration as
/// base-type bindings in the interface's file scope (namespace imports and
/// aliases applied). A missing `base_list` yields `None`; a parent spelling
/// that fails to normalize or resolve in file scope yields `Blocked` so the
/// resolver fails closed instead of guessing.
pub(in crate::symbol_dependency) fn csharp_interface_parent_bindings_for_interface(
    source_file_path: &str,
    interface_range: (usize, usize),
    source_namespace_path: Option<&str>,
    global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpInterfaceParents> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    let Some(parents) = context
        .interface_parent_bindings_by_range
        .get(&interface_range)
    else {
        return Ok(CSharpInterfaceParents::None);
    };
    if parents.is_empty() {
        return Ok(CSharpInterfaceParents::Blocked);
    }
    let mut resolved_parents = Vec::with_capacity(parents.len());
    for parent in parents {
        let Some(binding) = resolve_csharp_base_type_binding_parts(
            &context,
            global_import_context,
            CSharpBaseTypeBinding {
                semantic_type_path: parent.semantic_type_path.clone(),
                is_global_qualified: parent.is_global_qualified,
                alias_name: None,
                namespace_import_paths: Vec::new(),
                generic_arguments: Vec::new(),
                raw_generic_argument_spellings: parent.raw_generic_argument_spellings.clone(),
                enclosing_generic_arguments: Vec::new(),
            },
            source_namespace_path,
        ) else {
            return Ok(CSharpInterfaceParents::Blocked);
        };
        resolved_parents.push(binding);
    }
    Ok(CSharpInterfaceParents::Parents(resolved_parents))
}

/// Resolves a declared type spelling written in a caller (such as a receiver
/// parameter or field type) to a base-type binding. Simple names resolve
/// through the caller's namespace and import scopes, aliases expand to their
/// targets, and `global::`-qualified names bind directly. Malformed or
/// alias-colliding spellings return `None`.
pub(in crate::symbol_dependency) fn resolve_csharp_declared_type_binding_for_reference(
    source_file_path: &str,
    type_name: &str,
    source_namespace_path: Option<&str>,
    global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpBaseTypeBinding>> {
    let type_name = type_name.trim();
    if type_name.is_empty() {
        return Ok(None);
    }
    let is_global_qualified = type_name.starts_with("global::");
    let Some(semantic_type_path) = csharp_generic_type_semantic_path(type_name) else {
        return Ok(None);
    };
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(resolve_csharp_base_type_binding_parts(
        &context,
        global_import_context,
        CSharpBaseTypeBinding {
            semantic_type_path,
            is_global_qualified,
            alias_name: None,
            namespace_import_paths: Vec::new(),
            generic_arguments: Vec::new(),
            raw_generic_argument_spellings: Vec::new(),
            enclosing_generic_arguments: Vec::new(),
        },
        source_namespace_path,
    ))
}

pub(in crate::symbol_dependency) fn csharp_receiver_type_bindings_for_function(
    source_file_path: &str,
    function_range: (usize, usize),
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpReceiverTypeBindings>> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(context
        .receiver_type_bindings_by_range
        .get(&function_range)
        .cloned())
}

/// Returns the direct member bindings (fields, properties, and events) of a
/// type declaration in the given source file. `None` means no type declaration
/// occupies the range, so member-chain hops fail closed.
pub(in crate::symbol_dependency) fn csharp_member_type_bindings_for_type(
    source_file_path: &str,
    type_range: (usize, usize),
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpReceiverTypeBindings>> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(context
        .member_type_bindings_by_range
        .get(&type_range)
        .cloned())
}

/// Returns the ordered type-parameter names of a type declaration in the
/// given source file. `None` means no type declaration occupies the range,
/// so generic substitution fails closed.
pub(in crate::symbol_dependency) fn csharp_type_parameter_names_for_type(
    source_file_path: &str,
    type_range: (usize, usize),
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<Vec<String>>> {
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(context
        .type_parameter_names_by_range
        .get(&type_range)
        .cloned())
}

/// Substitutes a generic type's type parameters with a constructed
/// receiver's concrete type arguments in a member declared-type spelling
/// such as `T[]` or `Dictionary<string, T>`, replacing each parameter at
/// whole-identifier boundaries with its corresponding argument. A
/// parameter/argument arity mismatch leaves the spelling unchanged and fails
/// closed downstream.
pub(in crate::symbol_dependency) fn substitute_csharp_type_parameters(
    spelling: &str,
    parameters: &[String],
    arguments: &[String],
) -> String {
    if parameters.len() != arguments.len() {
        return spelling.to_string();
    }
    let mut result = String::with_capacity(spelling.len());
    let mut rest = spelling;
    while !rest.is_empty() {
        let Some(next) =
            rest.find(|character: char| character.is_ascii_alphabetic() || character == '_')
        else {
            result.push_str(rest);
            break;
        };
        result.push_str(&rest[..next]);
        rest = &rest[next..];
        let mut end = 0usize;
        for (byte_offset, character) in rest.char_indices() {
            if character.is_ascii_alphanumeric() || character == '_' {
                end = byte_offset + character.len_utf8();
            } else {
                break;
            }
        }
        let token = &rest[..end];
        match parameters.iter().position(|parameter| parameter == token) {
            Some(position) => result.push_str(&arguments[position]),
            None => result.push_str(token),
        }
        rest = &rest[end..];
    }
    result
}

fn resolve_csharp_base_type_binding_parts(
    context: &CSharpImportContext,
    global_import_context: Option<&CSharpGlobalImportContext>,
    mut binding: CSharpBaseTypeBinding,
    source_namespace_path: Option<&str>,
) -> Option<CSharpBaseTypeBinding> {
    if !binding.is_global_qualified && binding.semantic_type_path.contains("::") {
        let first_segment = binding.semantic_type_path.split("::").next()?;
        for scope_path in csharp_import_scope_paths(source_namespace_path) {
            let key = (scope_path, first_segment.to_string());
            if context.ambiguous_type_alias_names.contains(&key)
                || context.type_alias_bindings.contains_key(&key)
            {
                return None;
            }
        }
        if let Some(global_import_context) = global_import_context
            && (csharp_global_base_type_alias_is_ambiguous(first_segment, global_import_context)
                || resolve_csharp_global_base_type_alias(first_segment, global_import_context)
                    .is_some())
        {
            return None;
        }
        // A dotted nested spelling such as `Outer.Inner` resolves its first
        // segment through the same file-scope namespace imports as simple
        // names, so `using Lib;` lets a `Outer<Helper>.Inner<Helper>` base
        // reach `Lib::Outer::Inner` instead of failing closed.
        binding.namespace_import_paths = csharp_import_scope_paths(source_namespace_path)
            .into_iter()
            .flat_map(|scope_path| {
                context
                    .namespace_import_bindings
                    .iter()
                    .filter(move |candidate| candidate.scope_path == scope_path)
                    .map(|candidate| candidate.semantic_namespace_path.clone())
            })
            .collect();
        if let Some(global_import_context) = global_import_context {
            binding
                .namespace_import_paths
                .extend(csharp_global_base_namespace_import_paths(
                    global_import_context,
                ));
        }
    } else if !binding.is_global_qualified {
        let local_name = binding.semantic_type_path.clone();
        let scope_paths = csharp_import_scope_paths(source_namespace_path);
        for scope_path in &scope_paths {
            let key = (scope_path.clone(), local_name.clone());
            if context.ambiguous_type_alias_names.contains(&key) {
                return None;
            }
            if let Some(alias) = context.type_alias_bindings.get(&key) {
                binding.semantic_type_path = alias.semantic_type_path.clone();
                binding.is_global_qualified = true;
                binding.alias_name = Some(local_name.clone());
                break;
            }
        }
        if binding.alias_name.is_none() {
            binding.namespace_import_paths = scope_paths
                .into_iter()
                .flat_map(|scope_path| {
                    context
                        .namespace_import_bindings
                        .iter()
                        .filter(move |candidate| candidate.scope_path == scope_path)
                        .map(|candidate| candidate.semantic_namespace_path.clone())
                })
                .collect();
            if let Some(global_import_context) = global_import_context {
                if csharp_global_base_type_alias_is_ambiguous(&local_name, global_import_context) {
                    return None;
                }
                if let Some(alias) =
                    resolve_csharp_global_base_type_alias(&local_name, global_import_context)
                {
                    binding.semantic_type_path = alias.semantic_type_path;
                    binding.is_global_qualified = true;
                    binding.alias_name = Some(local_name.clone());
                    binding.namespace_import_paths.clear();
                } else {
                    binding.namespace_import_paths.extend(
                        csharp_global_base_namespace_import_paths(global_import_context),
                    );
                }
            }
        }
    }
    Some(binding)
}

pub(in crate::symbol_dependency) fn resolve_csharp_type_alias_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, CSharpTypeAliasBinding)>> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(None);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    for scope_path in csharp_import_scope_paths(source_namespace_path) {
        let key = (scope_path, local_type_name.to_string());
        if context.ambiguous_type_alias_names.contains(&key) {
            return Ok(None);
        }
        if let Some(binding) = context.type_alias_bindings.get(&key) {
            return Ok(Some((method_name.to_string(), binding.clone())));
        }
    }
    Ok(None)
}

pub(in crate::symbol_dependency) fn resolve_csharp_type_alias_binding_for_name(
    source_file_path: &str,
    local_type_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<CSharpTypeAliasBinding>> {
    if local_type_name.is_empty() || local_type_name.contains('.') {
        return Ok(None);
    }
    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    for scope_path in csharp_import_scope_paths(source_namespace_path) {
        let key = (scope_path, local_type_name.to_string());
        if context.ambiguous_type_alias_names.contains(&key) {
            return Ok(None);
        }
        if let Some(binding) = context.type_alias_bindings.get(&key) {
            return Ok(Some(binding.clone()));
        }
    }
    Ok(None)
}

pub(in crate::symbol_dependency) fn resolve_csharp_nested_type_alias_binding_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Option<(String, String, CSharpTypeAliasBinding)>> {
    let Some((local_type_name, nested_reference)) = reference_name.split_once('.') else {
        return Ok(None);
    };
    let Some((nested_type_path, method_name)) = nested_reference.rsplit_once('.') else {
        return Ok(None);
    };
    if local_type_name.is_empty()
        || nested_type_path.is_empty()
        || method_name.is_empty()
        || nested_type_path
            .split('.')
            .any(|segment| !is_safe_csharp_identifier(segment))
    {
        return Ok(None);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    for scope_path in csharp_import_scope_paths(source_namespace_path) {
        let key = (scope_path, local_type_name.to_string());
        if context.ambiguous_type_alias_names.contains(&key) {
            return Ok(None);
        }
        if let Some(binding) = context.type_alias_bindings.get(&key) {
            return Ok(Some((
                nested_type_path.replace('.', "::"),
                method_name.to_string(),
                binding.clone(),
            )));
        }
    }
    Ok(None)
}

pub(in crate::symbol_dependency) fn csharp_type_alias_name_is_declared_for_reference(
    source_file_path: &str,
    local_type_name: &str,
    source_namespace_path: Option<&str>,
    csharp_global_import_context: Option<&CSharpGlobalImportContext>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<bool> {
    if local_type_name.is_empty() {
        return Ok(false);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    if csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .map(|scope_path| (scope_path, local_type_name.to_string()))
        .any(|key| {
            context.type_alias_bindings.contains_key(&key)
                || context.ambiguous_type_alias_names.contains(&key)
        })
    {
        return Ok(true);
    }

    Ok(csharp_global_import_context.is_some_and(|context| {
        let key = (None, local_type_name.to_string());
        context.type_alias_bindings.contains_key(&key)
            || context.ambiguous_type_alias_names.contains(&key)
    }))
}

pub(in crate::symbol_dependency) fn csharp_type_alias_name_is_ambiguous_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<bool> {
    let Some((local_type_name, method_name)) = reference_name.split_once('.') else {
        return Ok(false);
    };
    if local_type_name.is_empty() || method_name.is_empty() || method_name.contains('.') {
        return Ok(false);
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .map(|scope_path| (scope_path, local_type_name.to_string()))
        .any(|key| context.ambiguous_type_alias_names.contains(&key)))
}

fn is_safe_csharp_identifier(identifier: &str) -> bool {
    let mut characters = identifier.chars();
    matches!(characters.next(), Some(character) if character == '_' || character.is_ascii_alphabetic())
        && characters.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn csharp_import_scope_paths(source_namespace_path: Option<&str>) -> Vec<Option<String>> {
    let mut scope_paths = Vec::new();
    let mut current_scope_path = source_namespace_path;
    while let Some(scope_path) = current_scope_path {
        scope_paths.push(Some(scope_path.to_string()));
        current_scope_path = scope_path
            .rsplit_once("::")
            .map(|(parent_path, _)| parent_path);
    }
    scope_paths.push(None);
    scope_paths
}

pub(in crate::symbol_dependency) fn resolve_csharp_static_type_imports_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<CSharpStaticTypeImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(Vec::new());
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .flat_map(|scope_path| {
            context
                .static_type_import_bindings
                .iter()
                .filter(move |binding| binding.scope_path == scope_path)
                .cloned()
        })
        .collect())
}

pub(in crate::symbol_dependency) fn resolve_csharp_namespace_imports_for_reference(
    source_file_path: &str,
    reference_name: &str,
    source_namespace_path: Option<&str>,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<Vec<CSharpNamespaceImportBinding>> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Ok(Vec::new());
    }

    let context = csharp_import_context_from_cache(
        source_file_path,
        file_overrides,
        contexts_by_file,
        deadline,
    )?;
    Ok(csharp_import_scope_paths(source_namespace_path)
        .into_iter()
        .flat_map(|scope_path| {
            context
                .namespace_import_bindings
                .iter()
                .filter(move |binding| binding.scope_path == scope_path)
                .cloned()
        })
        .collect())
}

pub(in crate::symbol_dependency) fn resolve_csharp_global_namespace_imports_for_reference(
    reference_name: &str,
    context: &CSharpGlobalImportContext,
) -> Vec<CSharpNamespaceImportBinding> {
    if reference_name.is_empty() || reference_name.contains('.') {
        return Vec::new();
    }
    context.namespace_import_bindings.clone()
}

fn csharp_import_context_from_cache(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    contexts_by_file: &mut BTreeMap<String, CSharpImportContext>,
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<CSharpImportContext> {
    let normalized_file_path = normalize_path(Path::new(file_path));
    if let Some(context) = contexts_by_file.get(&normalized_file_path) {
        return Ok(context.clone());
    }

    let context = csharp_import_context_for_file_with_overrides_and_deadline(
        &normalized_file_path,
        file_overrides,
        deadline,
    )?;
    contexts_by_file.insert(normalized_file_path, context.clone());
    Ok(context)
}
