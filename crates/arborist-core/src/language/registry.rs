use std::collections::BTreeMap;
use std::ops::{BitOr, BitOrAssign};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{Result, anyhow, bail};
use tree_sitter::{Language, Node, Tree};

use super::{C_LANGUAGE_EXTENSIONS, CPP_LANGUAGE_EXTENSIONS, ParsedDocument};
use crate::deadline::DeadlineCheck;
use crate::model::{LanguageId, SemanticSkeleton};
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

const PYTHON_EXTENSIONS: &[&str] = &["py", "pyi"];
const JAVASCRIPT_EXTENSIONS: &[&str] = &["js", "jsx", "mjs", "cjs"];
const TYPESCRIPT_EXTENSIONS: &[&str] = &["ts", "mts", "cts"];
const TSX_EXTENSIONS: &[&str] = &["tsx"];
const RUST_EXTENSIONS: &[&str] = &["rs"];
const GO_EXTENSIONS: &[&str] = &["go"];
const JAVA_EXTENSIONS: &[&str] = &["java"];
const CSHARP_EXTENSIONS: &[&str] = &["cs"];

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

    pub const INDEXED_SKELETON_SUPPORT: Self =
        Self(Self::TREE_QUERY.0 | Self::SEMANTIC_SKELETON.0 | Self::SYMBOL_INDEX.0);
    pub const INDEXED_SKELETON_DEPENDENCY_SUPPORT: Self =
        Self(Self::INDEXED_SKELETON_SUPPORT.0 | Self::FILE_DEPENDENCIES.0);
    pub const INDEXED_TRACE_SUPPORT: Self =
        Self(Self::TREE_QUERY.0 | Self::SYMBOL_INDEX.0 | Self::REFERENCE_TRACE.0);
    pub const INDEXED_SKELETON_TRACE_SUPPORT: Self =
        Self(Self::INDEXED_TRACE_SUPPORT.0 | Self::SEMANTIC_SKELETON.0);
    pub const INDEXED_SKELETON_DEPENDENCY_TRACE_SUPPORT: Self =
        Self(Self::INDEXED_SKELETON_TRACE_SUPPORT.0 | Self::FILE_DEPENDENCIES.0);
    pub const PATCHABLE_INDEXED_SKELETON_DEPENDENCY_TRACE_SUPPORT: Self = Self(
        Self::INDEXED_SKELETON_DEPENDENCY_TRACE_SUPPORT.0
            | Self::PATCH_TARGETING.0
            | Self::PATCH_VALIDATION.0,
    );

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

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool;

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

    fn supports_incremental_file_dependencies(&self) -> bool;

    fn collect_local_file_dependencies(
        &self,
        path: &Path,
        root: Node<'_>,
        source: &str,
    ) -> Result<Vec<PathBuf>>;

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
        let adapters: [&'static dyn LanguageAdapter; 10] = [
            &PYTHON_ADAPTER,
            &C_ADAPTER,
            &CPP_ADAPTER,
            &JAVASCRIPT_ADAPTER,
            &TYPESCRIPT_ADAPTER,
            &TSX_ADAPTER,
            &RUST_ADAPTER,
            &GO_ADAPTER,
            &JAVA_ADAPTER,
            &CSHARP_ADAPTER,
        ];
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

    pub(crate) fn supports_capability(
        &self,
        language_id: LanguageId,
        required: LanguageCapabilities,
    ) -> bool {
        self.descriptor(language_id)
            .is_some_and(|descriptor| descriptor.capabilities.contains(required))
    }

    pub(crate) fn require_capability(
        &self,
        language_id: LanguageId,
        required: LanguageCapabilities,
        operation: &str,
    ) -> Result<()> {
        let descriptor = self
            .descriptor(language_id)
            .ok_or_else(|| anyhow!("missing builtin language descriptor for {language_id:?}"))?;
        if descriptor.capabilities.contains(required) {
            return Ok(());
        }
        bail!(
            "{} does not support {} for {}",
            descriptor.display_name,
            capability_name(required),
            operation
        )
    }

    pub fn language_for_extension(&self, extension: &str) -> Option<LanguageId> {
        let extension = extension.to_ascii_lowercase();
        self.extensions.get(extension.as_str()).copied()
    }

    pub fn supported_language_names(&self) -> Vec<&'static str> {
        self.adapters
            .keys()
            .map(|language_id| persisted_language_id(*language_id))
            .collect()
    }

    pub(crate) fn analysis_provenance(&self) -> (Vec<String>, BTreeMap<String, String>, String) {
        let mut language_ids = Vec::new();
        let mut analysis_revisions = BTreeMap::new();
        let mut detection_entries = Vec::new();

        for (language_id, adapter) in &self.adapters {
            let descriptor = adapter.descriptor();
            let language_id = persisted_language_id(*language_id).to_string();
            let mut extensions = descriptor
                .extensions
                .iter()
                .map(|extension| (*extension).to_string())
                .collect::<Vec<_>>();
            extensions.sort();
            language_ids.push(language_id.clone());
            analysis_revisions.insert(
                language_id.clone(),
                descriptor.analysis_revision.to_string(),
            );
            detection_entries.push(format!("{language_id}:{}", extensions.join(",")));
        }

        (
            language_ids,
            analysis_revisions,
            format!(
                "builtin-extension-routing-v1;{}",
                detection_entries.join(";")
            ),
        )
    }
}

