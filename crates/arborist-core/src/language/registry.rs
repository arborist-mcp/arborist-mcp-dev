use std::collections::BTreeMap;
use std::ops::{BitOr, BitOrAssign};
use std::path::Path;
use std::sync::OnceLock;

use anyhow::{Result, anyhow};
use tree_sitter::{Language, Node, Tree};

use super::{C_LANGUAGE_EXTENSIONS, CPP_LANGUAGE_EXTENSIONS, ParsedDocument};
use crate::deadline::DeadlineCheck;
use crate::model::{LanguageId, SemanticSkeleton};
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

const PYTHON_EXTENSIONS: &[&str] = &["py", "pyi"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageCapabilities(u32);

impl LanguageCapabilities {
    pub const TREE_QUERY: Self = Self(1 << 0);
    pub const SEMANTIC_SKELETON: Self = Self(1 << 1);
    pub const SYMBOL_INDEX: Self = Self(1 << 2);
    pub const FILE_DEPENDENCIES: Self = Self(1 << 3);
    pub const REFERENCE_TRACE: Self = Self(1 << 4);
    pub const PATCH_TARGETING: Self = Self(1 << 5);
    pub const PATCH_VALIDATION: Self = Self(1 << 6);

    pub const FULL_CURRENT_SUPPORT: Self = Self(
        Self::TREE_QUERY.0
            | Self::SEMANTIC_SKELETON.0
            | Self::SYMBOL_INDEX.0
            | Self::FILE_DEPENDENCIES.0
            | Self::REFERENCE_TRACE.0
            | Self::PATCH_TARGETING.0
            | Self::PATCH_VALIDATION.0,
    );

    pub const fn contains(self, required: Self) -> bool {
        self.0 & required.0 == required.0
    }
}

impl BitOr for LanguageCapabilities {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for LanguageCapabilities {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

#[derive(Debug)]
pub struct LanguageDescriptor {
    pub id: LanguageId,
    pub display_name: &'static str,
    pub extensions: &'static [&'static str],
    pub capabilities: LanguageCapabilities,
    pub analysis_revision: &'static str,
    grammar: fn() -> Language,
}

impl LanguageDescriptor {
    pub fn tree_sitter_language(&self) -> Language {
        (self.grammar)()
    }
}

pub(crate) struct PositionSymbolIdentity {
    pub(crate) symbol_id: String,
    pub(crate) semantic_path: String,
    pub(crate) byte_range: (usize, usize),
}

pub(crate) trait LanguageAdapter: Sync {
    fn descriptor(&self) -> &'static LanguageDescriptor;

    fn build_semantic_skeleton(
        &self,
        path: &Path,
        source: &str,
        tree: &Tree,
        depth_limit: usize,
        expand_nodes: &[String],
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<SemanticSkeleton>;

    fn find_semantic_node<'tree>(
        &self,
        path: &Path,
        tree: &'tree Tree,
        source: &str,
        target_path: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<Node<'tree>>>;

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>>;

    fn position_symbol_identity(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity>;

    fn semantic_path_for_node(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>>;

    fn symbol_id_for_node(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>>;

    fn query_owner_candidates<'tree>(
        &self,
        path: &Path,
        root: Node<'tree>,
        source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>>;

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree>;

    fn normalize_patch_replacement(
        &self,
        source: &str,
        start_byte: usize,
        end_byte: usize,
        node_kind: &str,
        new_code: &str,
    ) -> Result<String>;

    fn replacement_preserves_required_wrappers(&self, node_kind: &str, replacement: &str) -> bool;

    fn reconcile_patch_symbol_id(
        &self,
        semantic_target: &str,
        resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String;

    fn collect_patch_reference_validation(
        &self,
        path: &Path,
        document: &ParsedDocument,
        source: &str,
        symbol_node: Node<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation>;

    fn query_capture_owner(
        &self,
        path: &Path,
        source: &str,
        node: Node<'_>,
        candidates: Option<&[Node<'_>]>,
    ) -> Result<(Option<String>, Option<String>, Option<String>)>;

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>>;
}

pub struct LanguageRegistry {
    adapters: BTreeMap<LanguageId, &'static dyn LanguageAdapter>,
    extensions: BTreeMap<&'static str, LanguageId>,
}

impl LanguageRegistry {
    fn builtin() -> Self {
        let adapters: [&'static dyn LanguageAdapter; 3] =
            [&PYTHON_ADAPTER, &C_ADAPTER, &CPP_ADAPTER];
        Self::new(adapters)
    }

    fn new(adapters: impl IntoIterator<Item = &'static dyn LanguageAdapter>) -> Self {
        let mut adapters_by_id = BTreeMap::new();
        let mut language_by_extension = BTreeMap::new();

        for adapter in adapters {
            let descriptor = adapter.descriptor();
            assert!(
                adapters_by_id.insert(descriptor.id, adapter).is_none(),
                "duplicate builtin language adapter for {:?}",
                descriptor.id,
            );
            for extension in descriptor.extensions {
                assert!(
                    language_by_extension
                        .insert(*extension, descriptor.id)
                        .is_none(),
                    "duplicate builtin language extension {extension}",
                );
            }
        }

        Self {
            adapters: adapters_by_id,
            extensions: language_by_extension,
        }
    }

    pub fn descriptor(&self, language_id: LanguageId) -> Option<&'static LanguageDescriptor> {
        self.adapter(language_id).map(LanguageAdapter::descriptor)
    }

    pub(crate) fn adapter(&self, language_id: LanguageId) -> Option<&'static dyn LanguageAdapter> {
        self.adapters.get(&language_id).copied()
    }

    pub fn language_for_extension(&self, extension: &str) -> Option<LanguageId> {
        let extension = extension.to_ascii_lowercase();
        self.extensions.get(extension.as_str()).copied()
    }

    pub fn supported_language_names(&self) -> Vec<&'static str> {
        self.adapters
            .keys()
            .map(|language_id| match language_id {
                LanguageId::Python => "python",
                LanguageId::C => "c",
                LanguageId::Cpp => "cpp",
            })
            .collect()
    }
}

pub fn builtin_language_registry() -> &'static LanguageRegistry {
    static REGISTRY: OnceLock<LanguageRegistry> = OnceLock::new();
    REGISTRY.get_or_init(LanguageRegistry::builtin)
}

struct PythonAdapter;
struct CAdapter;
struct CppAdapter;

static PYTHON_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::Python,
    display_name: "Python",
    extensions: PYTHON_EXTENSIONS,
    capabilities: LanguageCapabilities::FULL_CURRENT_SUPPORT,
    analysis_revision: "python-v1",
    grammar: python_grammar,
};
static C_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::C,
    display_name: "C",
    extensions: C_LANGUAGE_EXTENSIONS,
    capabilities: LanguageCapabilities::FULL_CURRENT_SUPPORT,
    analysis_revision: "c-v1",
    grammar: c_grammar,
};
static CPP_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::Cpp,
    display_name: "C++",
    extensions: CPP_LANGUAGE_EXTENSIONS,
    capabilities: LanguageCapabilities::FULL_CURRENT_SUPPORT,
    analysis_revision: "cpp-v1",
    grammar: cpp_grammar,
};

static PYTHON_ADAPTER: PythonAdapter = PythonAdapter;
static C_ADAPTER: CAdapter = CAdapter;
static CPP_ADAPTER: CppAdapter = CppAdapter;

impl LanguageAdapter for PythonAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        &PYTHON_DESCRIPTOR
    }

    fn build_semantic_skeleton(
        &self,
        path: &Path,
        source: &str,
        tree: &Tree,
        depth_limit: usize,
        expand_nodes: &[String],
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<SemanticSkeleton> {
        crate::semantic::python::build_python_skeleton(
            path,
            source,
            tree,
            depth_limit,
            expand_nodes,
            deadline,
        )
    }

    fn find_semantic_node<'tree>(
        &self,
        path: &Path,
        tree: &'tree Tree,
        source: &str,
        target_path: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<Node<'tree>>> {
        crate::semantic::python::find_python_semantic_node(
            path,
            tree,
            source,
            target_path,
            deadline,
        )
    }

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        crate::semantic::ascend_python_to_symbol(node)
    }

    fn position_symbol_identity(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity> {
        let semantic_path = crate::semantic::semantic_path(node, source)?;
        Ok(PositionSymbolIdentity {
            symbol_id: semantic_path.clone(),
            semantic_path,
            byte_range: crate::semantic::python_display_byte_range(node),
        })
    }

    fn semantic_path_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>> {
        crate::semantic::semantic_path(node, source).map(Some)
    }

    fn symbol_id_for_node(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        crate::semantic::python_symbol_id_for_node(path, node, source, deadline).map(Some)
    }

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        node.parent()
            .filter(|parent| parent.kind() == "decorated_definition")
            .unwrap_or(node)
    }

    fn normalize_patch_replacement(
        &self,
        source: &str,
        start_byte: usize,
        end_byte: usize,
        node_kind: &str,
        new_code: &str,
    ) -> Result<String> {
        Ok(
            crate::patching::python_replacement::normalize_python_replacement_indentation(
                source,
                start_byte,
                end_byte,
                node_kind == "decorated_definition",
                new_code,
            ),
        )
    }

    fn replacement_preserves_required_wrappers(&self, node_kind: &str, replacement: &str) -> bool {
        node_kind != "decorated_definition"
            || crate::patching::python_replacement::python_replacement_starts_with_decorator(
                replacement,
            )
    }

    fn reconcile_patch_symbol_id(
        &self,
        semantic_target: &str,
        resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String {
        if resolved_symbol_id == resolved_path
            && semantic_target.ends_with(&format!("::{resolved_path}"))
        {
            semantic_target.to_string()
        } else {
            resolved_symbol_id
        }
    }

    fn query_owner_candidates<'tree>(
        &self,
        _path: &Path,
        _root: Node<'tree>,
        _source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        Ok(None)
    }

    fn collect_patch_reference_validation(
        &self,
        path: &Path,
        _document: &ParsedDocument,
        source: &str,
        symbol_node: Node<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        crate::patching::python_references::collect_python_reference_validation_with_deadline(
            path,
            source,
            symbol_node,
            deadline,
        )
    }

    fn query_capture_owner(
        &self,
        _path: &Path,
        source: &str,
        node: Node<'_>,
        _candidates: Option<&[Node<'_>]>,
    ) -> Result<(Option<String>, Option<String>, Option<String>)> {
        crate::query::owners::python_capture_owner(source, node)
    }

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        crate::symbol_extractor::python::index_python_symbols_with_deadline(
            path,
            source,
            document.tree.root_node(),
            deadline,
        )
    }
}

