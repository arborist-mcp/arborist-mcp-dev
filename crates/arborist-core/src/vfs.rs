use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use tree_sitter::{InputEdit, Tree};

use crate::language::{
    normalize_absolute_path, normalize_path, offset_for_position, parse_document,
    parser_for_language, point_for_offset,
};
use crate::model::LanguageId;
use crate::model::{
    PatchAstNodeResult, PatchValidationReport, PositionEdit, RegisteredSymbolIndex,
    SymbolContextResult, SymbolIndexStats, SymbolListContextResult,
    SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult, SymbolListResult,
    SymbolNeighborhoodContextResult, SymbolReadResult, SymbolSearchContextResult,
    SymbolSearchDiscoveryContextResult, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
    TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult, VirtualEditResult,
    VirtualFileSnapshot, VirtualFileStatus,
};
use crate::patching::{
    build_patch_result, collect_syntax_errors, semantic_target_range, splice_source,
    validate_bypass_reason, validate_patch_replacement,
};
use crate::symbols::{
    list_symbols_context_with_overrides_filtered,
    list_symbols_discovery_context_with_overrides_filtered,
    list_symbols_neighborhood_context_with_overrides_filtered,
    list_symbols_with_overrides_filtered, read_symbol_context_with_overrides,
    read_symbol_neighborhood_context_with_overrides, read_symbol_with_overrides,
    rebuild_symbol_index, refresh_symbol_index_for_file,
    search_symbols_context_with_overrides_filtered,
    search_symbols_discovery_context_with_overrides_filtered,
    search_symbols_neighborhood_context_with_overrides_filtered,
    search_symbols_with_overrides_filtered, trace_symbol_graph_with_overrides,
    trace_symbol_neighborhood_with_overrides,
};

#[derive(Default)]
pub struct VirtualFileSystem {
    entries: HashMap<String, VirtualFileEntry>,
    symbol_indexes: HashMap<String, PathBuf>,
}

#[derive(Clone)]
struct VirtualFileEntry {
    path: PathBuf,
    language_id: LanguageId,
    disk_source: String,
    source: String,
    tree: Tree,
    version: u64,
    dirty: bool,
}

fn normalized_virtual_path(path: &Path) -> Result<(PathBuf, String)> {
    let absolute_path = normalize_absolute_path(path)?;
    let normalized = normalize_path(&absolute_path);
    Ok((absolute_path, normalized))
}

fn read_virtual_disk_source(path: &Path) -> Result<String> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(source),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(error) => {
            Err(error).with_context(|| format!("failed to read source file {}", path.display()))
        }
    }
}