fn persisted_language_id(language_id: LanguageId) -> &'static str {
    match language_id {
        LanguageId::Python => "python",
        LanguageId::C => "c",
        LanguageId::Cpp => "cpp",
        LanguageId::CSharp => "csharp",
        LanguageId::JavaScript => "javascript",
        LanguageId::TypeScript => "typescript",
        LanguageId::Tsx => "tsx",
        LanguageId::Rust => "rust",
        LanguageId::Go => "go",
        LanguageId::Java => "java",
    }
}

fn capability_name(capability: LanguageCapabilities) -> &'static str {
    match capability {
        LanguageCapabilities::TREE_QUERY => "Tree-sitter queries",
        LanguageCapabilities::SEMANTIC_SKELETON => "semantic skeletons",
        LanguageCapabilities::SYMBOL_INDEX => "symbol indexing",
        LanguageCapabilities::FILE_DEPENDENCIES => "file dependencies",
        LanguageCapabilities::REFERENCE_TRACE => "reference tracing",
        LanguageCapabilities::PATCH_TARGETING => "patch targeting",
        LanguageCapabilities::PATCH_VALIDATION => "patch validation",
        _ => "the requested operation",
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
static JAVASCRIPT_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::JavaScript,
    display_name: "JavaScript",
    extensions: JAVASCRIPT_EXTENSIONS,
    capabilities: LanguageCapabilities::PATCHABLE_INDEXED_SKELETON_DEPENDENCY_TRACE_SUPPORT,
    analysis_revision: "javascript-patching-v1",
    grammar: javascript_grammar,
};
static TYPESCRIPT_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::TypeScript,
    display_name: "TypeScript",
    extensions: TYPESCRIPT_EXTENSIONS,
    capabilities: LanguageCapabilities::PATCHABLE_INDEXED_SKELETON_DEPENDENCY_TRACE_SUPPORT,
    analysis_revision: "typescript-patching-v1",
    grammar: typescript_grammar,
};
static TSX_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::Tsx,
    display_name: "TSX",
    extensions: TSX_EXTENSIONS,
    capabilities: LanguageCapabilities::PATCHABLE_INDEXED_SKELETON_DEPENDENCY_TRACE_SUPPORT,
    analysis_revision: "tsx-patching-v1",
    grammar: tsx_grammar,
};
static RUST_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::Rust,
    display_name: "Rust",
    extensions: RUST_EXTENSIONS,
    capabilities: LanguageCapabilities::INDEXED_SKELETON_DEPENDENCY_TRACE_SUPPORT,
    analysis_revision: "rust-parent-qualified-call-trace-v10",
    grammar: rust_grammar,
};
static GO_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::Go,
    display_name: "Go",
    extensions: GO_EXTENSIONS,
    capabilities: LanguageCapabilities::INDEXED_SKELETON_DEPENDENCY_TRACE_SUPPORT,
    analysis_revision: "go-alias-conversion-method-trace-v15",
    grammar: go_grammar,
};
static JAVA_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::Java,
    display_name: "Java",
    extensions: JAVA_EXTENSIONS,
    capabilities: LanguageCapabilities(
        LanguageCapabilities::TREE_QUERY.0
            | LanguageCapabilities::SEMANTIC_SKELETON.0
            | LanguageCapabilities::SYMBOL_INDEX.0
            | LanguageCapabilities::FILE_DEPENDENCIES.0
            | LanguageCapabilities::REFERENCE_TRACE.0,
    ),
    analysis_revision: "java-default-interface-superclass-trace-v22",
    grammar: java_grammar,
};
static CSHARP_DESCRIPTOR: LanguageDescriptor = LanguageDescriptor {
    id: LanguageId::CSharp,
    display_name: "C#",
    extensions: CSHARP_EXTENSIONS,
    capabilities: LanguageCapabilities(
        LanguageCapabilities::TREE_QUERY.0
            | LanguageCapabilities::SEMANTIC_SKELETON.0
            | LanguageCapabilities::SYMBOL_INDEX.0
            | LanguageCapabilities::REFERENCE_TRACE.0,
    ),
    analysis_revision: "csharp-generic-base-trace-v27",
    grammar: csharp_grammar,
};