impl LanguageAdapter for CAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        &C_DESCRIPTOR
    }

    fn build_semantic_skeleton(
        &self,
        path: &Path,
        source: &str,
        tree: &Tree,
        _depth_limit: usize,
        expand_nodes: &[String],
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<SemanticSkeleton> {
        crate::semantic::c::build_c_skeleton(path, source, tree, expand_nodes, deadline)
    }

    fn find_semantic_node<'tree>(
        &self,
        path: &Path,
        tree: &'tree Tree,
        source: &str,
        target_path: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<Node<'tree>>> {
        crate::semantic::c::find_c_semantic_node(path, tree, source, target_path, deadline)
    }

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        crate::semantic::ascend_c_to_symbol(node)
    }

    fn position_symbol_identity(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity> {
        let semantic_path = crate::semantic::c_semantic_path(path, node, source)?
            .ok_or_else(|| anyhow!("position does not resolve to a C semantic symbol"))?;
        let symbol_id = crate::semantic::c_symbol_id_for_node(path, node, source)?
            .ok_or_else(|| anyhow!("position does not resolve to a C symbol id"))?;
        Ok(PositionSymbolIdentity {
            symbol_id,
            semantic_path,
            byte_range: (node.start_byte(), node.end_byte()),
        })
    }

    fn semantic_path_for_node(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>> {
        crate::semantic::c_semantic_path(path, node, source)
    }

    fn symbol_id_for_node(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        crate::semantic::c_symbol_id_for_node(path, node, source)
    }

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        node
    }

    fn normalize_patch_replacement(
        &self,
        _source: &str,
        _start_byte: usize,
        _end_byte: usize,
        _node_kind: &str,
        new_code: &str,
    ) -> Result<String> {
        Ok(new_code.to_string())
    }

    fn replacement_preserves_required_wrappers(
        &self,
        _node_kind: &str,
        _replacement: &str,
    ) -> bool {
        true
    }

    fn reconcile_patch_symbol_id(
        &self,
        _semantic_target: &str,
        _resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String {
        resolved_symbol_id
    }

    fn query_owner_candidates<'tree>(
        &self,
        path: &Path,
        root: Node<'tree>,
        source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        crate::semantic::c_symbol_nodes(path, root, source).map(Some)
    }

    fn collect_patch_reference_validation(
        &self,
        path: &Path,
        document: &ParsedDocument,
        source: &str,
        symbol_node: Node<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        crate::patching::c_validation::collect_c_reference_validation_with_deadline(
            path,
            document,
            source,
            symbol_node,
            deadline,
        )
    }

    fn query_capture_owner(
        &self,
        path: &Path,
        source: &str,
        node: Node<'_>,
        candidates: Option<&[Node<'_>]>,
    ) -> Result<(Option<String>, Option<String>, Option<String>)> {
        crate::query::owners::c_capture_owner(path, source, node, candidates.unwrap_or_default())
    }

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        crate::symbol_extractor::c::index_c_symbols_with_deadline(
            path,
            source,
            document.tree.root_node(),
            false,
            deadline,
        )
    }
}

