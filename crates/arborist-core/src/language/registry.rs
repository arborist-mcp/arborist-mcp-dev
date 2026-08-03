use std::collections::BTreeMap;
use std::ops::{BitOr, BitOrAssign};
use std::path::Path;
use std::sync::OnceLock;

use anyhow::Result;
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