static PYTHON_ADAPTER: PythonAdapter = PythonAdapter;
static C_ADAPTER: CAdapter = CAdapter;
static CPP_ADAPTER: CppAdapter = CppAdapter;
static JAVASCRIPT_ADAPTER: JavaScriptFamilyAdapter = JavaScriptFamilyAdapter {
    descriptor: &JAVASCRIPT_DESCRIPTOR,
};
static TYPESCRIPT_ADAPTER: JavaScriptFamilyAdapter = JavaScriptFamilyAdapter {
    descriptor: &TYPESCRIPT_DESCRIPTOR,
};
static TSX_ADAPTER: JavaScriptFamilyAdapter = JavaScriptFamilyAdapter {
    descriptor: &TSX_DESCRIPTOR,
};
static RUST_ADAPTER: RustAdapter = RustAdapter {
    syntax: SyntaxOnlyAdapter {
        descriptor: &RUST_DESCRIPTOR,
    },
};
static GO_ADAPTER: GoAdapter = GoAdapter {
    syntax: SyntaxOnlyAdapter {
        descriptor: &GO_DESCRIPTOR,
    },
};
static JAVA_ADAPTER: JavaAdapter = JavaAdapter {
    syntax: SyntaxOnlyAdapter {
        descriptor: &JAVA_DESCRIPTOR,
    },
};
static CSHARP_ADAPTER: CSharpAdapter = CSharpAdapter {
    syntax: SyntaxOnlyAdapter {
        descriptor: &CSHARP_DESCRIPTOR,
    },
};

struct JavaScriptFamilyAdapter {
    descriptor: &'static LanguageDescriptor,
}

impl LanguageAdapter for JavaScriptFamilyAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        self.descriptor
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
        crate::semantic::javascript::build_javascript_skeleton(
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
        crate::semantic::javascript::find_javascript_semantic_node(
            path,
            tree,
            source,
            target_path,
            deadline,
        )
    }

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        crate::semantic::ascend_javascript_to_symbol(node)
    }

    fn position_symbol_identity(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity> {
        let semantic_path = javascript_semantic_path_for_node(node, source)?.ok_or_else(|| {
            anyhow!("position does not resolve to a JavaScript/TypeScript symbol")
        })?;
        Ok(PositionSymbolIdentity {
            symbol_id: semantic_path.clone(),
            semantic_path,
            byte_range: (node.start_byte(), node.end_byte()),
        })
    }

    fn semantic_path_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>> {
        javascript_semantic_path_for_node(node, source)
    }

    fn symbol_id_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        javascript_semantic_path_for_node(node, source)
    }

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        false
    }

    fn query_owner_candidates<'tree>(
        &self,
        _path: &Path,
        _root: Node<'tree>,
        _source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        Ok(None)
    }

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        crate::semantic::javascript::javascript_patch_replacement_node(node)
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

    fn collect_patch_reference_validation(
        &self,
        _path: &Path,
        _document: &ParsedDocument,
        _source: &str,
        _symbol_node: Node<'_>,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        Ok(crate::patching::ReferenceValidation::default())
    }

    fn query_capture_owner(
        &self,
        _path: &Path,
        _source: &str,
        _node: Node<'_>,
        _candidates: Option<&[Node<'_>]>,
    ) -> Result<(Option<String>, Option<String>, Option<String>)> {
        Ok((None, None, None))
    }

    fn supports_incremental_file_dependencies(&self) -> bool {
        true
    }

    fn collect_local_file_dependencies(
        &self,
        path: &Path,
        root: Node<'_>,
        source: &str,
    ) -> Result<Vec<PathBuf>> {
        crate::language::javascript_local_module_dependency_paths(path, root, source)
            .map(|paths| paths.into_iter().collect())
    }

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        crate::symbol_extractor::javascript::index_javascript_symbols_with_deadline(
            path,
            source,
            document.tree.root_node(),
            deadline,
        )
    }
}