impl VirtualFileSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open_file(&mut self, path: &Path, source: Option<&str>) -> Result<VirtualFileSnapshot> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, source)?;
        self.refresh_if_clean(&normalized)?;

        let entry = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
        snapshot_from_entry(&normalized, entry)
    }

    pub fn read_file(&mut self, path: &Path) -> Result<VirtualFileSnapshot> {
        self.open_file(path, None)
    }

    pub fn apply_edit(
        &mut self,
        path: &Path,
        start_byte: usize,
        old_end_byte: usize,
        new_text: &str,
    ) -> Result<VirtualEditResult> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let entry = self
            .entries
            .get_mut(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;

        validate_edit_range(&entry.source, start_byte, old_end_byte)?;
        let updated_source = splice_source(&entry.source, start_byte..old_end_byte, new_text);

        let edit = InputEdit {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte + new_text.len(),
            start_position: point_for_offset(&entry.source, start_byte)?,
            old_end_position: point_for_offset(&entry.source, old_end_byte)?,
            new_end_position: point_for_offset(&updated_source, start_byte + new_text.len())?,
        };

        entry.tree.edit(&edit);
        let mut parser = parser_for_language(entry.language_id)?;
        let new_tree = parser
            .parse(&updated_source, Some(&entry.tree))
            .ok_or_else(|| anyhow!("incremental parse failed for {}", entry.path.display()))?;

        let syntax_errors = collect_syntax_errors(new_tree.root_node(), &updated_source);
        entry.source = updated_source.clone();
        entry.tree = new_tree;
        entry.version += 1;
        entry.dirty = entry.source != entry.disk_source;

        let result = VirtualEditResult {
            file: normalized,
            source: updated_source,
            dirty: entry.dirty,
            version: entry.version,
            incremental_parse: true,
            validation: PatchValidationReport {
                syntax_errors,
                unresolved_identifiers: Vec::new(),
                resolved_identifiers: Vec::new(),
                ambiguous_identifiers: Vec::new(),
                binding_decisions: Vec::new(),
                commit_gate: Default::default(),
            },
        };
        result.validate_public_output()?;
        Ok(result)
    }

    pub fn apply_position_edits(
        &mut self,
        path: &Path,
        edits: &[PositionEdit],
    ) -> Result<VirtualEditResult> {
        if edits.is_empty() {
            let (path, normalized) = normalized_virtual_path(path)?;
            self.ensure_loaded(&path, None)?;
            self.refresh_if_clean(&normalized)?;

            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            let result = VirtualEditResult {
                file: normalized,
                source: entry.source.clone(),
                dirty: entry.dirty,
                version: entry.version,
                incremental_parse: true,
                validation: PatchValidationReport {
                    syntax_errors: collect_syntax_errors(entry.tree.root_node(), &entry.source),
                    unresolved_identifiers: Vec::new(),
                    resolved_identifiers: Vec::new(),
                    ambiguous_identifiers: Vec::new(),
                    binding_decisions: Vec::new(),
                    commit_gate: Default::default(),
                },
            };
            result.validate_public_output()?;
            return Ok(result);
        }

        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let previous = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .clone();

        let mut last_result = None;
        for (index, edit) in edits.iter().enumerate() {
            let result = (|| -> Result<VirtualEditResult> {
                let source = self
                    .entries
                    .get(&normalized)
                    .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
                    .source
                    .clone();
                let start_byte = offset_for_position(&source, &edit.start)?;
                let old_end_byte = offset_for_position(&source, &edit.end)?;
                self.apply_edit(&path, start_byte, old_end_byte, &edit.new_text)
            })()
            .with_context(|| format!("failed to apply position edit at index {index}"));

            match result {
                Ok(result) => last_result = Some(result),
                Err(error) => {
                    self.entries.insert(normalized, previous);
                    return Err(error);
                }
            }
        }

        last_result.ok_or_else(|| anyhow!("position edits did not produce a result"))
    }

    pub fn patch_node(
        &mut self,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
    ) -> Result<PatchAstNodeResult> {
        validate_patch_replacement(new_code)?;
        validate_bypass_reason(bypass_reason)?;

        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let (start_byte, end_byte) = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            semantic_target_range(&entry.path, &entry.source, semantic_target)?
        };

        let previous = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?
            .clone();

        self.apply_edit(&path, start_byte, end_byte, new_code)?;

        let validation_result = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            build_patch_result(
                &entry.path,
                semantic_target,
                entry.source.clone(),
                bypass_reason,
                start_byte,
                new_code.len(),
            )
        };

        let result = match validation_result {
            Ok(result) => result,
            Err(error) => {
                self.entries.insert(normalized, previous);
                return Err(error).context("failed to validate virtual patch");
            }
        };

        if !result.applied {
            self.entries.insert(normalized, previous);
        }

        Ok(result)
    }

    pub fn commit_file(&mut self, path: &Path) -> Result<VirtualFileSnapshot> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let committed_path = {
            let entry = self
                .entries
                .get_mut(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;

            if entry.dirty {
                fs::write(&entry.path, &entry.source)
                    .with_context(|| format!("failed to write {}", entry.path.display()))?;
                entry.disk_source = entry.source.clone();
                entry.dirty = false;
            }

            entry.path.clone()
        };

        self.sync_registered_indexes(&committed_path)?;

        let entry = self
            .entries
            .get(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
        snapshot_from_entry(&normalized, entry)
    }

    pub fn discard_file(&mut self, path: &Path) -> Result<VirtualFileSnapshot> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;

        let entry = self
            .entries
            .get_mut(&normalized)
            .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;

        let disk_source = read_virtual_disk_source(&entry.path)?;
        let document = parse_document(&entry.path, &disk_source)?;
        entry.language_id = document.language_id;
        entry.disk_source = disk_source.clone();
        entry.source = disk_source;
        entry.tree = document.tree;
        entry.version += 1;
        entry.dirty = false;

        snapshot_from_entry(&normalized, entry)
    }

    pub fn close_file(&mut self, path: &Path, persist: bool) -> Result<VirtualFileSnapshot> {
        let snapshot = if persist {
            self.commit_file(path)?
        } else {
            self.discard_file(path)?
        };
        self.entries.remove(&snapshot.file);
        Ok(snapshot)
    }

    pub fn register_symbol_index(
        &mut self,
        workspace_root: &Path,
        db_path: &Path,
    ) -> Result<SymbolIndexStats> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let db_path = normalize_absolute_path(db_path)?;
        let stats = rebuild_symbol_index(&workspace_root, &db_path)?;
        self.symbol_indexes
            .insert(normalize_path(&workspace_root), db_path);
        Ok(stats)
    }

    pub fn unregister_symbol_index(&mut self, workspace_root: &Path) -> Result<bool> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        Ok(self
            .symbol_indexes
            .remove(&normalize_path(&workspace_root))
            .is_some())
    }

    pub fn registered_symbol_indexes(&self) -> Vec<RegisteredSymbolIndex> {
        let mut indexes: Vec<_> = self
            .symbol_indexes
            .iter()
            .map(|(workspace_root, db_path)| RegisteredSymbolIndex {
                workspace_root: workspace_root.clone(),
                db_path: normalize_path(db_path),
            })
            .collect();
        indexes.sort_by(|left, right| left.workspace_root.cmp(&right.workspace_root));
        indexes
    }

    pub fn registered_symbol_indexes_checked(&self) -> Result<Vec<RegisteredSymbolIndex>> {
        let indexes = self.registered_symbol_indexes();
        for (index, registered) in indexes.iter().enumerate() {
            registered.validate_public_output(index)?;
        }
        Ok(indexes)
    }

    pub fn virtual_file_statuses(&mut self, dirty_only: bool) -> Result<Vec<VirtualFileStatus>> {
        let loaded_files: Vec<_> = self.entries.keys().cloned().collect();
        for normalized in &loaded_files {
            self.refresh_if_clean(normalized)?;
        }

        let mut statuses: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(file, entry)| {
                if dirty_only && !entry.dirty {
                    return None;
                }

                Some(VirtualFileStatus {
                    file: file.clone(),
                    dirty: entry.dirty,
                    version: entry.version,
                    syntax_error_count: collect_syntax_errors(
                        entry.tree.root_node(),
                        &entry.source,
                    )
                    .len(),
                })
            })
            .collect();
        statuses.sort_by(|left, right| left.file.cmp(&right.file));
        for (index, status) in statuses.iter().enumerate() {
            status.validate_public_output(index)?;
        }
        Ok(statuses)
    }

    pub fn trace_symbol_graph(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_graph_with_overrides(&workspace_root, &overrides, symbol_path, direction)
    }

    pub fn trace_symbol_neighborhood(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_neighborhood_with_overrides(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn read_symbol(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
    ) -> Result<SymbolReadResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_with_overrides(&workspace_root, &overrides, symbol_path)
    }

    pub fn read_symbol_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_context_with_overrides(&workspace_root, &overrides, symbol_path, direction)
    }

    pub fn read_symbol_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_neighborhood_context_with_overrides(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn search_symbols(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<SymbolSearchResult> {
        self.search_symbols_filtered(workspace_root, query, limit, None, None)
    }

    pub fn search_symbols_filtered(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        search_symbols_with_overrides_filtered(
            &workspace_root,
            &overrides,
            query,
            limit,
            file_path_contains,
            node_kind,
        )
    }

    pub fn search_symbols_context(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
    ) -> Result<SymbolSearchContextResult> {
        self.search_symbols_context_filtered(workspace_root, query, limit, None, None)
    }

    pub fn search_symbols_context_filtered(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        search_symbols_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            query,
            limit,
            file_path_contains,
            node_kind,
        )
    }

    pub fn search_symbols_discovery_context(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolSearchDiscoveryContextResult> {
        self.search_symbols_discovery_context_filtered(
            workspace_root,
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            None,
            None,
        )
    }

    pub fn search_symbols_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolSearchNeighborhoodContextResult> {
        self.search_symbols_neighborhood_context_filtered(
            workspace_root,
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_symbols_neighborhood_context_filtered(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchNeighborhoodContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        search_symbols_neighborhood_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_symbols_discovery_context_filtered(
        &mut self,
        workspace_root: &Path,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchDiscoveryContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        search_symbols_discovery_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            query,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    }

    pub fn list_symbols(
        &mut self,
        workspace_root: &Path,
        limit: usize,
    ) -> Result<SymbolListResult> {
        self.list_symbols_filtered(workspace_root, limit, None, None)
    }

    pub fn list_symbols_context(
        &mut self,
        workspace_root: &Path,
        limit: usize,
    ) -> Result<SymbolListContextResult> {
        self.list_symbols_context_filtered(workspace_root, limit, None, None)
    }

    pub fn list_symbols_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolListNeighborhoodContextResult> {
        self.list_symbols_neighborhood_context_filtered(
            workspace_root,
            limit,
            direction,
            max_depth,
            max_nodes,
            None,
            None,
        )
    }

    pub fn list_symbols_discovery_context(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolListDiscoveryContextResult> {
        self.list_symbols_discovery_context_filtered(
            workspace_root,
            limit,
            direction,
            max_depth,
            max_nodes,
            None,
            None,
        )
    }

    pub fn list_symbols_filtered(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        list_symbols_with_overrides_filtered(
            &workspace_root,
            &overrides,
            limit,
            file_path_contains,
            node_kind,
        )
    }

    pub fn list_symbols_context_filtered(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        list_symbols_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            limit,
            file_path_contains,
            node_kind,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_neighborhood_context_filtered(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListNeighborhoodContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        list_symbols_neighborhood_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_discovery_context_filtered(
        &mut self,
        workspace_root: &Path,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListDiscoveryContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        list_symbols_discovery_context_with_overrides_filtered(
            &workspace_root,
            &overrides,
            limit,
            direction,
            max_depth,
            max_nodes,
            file_path_contains,
            node_kind,
        )
    }

    fn ensure_loaded(&mut self, path: &Path, source_override: Option<&str>) -> Result<()> {
        let (path, normalized) = normalized_virtual_path(path)?;
        match self.entries.get_mut(&normalized) {
            Some(entry) => {
                if let Some(source_override) = source_override {
                    let disk_source = read_virtual_disk_source(&path)?;
                    let document = parse_document(&path, source_override)?;
                    entry.path = path;
                    entry.language_id = document.language_id;
                    entry.disk_source = disk_source;
                    entry.source = source_override.to_string();
                    entry.tree = document.tree;
                    entry.version += 1;
                    entry.dirty = entry.source != entry.disk_source;
                }
            }
            None => {
                let disk_source = read_virtual_disk_source(&path)?;
                let initial_source = source_override.unwrap_or(&disk_source).to_string();
                let document = parse_document(&path, &initial_source)?;
                let dirty = initial_source != disk_source;
                self.entries.insert(
                    normalized,
                    VirtualFileEntry {
                        path,
                        language_id: document.language_id,
                        disk_source,
                        source: initial_source,
                        tree: document.tree,
                        version: 0,
                        dirty,
                    },
                );
            }
        }
        Ok(())
    }

    fn refresh_if_clean(&mut self, normalized: &str) -> Result<()> {
        let Some(entry) = self.entries.get_mut(normalized) else {
            return Ok(());
        };
        if entry.dirty {
            return Ok(());
        }

        let disk_source = read_virtual_disk_source(&entry.path)?;
        if disk_source == entry.disk_source {
            return Ok(());
        }

        let document = parse_document(&entry.path, &disk_source)?;
        entry.language_id = document.language_id;
        entry.disk_source = disk_source.clone();
        entry.source = disk_source;
        entry.tree = document.tree;
        entry.version += 1;
        Ok(())
    }

    fn sync_registered_indexes(&self, file_path: &Path) -> Result<()> {
        let file_path = normalize_absolute_path(file_path)?;
        for (workspace_root, db_path) in &self.symbol_indexes {
            let workspace_root_path = Path::new(workspace_root);
            if file_path.starts_with(workspace_root_path) {
                refresh_symbol_index_for_file(workspace_root_path, db_path, &file_path)?;
            }
        }
        Ok(())
    }

    fn virtual_overrides_for_workspace(
        &mut self,
        workspace_root: &Path,
    ) -> Result<BTreeMap<String, String>> {
        let loaded_files: Vec<_> = self.entries.keys().cloned().collect();
        for normalized in &loaded_files {
            self.refresh_if_clean(normalized)?;
        }

        let mut overrides = BTreeMap::new();
        for entry in self.entries.values() {
            if !entry.dirty {
                continue;
            }

            let absolute_path = normalize_absolute_path(&entry.path)?;
            if absolute_path.starts_with(workspace_root) {
                overrides.insert(normalize_path(&absolute_path), entry.source.clone());
            }
        }

        Ok(overrides)
    }
}

fn snapshot_from_entry(file: &str, entry: &VirtualFileEntry) -> Result<VirtualFileSnapshot> {
    let snapshot = VirtualFileSnapshot {
        file: file.to_string(),
        source: entry.source.clone(),
        disk_source: entry.disk_source.clone(),
        dirty: entry.dirty,
        version: entry.version,
        syntax_error_count: collect_syntax_errors(entry.tree.root_node(), &entry.source).len(),
    };
    snapshot.validate_public_output()?;
    Ok(snapshot)
}

fn validate_edit_range(source: &str, start_byte: usize, old_end_byte: usize) -> Result<()> {
    if start_byte > old_end_byte {
        bail!(
            "edit start_byte {} is after old_end_byte {}",
            start_byte,
            old_end_byte
        );
    }
    if old_end_byte > source.len() {
        bail!(
            "edit old_end_byte {} is out of bounds for source of length {}",
            old_end_byte,
            source.len()
        );
    }
    if !source.is_char_boundary(start_byte) || !source.is_char_boundary(old_end_byte) {
        bail!("edit range must align to UTF-8 character boundaries");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::VirtualFileSystem;
    use crate::{Position, PositionEdit, TraceDirection, trace_symbol_graph_from_index};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn applies_incremental_edit_and_commits() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        let snapshot = vfs.read_file(&file).unwrap();
        assert!(!snapshot.dirty);
        assert_eq!(snapshot.version, 0);
        let digit_offset = snapshot.source.rfind('1').unwrap();

        let result = vfs
            .apply_edit(&file, digit_offset, digit_offset + 1, "2")
            .unwrap();
        assert!(result.incremental_parse);
        assert!(result.dirty);
        assert_eq!(result.version, 1);
        assert!(result.source.contains("return 2"));

        let committed = vfs.commit_file(&file).unwrap();
        assert!(!committed.dirty);
        assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
    }

    #[test]
    fn path_aliases_share_one_virtual_entry() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let alias_dir = file.parent().unwrap().join("child");
        fs::create_dir_all(&alias_dir).unwrap();
        let alias = alias_dir.join("..").join("buffer.py");
        let mut vfs = VirtualFileSystem::new();

        let snapshot = vfs.read_file(&alias).unwrap();
        assert!(!snapshot.file.contains("/../"));
        let digit_offset = snapshot.source.rfind('1').unwrap();

        vfs.apply_edit(&file, digit_offset, digit_offset + 1, "2")
            .unwrap();

        let statuses = vfs.virtual_file_statuses(false).unwrap();
        assert_eq!(statuses.len(), 1);
        assert!(statuses[0].dirty);

        let aliased_snapshot = vfs.read_file(&alias).unwrap();
        assert!(aliased_snapshot.source.contains("return 2"));

        let committed = vfs.commit_file(&alias).unwrap();
        assert!(!committed.dirty);
        assert!(fs::read_to_string(&file).unwrap().contains("return 2"));
    }

    #[test]
    fn discards_virtual_changes() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        let snapshot = vfs.read_file(&file).unwrap();
        let digit_offset = snapshot.source.rfind('1').unwrap();
        vfs.apply_edit(&file, digit_offset, digit_offset + 1, "9")
            .unwrap();
        let discarded = vfs.discard_file(&file).unwrap();

        assert!(!discarded.dirty);
        assert!(discarded.source.contains("return 1"));
    }

    #[test]
    fn rejects_byte_edit_inside_utf8_character() {
        let file = temp_file("def value() -> str:\n    return 'é'\n");
        let mut vfs = VirtualFileSystem::new();

        let snapshot = vfs.read_file(&file).unwrap();
        let character_start = snapshot.source.find('é').unwrap();
        let interior_byte = character_start + 1;
        let error = vfs
            .apply_edit(&file, interior_byte, interior_byte, "x")
            .expect_err("byte edits must not split UTF-8 characters");

        assert!(
            error
                .to_string()
                .contains("edit range must align to UTF-8 character boundaries")
        );
        let unchanged = vfs.read_file(&file).unwrap();
        assert!(!unchanged.dirty);
        assert_eq!(unchanged.source, snapshot.source);
    }

    #[test]
    fn discard_refreshes_from_current_disk_source() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        vfs.open_file(&file, Some("def value() -> int:\n    return 9\n"))
            .unwrap();
        fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
        let discarded = vfs.discard_file(&file).unwrap();

        assert!(!discarded.dirty);
        assert!(discarded.source.contains("return 2"));
        assert_eq!(discarded.disk_source, discarded.source);
    }

    #[test]
    fn refreshes_clean_file_deleted_on_disk_as_empty() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        let snapshot = vfs.read_file(&file).unwrap();
        assert!(!snapshot.dirty);
        assert_eq!(snapshot.version, 0);

        fs::remove_file(&file).unwrap();
        let refreshed = vfs.read_file(&file).unwrap();

        assert!(!refreshed.dirty);
        assert_eq!(refreshed.source, "");
        assert_eq!(refreshed.disk_source, "");
        assert_eq!(refreshed.version, 1);
    }

    #[test]
    fn commit_refreshes_clean_file_changed_on_disk() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        let snapshot = vfs.read_file(&file).unwrap();
        assert!(!snapshot.dirty);
        assert_eq!(snapshot.version, 0);

        fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
        let committed = vfs.commit_file(&file).unwrap();

        assert!(!committed.dirty);
        assert!(committed.source.contains("return 2"));
        assert_eq!(committed.disk_source, committed.source);
        assert_eq!(committed.version, 1);
    }

    #[test]
    fn patches_virtual_symbol_without_immediate_commit() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        let result = vfs
            .patch_node(&file, "value", "def value() -> int:\n    return 3\n", None)
            .unwrap();

        assert!(result.applied);
        let snapshot = vfs.read_file(&file).unwrap();
        assert!(snapshot.dirty);
        assert!(snapshot.source.contains("return 3"));
        assert!(fs::read_to_string(&file).unwrap().contains("return 1"));
    }

    #[test]
    fn rejects_blank_virtual_patch_without_dirtying_buffer() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();
        let initial = vfs.read_file(&file).unwrap();

        let error = vfs
            .patch_node(&file, "value", " \t", None)
            .expect_err("blank virtual patch replacements should be rejected");

        assert!(error.to_string().contains("new_code"));
        assert!(error.to_string().contains("blank"));
        let snapshot = vfs.read_file(&file).unwrap();
        assert_eq!(snapshot.source, initial.source);
        assert_eq!(snapshot.version, initial.version);
        assert_eq!(snapshot.dirty, initial.dirty);
    }

    #[test]
    fn rejects_blank_virtual_patch_bypass_without_dirtying_buffer() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();
        let initial = vfs.read_file(&file).unwrap();

        let error = vfs
            .patch_node(
                &file,
                "value",
                "def value() -> int:\n    return 2\n",
                Some(" \t"),
            )
            .expect_err("blank virtual patch bypass reasons should be rejected");

        assert!(error.to_string().contains("bypass_reason"));
        assert!(error.to_string().contains("blank"));
        let snapshot = vfs.read_file(&file).unwrap();
        assert_eq!(snapshot.source, initial.source);
        assert_eq!(snapshot.version, initial.version);
        assert_eq!(snapshot.dirty, initial.dirty);
    }

    #[test]
    fn rolls_back_invalid_virtual_patch() {
        let file = temp_file(
            "def helper(value: int) -> int:\n    return value + 1\n\ndef value() -> int:\n    return helper(1)\n",
        );
        let mut vfs = VirtualFileSystem::new();

        let result = vfs
            .patch_node(
                &file,
                "value",
                "def value() -> int:\n    return missing_helper(1)\n",
                None,
            )
            .unwrap();

        assert!(!result.applied);
        assert_eq!(
            result.validation.unresolved_identifiers,
            vec!["missing_helper"]
        );

        let snapshot = vfs.read_file(&file).unwrap();
        assert!(!snapshot.dirty);
        assert!(snapshot.source.contains("return helper(1)"));
    }

    #[test]
    fn rolls_back_virtual_patch_when_validation_errors() {
        let workspace = temp_workspace();
        let file = workspace.join("sample.c");
        let bad_include = workspace.join("bad.txt");
        fs::write(&bad_include, "int helper(void);\n").unwrap();
        fs::write(
            &file,
            "#include \"bad.txt\"\n\nint value(void) {\n    return 1;\n}\n",
        )
        .unwrap();
        let mut vfs = VirtualFileSystem::new();
        let initial = vfs.read_file(&file).unwrap();

        let error = vfs
            .patch_node(
                &file,
                "value",
                "int value(void) {\n    return helper();\n}\n",
                None,
            )
            .expect_err("validation errors should reject the virtual patch");

        assert!(
            error
                .to_string()
                .contains("failed to validate virtual patch")
        );
        let snapshot = vfs.read_file(&file).unwrap();
        assert_eq!(snapshot.source, initial.source);
        assert_eq!(snapshot.version, initial.version);
        assert_eq!(snapshot.dirty, initial.dirty);
    }

    #[test]
    fn opens_with_virtual_source_and_lists_dirty_files() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        let snapshot = vfs
            .open_file(&file, Some("def value() -> int:\n    return 7\n"))
            .unwrap();
        assert!(snapshot.dirty);
        assert!(snapshot.source.contains("return 7"));
        assert!(snapshot.disk_source.contains("return 1"));

        let dirty_files = vfs.virtual_file_statuses(true).unwrap();
        assert_eq!(dirty_files.len(), 1);
        assert_eq!(dirty_files[0].file, snapshot.file);
        assert!(dirty_files[0].dirty);
    }

    #[test]
    fn open_with_source_refreshes_disk_baseline() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        let initial = vfs.read_file(&file).unwrap();
        assert!(!initial.dirty);

        fs::write(&file, "def value() -> int:\n    return 2\n").unwrap();
        let reopened = vfs
            .open_file(&file, Some("def value() -> int:\n    return 2\n"))
            .unwrap();

        assert!(!reopened.dirty);
        assert!(reopened.source.contains("return 2"));
        assert_eq!(reopened.disk_source, reopened.source);
    }

    #[test]
    fn list_virtual_files_refreshes_clean_disk_changes() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        vfs.read_file(&file).unwrap();
        fs::write(&file, "def value(\n").unwrap();
        let statuses = vfs.virtual_file_statuses(false).unwrap();

        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].version, 1);
        assert!(statuses[0].syntax_error_count > 0);
        assert!(!statuses[0].dirty);
    }

    #[test]
    fn applies_position_edits_in_sequence() {
        let file = temp_file("def value() -> int:\n    return 10\n");
        let mut vfs = VirtualFileSystem::new();

        let result = vfs
            .apply_position_edits(
                &file,
                &[
                    PositionEdit {
                        start: Position { row: 1, column: 11 },
                        end: Position { row: 1, column: 13 },
                        new_text: "20".to_string(),
                    },
                    PositionEdit {
                        start: Position { row: 1, column: 0 },
                        end: Position { row: 1, column: 0 },
                        new_text: "# staged\n".to_string(),
                    },
                ],
            )
            .unwrap();

        assert!(result.source.contains("return 20"));
        assert!(result.source.contains("# staged"));
        assert!(result.dirty);
    }

    #[test]
    fn empty_position_edits_report_current_syntax_errors() {
        let file = temp_file("def value(\n");
        let mut vfs = VirtualFileSystem::new();

        let result = vfs.apply_position_edits(&file, &[]).unwrap();

        assert!(result.incremental_parse);
        assert!(!result.validation.syntax_errors.is_empty());
        assert!(result.validation.resolved_identifiers.is_empty());
        assert_eq!(result.validation.commit_gate.status, "not_evaluated");
    }

    #[test]
    fn rolls_back_position_edits_when_later_edit_fails() {
        let file = temp_file("def value() -> int:\n    return 10\n");
        let mut vfs = VirtualFileSystem::new();
        let initial = vfs.read_file(&file).unwrap();

        let error = vfs
            .apply_position_edits(
                &file,
                &[
                    PositionEdit {
                        start: Position { row: 1, column: 11 },
                        end: Position { row: 1, column: 13 },
                        new_text: "20".to_string(),
                    },
                    PositionEdit {
                        start: Position { row: 99, column: 0 },
                        end: Position { row: 99, column: 0 },
                        new_text: "# bad\n".to_string(),
                    },
                ],
            )
            .expect_err("later edit failure should reject the whole batch");

        assert!(
            error
                .to_string()
                .contains("failed to apply position edit at index 1")
        );
        let snapshot = vfs.read_file(&file).unwrap();
        assert_eq!(snapshot.source, initial.source);
        assert_eq!(snapshot.version, initial.version);
        assert_eq!(snapshot.dirty, initial.dirty);
    }

    #[test]
    fn rolls_back_position_edits_when_later_edit_splits_utf8_character() {
        let file = temp_file("def value() -> str:\n    return 'é'\n");
        let mut vfs = VirtualFileSystem::new();
        let initial = vfs.read_file(&file).unwrap();

        let error = vfs
            .apply_position_edits(
                &file,
                &[
                    PositionEdit {
                        start: Position { row: 0, column: 0 },
                        end: Position { row: 0, column: 0 },
                        new_text: "# staged\n".to_string(),
                    },
                    PositionEdit {
                        start: Position { row: 2, column: 13 },
                        end: Position { row: 2, column: 13 },
                        new_text: "x".to_string(),
                    },
                ],
            )
            .expect_err("position edits must not split UTF-8 characters");

        assert!(
            error
                .to_string()
                .contains("failed to apply position edit at index 1")
        );
        let error_chain = format!("{error:#}");
        assert!(error_chain.contains("does not align to a UTF-8 character boundary"));
        let snapshot = vfs.read_file(&file).unwrap();
        assert_eq!(snapshot.source, initial.source);
        assert_eq!(snapshot.version, initial.version);
        assert_eq!(snapshot.dirty, initial.dirty);
    }

    #[test]
    fn closes_virtual_file_without_persisting_changes() {
        let file = temp_file("def value() -> int:\n    return 1\n");
        let mut vfs = VirtualFileSystem::new();

        vfs.open_file(&file, Some("def value() -> int:\n    return 8\n"))
            .unwrap();
        let snapshot = vfs.close_file(&file, false).unwrap();

        assert!(!snapshot.dirty);
        assert!(snapshot.source.contains("return 1"));
        assert!(vfs.virtual_file_statuses(false).unwrap().is_empty());
        assert!(fs::read_to_string(&file).unwrap().contains("return 1"));
    }

    #[test]
    fn traces_symbol_graph_from_unsaved_virtual_changes() {
        let workspace = temp_workspace();
        let helper_path = workspace.join("helper.py");
        let caller_path = workspace.join("caller.py");

        fs::write(
            &helper_path,
            "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();
        fs::write(
            &caller_path,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        let mut vfs = VirtualFileSystem::new();
        vfs.patch_node(
            &helper_path,
            "helper",
            "def helper(value: int) -> int:\n    return branch(value)\n",
            None,
        )
        .unwrap();

        let trace = vfs
            .trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
            .unwrap();
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "branch")
        );
        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "leaf")
        );
        assert!(
            fs::read_to_string(&helper_path)
                .unwrap()
                .contains("return leaf")
        );
    }

    #[test]
    fn trace_symbol_graph_ignores_virtual_files_in_skipped_dirs() {
        let workspace = temp_workspace();
        let helper_path = workspace.join("helper.py");
        let venv_path = workspace.join("VENV").join("installed.py");

        fs::create_dir_all(venv_path.parent().unwrap()).unwrap();
        fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

        let mut vfs = VirtualFileSystem::new();
        vfs.open_file(&venv_path, Some("def installed() -> int:\n    return 2\n"))
            .unwrap();

        assert!(
            vfs.trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
                .is_ok()
        );
        assert!(
            vfs.trace_symbol_graph(&workspace, "installed", TraceDirection::Both)
                .is_err()
        );
    }

    #[test]
    fn trace_symbol_graph_ignores_virtual_files_in_sibling_workspace_prefix() {
        let dir = temp_workspace();
        let workspace = dir.join("project");
        let sibling = dir.join("project-extra");
        let helper_path = workspace.join("helper.py");
        let sibling_path = sibling.join("installed.py");

        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

        let mut vfs = VirtualFileSystem::new();
        vfs.open_file(
            &sibling_path,
            Some("def installed() -> int:\n    return 2\n"),
        )
        .unwrap();

        assert!(
            vfs.trace_symbol_graph(&workspace, "helper", TraceDirection::Both)
                .is_ok()
        );
        assert!(
            vfs.trace_symbol_graph(&workspace, "installed", TraceDirection::Both)
                .is_err()
        );
    }

    #[test]
    fn commits_refresh_registered_symbol_index() {
        let workspace = temp_workspace();
        let helper_path = workspace.join("helper.py");
        let caller_path = workspace.join("caller.py");
        let db_path = workspace.join("symbols.db");

        fs::write(
            &helper_path,
            "def helper(value: int) -> int:\n    return leaf(value)\n\n\ndef leaf(value: int) -> int:\n    return value + 1\n\n\ndef branch(value: int) -> int:\n    return value + 2\n",
        )
        .unwrap();
        fs::write(
            &caller_path,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        let mut vfs = VirtualFileSystem::new();
        let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
        assert_eq!(stats.indexed_files, 2);
        assert_eq!(stats.reused_files, 0);
        assert_eq!(vfs.registered_symbol_indexes().len(), 1);

        vfs.patch_node(
            &helper_path,
            "helper",
            "def helper(value: int) -> int:\n    return branch(value)\n",
            None,
        )
        .unwrap();
        vfs.commit_file(&helper_path).unwrap();

        let trace =
            trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).unwrap();
        assert!(
            trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "branch")
        );
        assert!(
            !trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "leaf")
        );

        assert!(vfs.unregister_symbol_index(&workspace).unwrap());
        assert!(vfs.registered_symbol_indexes().is_empty());
    }

    #[test]
    fn commits_new_file_refresh_registered_symbol_index() {
        let workspace = temp_workspace();
        let helper_path = workspace.join("helper.py");
        let caller_path = workspace.join("caller.py");
        let db_path = workspace.join("symbols.db");

        fs::write(
            &caller_path,
            "def orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        let mut vfs = VirtualFileSystem::new();
        let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
        assert_eq!(stats.indexed_files, 1);

        let initial_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(initial_trace.callees.is_empty());

        vfs.open_file(
            &helper_path,
            Some("def helper(value: int) -> int:\n    return value + 1\n"),
        )
        .unwrap();
        vfs.commit_file(&helper_path).unwrap();

        let updated_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(
            updated_trace
                .callees
                .iter()
                .any(|symbol| symbol.semantic_path == "helper")
        );
    }

    #[test]
    fn commits_clean_deleted_file_refresh_registered_symbol_index() {
        let workspace = temp_workspace();
        let helper_path = workspace.join("helper.py");
        let caller_path = workspace.join("caller.py");
        let db_path = workspace.join("symbols.db");

        fs::write(
            &helper_path,
            "def helper(value: int) -> int:\n    return value + 1\n",
        )
        .unwrap();
        fs::write(
            &caller_path,
            "from helper import helper\n\n\ndef orchestrate(value: int) -> int:\n    return helper(value)\n",
        )
        .unwrap();

        let mut vfs = VirtualFileSystem::new();
        let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
        assert_eq!(stats.indexed_files, 2);
        vfs.read_file(&helper_path).unwrap();

        fs::remove_file(&helper_path).unwrap();
        let committed = vfs.commit_file(&helper_path).unwrap();

        assert_eq!(committed.source, "");
        assert!(!committed.dirty);
        assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_err());
        let updated_trace =
            trace_symbol_graph_from_index(&db_path, "orchestrate", TraceDirection::Both).unwrap();
        assert!(updated_trace.callees.is_empty());
    }

    #[test]
    fn commits_skip_registered_index_refresh_for_ignored_dirs() {
        let workspace = temp_workspace();
        let helper_path = workspace.join("helper.py");
        let venv_path = workspace.join("VENV").join("installed.py");
        let db_path = workspace.join("symbols.db");

        fs::create_dir_all(venv_path.parent().unwrap()).unwrap();
        fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

        let mut vfs = VirtualFileSystem::new();
        let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
        assert_eq!(stats.indexed_files, 1);

        vfs.open_file(&venv_path, Some("def installed() -> int:\n    return 2\n"))
            .unwrap();
        vfs.commit_file(&venv_path).unwrap();

        assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
        assert!(
            trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err()
        );
    }

    #[test]
    fn commit_skips_registered_index_refresh_for_sibling_workspace_prefix() {
        let dir = temp_workspace();
        let workspace = dir.join("project");
        let sibling = dir.join("project-extra");
        let helper_path = workspace.join("helper.py");
        let sibling_path = sibling.join("installed.py");
        let db_path = workspace.join("symbols.db");

        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&sibling).unwrap();
        fs::write(&helper_path, "def helper() -> int:\n    return 1\n").unwrap();

        let mut vfs = VirtualFileSystem::new();
        let stats = vfs.register_symbol_index(&workspace, &db_path).unwrap();
        assert_eq!(stats.indexed_files, 1);

        vfs.open_file(
            &sibling_path,
            Some("def installed() -> int:\n    return 2\n"),
        )
        .unwrap();
        vfs.commit_file(&sibling_path).unwrap();

        assert!(trace_symbol_graph_from_index(&db_path, "helper", TraceDirection::Both).is_ok());
        assert!(
            trace_symbol_graph_from_index(&db_path, "installed", TraceDirection::Both).is_err()
        );
    }

    fn temp_file(contents: &str) -> std::path::PathBuf {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(format!("arborist-vfs-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join(Path::new("buffer.py"));
        fs::write(&file, contents).unwrap();
        file
    }

    fn temp_workspace() -> std::path::PathBuf {
        let suffix = format!(
            "{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(format!("arborist-vfs-workspace-{suffix}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