impl LanguageAdapter for CppAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        &CPP_DESCRIPTOR
    }

    fn build_semantic_skeleton(
        &self,
        path: &Path,
        source: &str,
        tree: &Tree,
        depth_limit: usize,
        expand_nodes: &[String],
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<SemanticSkeleton> {
        C_ADAPTER.build_semantic_skeleton(path, source, tree, depth_limit, expand_nodes, deadline)
    }

    fn find_semantic_node<'tree>(
        &self,
        path: &Path,
        tree: &'tree Tree,
        source: &str,
        target_path: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<Node<'tree>>> {
        C_ADAPTER.find_semantic_node(path, tree, source, target_path, deadline)
    }

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        C_ADAPTER.ascend_to_symbol(node)
    }

    fn position_symbol_identity(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity> {
        C_ADAPTER.position_symbol_identity(path, node, source)
    }

    fn semantic_path_for_node(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>> {
        C_ADAPTER.semantic_path_for_node(path, node, source)
    }

    fn symbol_id_for_node(
        &self,
        path: &Path,
        node: Node<'_>,
        source: &str,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        C_ADAPTER.symbol_id_for_node(path, node, source, deadline)
    }

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        C_ADAPTER.patch_replacement_node(node)
    }

    fn normalize_patch_replacement(
        &self,
        source: &str,
        start_byte: usize,
        end_byte: usize,
        node_kind: &str,
        new_code: &str,
    ) -> Result<String> {
        C_ADAPTER.normalize_patch_replacement(source, start_byte, end_byte, node_kind, new_code)
    }

    fn replacement_preserves_required_wrappers(&self, node_kind: &str, replacement: &str) -> bool {
        C_ADAPTER.replacement_preserves_required_wrappers(node_kind, replacement)
    }

    fn reconcile_patch_symbol_id(
        &self,
        semantic_target: &str,
        resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String {
        C_ADAPTER.reconcile_patch_symbol_id(semantic_target, resolved_path, resolved_symbol_id)
    }

    fn query_owner_candidates<'tree>(
        &self,
        path: &Path,
        root: Node<'tree>,
        source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        C_ADAPTER.query_owner_candidates(path, root, source)
    }

    fn collect_patch_reference_validation(
        &self,
        path: &Path,
        document: &ParsedDocument,
        source: &str,
        symbol_node: Node<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        C_ADAPTER.collect_patch_reference_validation(path, document, source, symbol_node, deadline)
    }

    fn query_capture_owner(
        &self,
        path: &Path,
        source: &str,
        node: Node<'_>,
        candidates: Option<&[Node<'_>]>,
    ) -> Result<(Option<String>, Option<String>, Option<String>)> {
        C_ADAPTER.query_capture_owner(path, source, node, candidates)
    }

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        crate::symbol_extractor::c::index_c_symbols_with_deadline(
            path,
            source,
            document.tree.root_node(),
            true,
            deadline,
        )
    }
}

fn python_grammar() -> Language {
    tree_sitter_python::LANGUAGE.into()
}

fn c_grammar() -> Language {
    tree_sitter_c::LANGUAGE.into()
}

fn cpp_grammar() -> Language {
    tree_sitter_cpp::LANGUAGE.into()
}