fn javascript_semantic_path_for_node(node: Node<'_>, source: &str) -> Result<Option<String>> {
    crate::semantic::javascript::javascript_symbol_name(node, source)?
        .map(|name| crate::semantic::javascript::javascript_semantic_path(node, source, &name))
        .transpose()
}

fn rust_semantic_path_for_node(node: Node<'_>, source: &str) -> Result<Option<String>> {
    crate::semantic::rust::rust_symbol_name(node, source)?
        .map(|name| crate::semantic::rust::rust_semantic_path(node, source, &name))
        .transpose()
        .map(Option::flatten)
}

fn go_semantic_path_for_node(node: Node<'_>, source: &str) -> Result<Option<String>> {
    crate::semantic::go::go_symbol_name(node, source)?
        .map(|name| crate::semantic::go::go_semantic_path(node, source, &name))
        .transpose()
        .map(Option::flatten)
}

fn java_semantic_path_for_node(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut root = node;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    crate::semantic::java::java_symbol_name(node, source)?
        .map(|name| crate::semantic::java::java_semantic_path(root, node, source, &name))
        .transpose()
        .map(Option::flatten)
}

fn csharp_semantic_path_for_node(node: Node<'_>, source: &str) -> Result<Option<String>> {
    let mut root = node;
    while let Some(parent) = root.parent() {
        root = parent;
    }
    crate::semantic::csharp::csharp_symbol_name(node, source)?
        .map(|name| crate::semantic::csharp::csharp_semantic_path(root, node, source, &name))
        .transpose()
        .map(Option::flatten)
}

struct RustAdapter {
    syntax: SyntaxOnlyAdapter,
}

impl LanguageAdapter for RustAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        self.syntax.descriptor()
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
        crate::semantic::rust::build_rust_skeleton(
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
        crate::semantic::rust::find_rust_semantic_node(path, tree, source, target_path, deadline)
    }

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        let mut current = Some(node);
        while let Some(candidate) = current {
            if crate::semantic::rust::is_rust_symbol_node(candidate) {
                return Some(candidate);
            }
            current = candidate.parent();
        }
        None
    }

    fn position_symbol_identity(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity> {
        let semantic_path = rust_semantic_path_for_node(node, source)?.ok_or_else(|| {
            anyhow!("position does not resolve to a Rust symbol with a stable semantic path")
        })?;
        Ok(PositionSymbolIdentity {
            symbol_id: semantic_path.clone(),
            semantic_path,
            byte_range: (node.start_byte(), node.end_byte()),
        })
    }

    fn semantic_path_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>> {
        rust_semantic_path_for_node(node, source)
    }

    fn symbol_id_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        rust_semantic_path_for_node(node, source)
    }

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        self.syntax
            .requires_exact_symbol_id_for_ambiguous_semantic_paths()
    }

    fn query_owner_candidates<'tree>(
        &self,
        path: &Path,
        root: Node<'tree>,
        source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        self.syntax.query_owner_candidates(path, root, source)
    }

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        self.syntax.patch_replacement_node(node)
    }

    fn normalize_patch_replacement(
        &self,
        source: &str,
        start_byte: usize,
        end_byte: usize,
        node_kind: &str,
        new_code: &str,
    ) -> Result<String> {
        self.syntax
            .normalize_patch_replacement(source, start_byte, end_byte, node_kind, new_code)
    }

    fn replacement_preserves_required_wrappers(&self, node_kind: &str, replacement: &str) -> bool {
        self.syntax
            .replacement_preserves_required_wrappers(node_kind, replacement)
    }

    fn reconcile_patch_symbol_id(
        &self,
        semantic_target: &str,
        resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String {
        self.syntax
            .reconcile_patch_symbol_id(semantic_target, resolved_path, resolved_symbol_id)
    }

    fn collect_patch_reference_validation(
        &self,
        path: &Path,
        document: &ParsedDocument,
        source: &str,
        symbol_node: Node<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        self.syntax.collect_patch_reference_validation(
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
        self.syntax
            .query_capture_owner(path, source, node, candidates)
    }

    fn supports_incremental_file_dependencies(&self) -> bool {
        true
    }

    fn collect_local_file_dependencies(
        &self,
        path: &Path,
        root: Node<'_>,
        source: &str,
    ) -> Result<Vec<PathBuf>> {
        crate::language::rust_local_module_dependency_paths(path, root, source)
            .map(|paths| paths.into_iter().collect())
    }

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        crate::symbol_extractor::rust::index_rust_symbols_with_deadline(
            path,
            source,
            document.tree.root_node(),
            deadline,
        )
    }
}
struct GoAdapter {
    syntax: SyntaxOnlyAdapter,
}

impl LanguageAdapter for GoAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        self.syntax.descriptor()
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
        crate::semantic::go::build_go_skeleton(
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
        crate::semantic::go::find_go_semantic_node(path, tree, source, target_path, deadline)
    }

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        let mut current = Some(node);
        while let Some(candidate) = current {
            if crate::semantic::go::is_go_symbol_node(candidate) {
                return Some(candidate);
            }
            current = candidate.parent();
        }
        None
    }

    fn position_symbol_identity(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity> {
        let semantic_path = go_semantic_path_for_node(node, source)?.ok_or_else(|| {
            anyhow!("position does not resolve to a Go symbol with a stable semantic path")
        })?;
        Ok(PositionSymbolIdentity {
            symbol_id: semantic_path.clone(),
            semantic_path,
            byte_range: (node.start_byte(), node.end_byte()),
        })
    }

    fn semantic_path_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>> {
        go_semantic_path_for_node(node, source)
    }

    fn symbol_id_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        go_semantic_path_for_node(node, source)
    }

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        self.syntax
            .requires_exact_symbol_id_for_ambiguous_semantic_paths()
    }

    fn query_owner_candidates<'tree>(
        &self,
        path: &Path,
        root: Node<'tree>,
        source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        self.syntax.query_owner_candidates(path, root, source)
    }

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        self.syntax.patch_replacement_node(node)
    }

    fn normalize_patch_replacement(
        &self,
        source: &str,
        start_byte: usize,
        end_byte: usize,
        node_kind: &str,
        new_code: &str,
    ) -> Result<String> {
        self.syntax
            .normalize_patch_replacement(source, start_byte, end_byte, node_kind, new_code)
    }

    fn replacement_preserves_required_wrappers(&self, node_kind: &str, replacement: &str) -> bool {
        self.syntax
            .replacement_preserves_required_wrappers(node_kind, replacement)
    }

    fn reconcile_patch_symbol_id(
        &self,
        semantic_target: &str,
        resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String {
        self.syntax
            .reconcile_patch_symbol_id(semantic_target, resolved_path, resolved_symbol_id)
    }

    fn collect_patch_reference_validation(
        &self,
        path: &Path,
        document: &ParsedDocument,
        source: &str,
        symbol_node: Node<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        self.syntax.collect_patch_reference_validation(
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
        self.syntax
            .query_capture_owner(path, source, node, candidates)
    }

    fn supports_incremental_file_dependencies(&self) -> bool {
        true
    }

    fn collect_local_file_dependencies(
        &self,
        path: &Path,
        root: Node<'_>,
        source: &str,
    ) -> Result<Vec<PathBuf>> {
        crate::language::go_local_package_dependency_paths(path, root, source)
            .map(|paths| paths.into_iter().collect())
    }

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        crate::symbol_extractor::go::index_go_symbols_with_deadline(
            path,
            source,
            document.tree.root_node(),
            deadline,
        )
    }
}

struct JavaAdapter {
    syntax: SyntaxOnlyAdapter,
}

impl LanguageAdapter for JavaAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        self.syntax.descriptor()
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
        crate::semantic::java::build_java_skeleton(
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
        crate::semantic::java::find_java_semantic_node(path, tree, source, target_path, deadline)
    }

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        let mut current = Some(node);
        while let Some(candidate) = current {
            if crate::semantic::java::is_java_symbol_node(candidate) {
                return Some(candidate);
            }
            current = candidate.parent();
        }
        None
    }

    fn position_symbol_identity(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity> {
        let semantic_path = java_semantic_path_for_node(node, source)?.ok_or_else(|| {
            anyhow!("position does not resolve to a Java symbol with a stable semantic path")
        })?;
        Ok(PositionSymbolIdentity {
            symbol_id: semantic_path.clone(),
            semantic_path,
            byte_range: (node.start_byte(), node.end_byte()),
        })
    }

    fn semantic_path_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>> {
        java_semantic_path_for_node(node, source)
    }

    fn symbol_id_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        java_semantic_path_for_node(node, source)
    }

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        self.syntax
            .requires_exact_symbol_id_for_ambiguous_semantic_paths()
    }

    fn query_owner_candidates<'tree>(
        &self,
        path: &Path,
        root: Node<'tree>,
        source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        self.syntax.query_owner_candidates(path, root, source)
    }

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        self.syntax.patch_replacement_node(node)
    }

    fn normalize_patch_replacement(
        &self,
        source: &str,
        start_byte: usize,
        end_byte: usize,
        node_kind: &str,
        new_code: &str,
    ) -> Result<String> {
        self.syntax
            .normalize_patch_replacement(source, start_byte, end_byte, node_kind, new_code)
    }

    fn replacement_preserves_required_wrappers(&self, node_kind: &str, replacement: &str) -> bool {
        self.syntax
            .replacement_preserves_required_wrappers(node_kind, replacement)
    }

    fn reconcile_patch_symbol_id(
        &self,
        semantic_target: &str,
        resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String {
        self.syntax
            .reconcile_patch_symbol_id(semantic_target, resolved_path, resolved_symbol_id)
    }

    fn collect_patch_reference_validation(
        &self,
        path: &Path,
        document: &ParsedDocument,
        source: &str,
        symbol_node: Node<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        self.syntax.collect_patch_reference_validation(
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
        self.syntax
            .query_capture_owner(path, source, node, candidates)
    }

    fn supports_incremental_file_dependencies(&self) -> bool {
        true
    }

    fn collect_local_file_dependencies(
        &self,
        path: &Path,
        root: Node<'_>,
        source: &str,
    ) -> Result<Vec<PathBuf>> {
        crate::language::java_local_file_dependency_paths(path, root, source)
            .map(|paths| paths.into_iter().collect())
    }

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        crate::symbol_extractor::java::index_java_symbols_with_deadline(
            path,
            source,
            document.tree.root_node(),
            deadline,
        )
    }
}

struct CSharpAdapter {
    syntax: SyntaxOnlyAdapter,
}

impl LanguageAdapter for CSharpAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        self.syntax.descriptor()
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
        crate::semantic::csharp::build_csharp_skeleton(
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
        crate::semantic::csharp::find_csharp_semantic_node(
            path,
            tree,
            source,
            target_path,
            deadline,
        )
    }

    fn ascend_to_symbol<'tree>(&self, node: Node<'tree>) -> Option<Node<'tree>> {
        let mut current = Some(node);
        while let Some(candidate) = current {
            if crate::semantic::csharp::is_csharp_symbol_node(candidate) {
                return Some(candidate);
            }
            current = candidate.parent();
        }
        None
    }

    fn position_symbol_identity(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<PositionSymbolIdentity> {
        let semantic_path = csharp_semantic_path_for_node(node, source)?.ok_or_else(|| {
            anyhow!("position does not resolve to a C# symbol with a stable semantic path")
        })?;
        Ok(PositionSymbolIdentity {
            symbol_id: semantic_path.clone(),
            semantic_path,
            byte_range: (node.start_byte(), node.end_byte()),
        })
    }

    fn semantic_path_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
    ) -> Result<Option<String>> {
        csharp_semantic_path_for_node(node, source)
    }

    fn symbol_id_for_node(
        &self,
        _path: &Path,
        node: Node<'_>,
        source: &str,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        csharp_semantic_path_for_node(node, source)
    }

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        self.syntax
            .requires_exact_symbol_id_for_ambiguous_semantic_paths()
    }

    fn query_owner_candidates<'tree>(
        &self,
        path: &Path,
        root: Node<'tree>,
        source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        self.syntax.query_owner_candidates(path, root, source)
    }

    fn patch_replacement_node<'tree>(&self, node: Node<'tree>) -> Node<'tree> {
        self.syntax.patch_replacement_node(node)
    }

    fn normalize_patch_replacement(
        &self,
        source: &str,
        start_byte: usize,
        end_byte: usize,
        node_kind: &str,
        new_code: &str,
    ) -> Result<String> {
        self.syntax
            .normalize_patch_replacement(source, start_byte, end_byte, node_kind, new_code)
    }

    fn replacement_preserves_required_wrappers(&self, node_kind: &str, replacement: &str) -> bool {
        self.syntax
            .replacement_preserves_required_wrappers(node_kind, replacement)
    }

    fn reconcile_patch_symbol_id(
        &self,
        semantic_target: &str,
        resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String {
        self.syntax
            .reconcile_patch_symbol_id(semantic_target, resolved_path, resolved_symbol_id)
    }

    fn collect_patch_reference_validation(
        &self,
        path: &Path,
        document: &ParsedDocument,
        source: &str,
        symbol_node: Node<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        self.syntax.collect_patch_reference_validation(
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
        self.syntax
            .query_capture_owner(path, source, node, candidates)
    }

    fn supports_incremental_file_dependencies(&self) -> bool {
        self.syntax.supports_incremental_file_dependencies()
    }

    fn collect_local_file_dependencies(
        &self,
        path: &Path,
        root: Node<'_>,
        source: &str,
    ) -> Result<Vec<PathBuf>> {
        self.syntax
            .collect_local_file_dependencies(path, root, source)
    }

    fn extract_symbols(
        &self,
        path: &Path,
        source: &str,
        document: &ParsedDocument,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        crate::symbol_extractor::csharp::index_csharp_symbols_with_deadline(
            path,
            source,
            document.tree.root_node(),
            deadline,
        )
    }
}

struct SyntaxOnlyAdapter {
    descriptor: &'static LanguageDescriptor,
}

impl SyntaxOnlyAdapter {
    fn unsupported<T>(&self, operation: &str) -> Result<T> {
        bail!(
            "{} does not support {operation}",
            self.descriptor.display_name
        )
    }
}

impl LanguageAdapter for SyntaxOnlyAdapter {
    fn descriptor(&self) -> &'static LanguageDescriptor {
        self.descriptor
    }

    fn build_semantic_skeleton(
        &self,
        _path: &Path,
        _source: &str,
        _tree: &Tree,
        _depth_limit: usize,
        _expand_nodes: &[String],
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<SemanticSkeleton> {
        self.unsupported("semantic skeletons")
    }

    fn find_semantic_node<'tree>(
        &self,
        _path: &Path,
        _tree: &'tree Tree,
        _source: &str,
        _target_path: &str,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<Node<'tree>>> {
        self.unsupported("semantic symbol lookup")
    }

    fn ascend_to_symbol<'tree>(&self, _node: Node<'tree>) -> Option<Node<'tree>> {
        None
    }

    fn position_symbol_identity(
        &self,
        _path: &Path,
        _node: Node<'_>,
        _source: &str,
    ) -> Result<PositionSymbolIdentity> {
        self.unsupported("symbol positions")
    }

    fn semantic_path_for_node(
        &self,
        _path: &Path,
        _node: Node<'_>,
        _source: &str,
    ) -> Result<Option<String>> {
        self.unsupported("semantic skeletons")
    }

    fn symbol_id_for_node(
        &self,
        _path: &Path,
        _node: Node<'_>,
        _source: &str,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<String>> {
        self.unsupported("symbol positions")
    }

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        false
    }

    fn query_owner_candidates<'tree>(
        &self,
        _path: &Path,
        _root: Node<'tree>,
        _source: &str,
    ) -> Result<Option<Vec<Node<'tree>>>> {
        Ok(None)
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
        _new_code: &str,
    ) -> Result<String> {
        self.unsupported("patch targeting")
    }

    fn replacement_preserves_required_wrappers(
        &self,
        _node_kind: &str,
        _replacement: &str,
    ) -> bool {
        false
    }

    fn reconcile_patch_symbol_id(
        &self,
        _semantic_target: &str,
        _resolved_path: &str,
        resolved_symbol_id: String,
    ) -> String {
        resolved_symbol_id
    }

    fn collect_patch_reference_validation(
        &self,
        _path: &Path,
        _document: &ParsedDocument,
        _source: &str,
        _symbol_node: Node<'_>,
        _deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<crate::patching::ReferenceValidation> {
        self.unsupported("patch validation")
    }

    fn query_capture_owner(
        &self,
        _path: &Path,
        _source: &str,
        _node: Node<'_>,
        _candidates: Option<&[Node<'_>]>,
    ) -> Result<(Option<String>, Option<String>, Option<String>)> {
        Ok((None, None, None))
    }

    fn supports_incremental_file_dependencies(&self) -> bool {
        false
    }

    fn collect_local_file_dependencies(
        &self,
        _path: &Path,
        _root: Node<'_>,
        _source: &str,
    ) -> Result<Vec<PathBuf>> {
        self.unsupported("file dependency extraction")
    }

    fn extract_symbols(
        &self,
        _path: &Path,
        _source: &str,
        _document: &ParsedDocument,
        _deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbol>> {
        self.unsupported("symbol indexing")
    }
}

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

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        true
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

    fn supports_incremental_file_dependencies(&self) -> bool {
        false
    }

    fn collect_local_file_dependencies(
        &self,
        _path: &Path,
        _root: Node<'_>,
        _source: &str,
    ) -> Result<Vec<PathBuf>> {
        Ok(Vec::new())
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

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        false
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

    fn supports_incremental_file_dependencies(&self) -> bool {
        true
    }

    fn collect_local_file_dependencies(
        &self,
        path: &Path,
        root: Node<'_>,
        source: &str,
    ) -> Result<Vec<PathBuf>> {
        crate::language::c_local_include_dependency_paths(path, root, source)
            .map(|paths| paths.into_iter().collect())
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

    fn requires_exact_symbol_id_for_ambiguous_semantic_paths(&self) -> bool {
        C_ADAPTER.requires_exact_symbol_id_for_ambiguous_semantic_paths()
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

    fn supports_incremental_file_dependencies(&self) -> bool {
        C_ADAPTER.supports_incremental_file_dependencies()
    }

    fn collect_local_file_dependencies(
        &self,
        path: &Path,
        root: Node<'_>,
        source: &str,
    ) -> Result<Vec<PathBuf>> {
        C_ADAPTER.collect_local_file_dependencies(path, root, source)
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

fn javascript_grammar() -> Language {
    tree_sitter_javascript::LANGUAGE.into()
}

fn typescript_grammar() -> Language {
    tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
}

fn tsx_grammar() -> Language {
    tree_sitter_typescript::LANGUAGE_TSX.into()
}

fn rust_grammar() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}

fn go_grammar() -> Language {
    tree_sitter_go::LANGUAGE.into()
}

fn java_grammar() -> Language {
    tree_sitter_java::LANGUAGE.into()
}

fn csharp_grammar() -> Language {
    tree_sitter_c_sharp::LANGUAGE.into()
}
