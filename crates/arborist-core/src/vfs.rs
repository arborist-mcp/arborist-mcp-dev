use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use tree_sitter::{InputEdit, Tree};

use crate::language::{
    ensure_path_inside_workspace, normalize_absolute_path, normalize_path, offset_for_position,
    parse_document, parser_for_language, path_is_inside_workspace, point_for_offset,
};
use crate::model::LanguageId;
use crate::model::{
    DiscoveryContextPatchResult, GraphBackedPatchResult, NeighborhoodContextPatchResult,
    PatchAstNodeResult, PatchValidationReport, PositionEdit, RegisteredSymbolIndex,
    SymbolContextResult, SymbolIndexStats, SymbolListContextResult,
    SymbolListDiscoveryContextResult, SymbolListNeighborhoodContextResult, SymbolListResult,
    SymbolNeighborhoodContextResult, SymbolReadDiscoveryContextResult, SymbolReadResult,
    SymbolSearchContextResult, SymbolSearchDiscoveryContextResult,
    SymbolSearchNeighborhoodContextResult, SymbolSearchResult, TraceBackedPatchResult,
    TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult, VirtualEditResult,
    VirtualFileSnapshot, VirtualFileStatus,
};
use crate::patching::{
    build_patch_result, collect_syntax_errors, semantic_target_at_position, semantic_target_range,
    splice_source, validate_bypass_reason, validate_patch_replacement,
};
use crate::symbols::{
    list_symbols_context_with_overrides_filtered,
    list_symbols_discovery_context_with_overrides_filtered,
    list_symbols_neighborhood_context_with_overrides_filtered,
    list_symbols_with_overrides_filtered, read_symbol_at_position_with_overrides,
    read_symbol_context_at_position_with_overrides, read_symbol_context_with_overrides,
    read_symbol_discovery_context_at_position_with_overrides,
    read_symbol_discovery_context_with_overrides,
    read_symbol_neighborhood_context_at_position_with_overrides,
    read_symbol_neighborhood_context_with_overrides, read_symbol_with_overrides,
    rebuild_symbol_index, refresh_symbol_index_for_file,
    search_symbols_context_with_overrides_filtered,
    search_symbols_discovery_context_with_overrides_filtered,
    search_symbols_neighborhood_context_with_overrides_filtered,
    search_symbols_with_overrides_filtered, trace_symbol_graph_at_position_with_overrides,
    trace_symbol_graph_with_overrides, trace_symbol_neighborhood_at_position_with_overrides,
    trace_symbol_neighborhood_with_overrides,
};
use crate::{
    validate_discovery_context_patch_result, validate_graph_backed_patch_result,
    validate_neighborhood_context_patch_result, validate_patch_commit_with_trace,
    validate_trace_backed_patch_result,
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

    pub fn patch_node_at_position(
        &mut self,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
    ) -> Result<PatchAstNodeResult> {
        let (path, normalized) = normalized_virtual_path(path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let semantic_target = {
            let entry = self
                .entries
                .get(&normalized)
                .ok_or_else(|| anyhow!("virtual file not loaded: {normalized}"))?;
            semantic_target_at_position(&entry.path, &entry.source, position)?
        };

        self.patch_node(&path, &semantic_target, new_code, bypass_reason)
    }

    pub fn validate_patch_with_trace_context(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
    ) -> Result<TraceBackedPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        self.trace_backed_patch_result(&workspace_root, &patch, direction)
    }

    pub fn validate_patch_with_trace_context_at_position(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
    ) -> Result<TraceBackedPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        self.trace_backed_patch_result(&workspace_root, &patch, direction)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_graph_context(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<GraphBackedPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        self.graph_backed_patch_result(&workspace_root, &patch, direction, max_depth, max_nodes)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_graph_context_at_position(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<GraphBackedPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        self.graph_backed_patch_result(&workspace_root, &patch, direction, max_depth, max_nodes)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_neighborhood_context(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<NeighborhoodContextPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        self.neighborhood_context_patch_result(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_neighborhood_context_at_position(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<NeighborhoodContextPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        self.neighborhood_context_patch_result(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_discovery_context(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        semantic_target: &str,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<DiscoveryContextPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node(&path, semantic_target, new_code, bypass_reason)?;
        self.discovery_context_patch_result(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn validate_patch_with_discovery_context_at_position(
        &mut self,
        workspace_root: &Path,
        path: &Path,
        position: &crate::model::Position,
        new_code: &str,
        bypass_reason: Option<&str>,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<DiscoveryContextPatchResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let (path, normalized) = normalized_virtual_path(path)?;
        ensure_path_inside_workspace(&workspace_root, &path)?;
        self.ensure_loaded(&path, None)?;
        self.refresh_if_clean(&normalized)?;

        let patch = self.patch_node_at_position(&path, position, new_code, bypass_reason)?;
        self.discovery_context_patch_result(
            &workspace_root,
            &patch,
            direction,
            max_depth,
            max_nodes,
        )
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

    pub fn trace_symbol_graph_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_graph_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
        )
    }

    pub fn trace_symbol_neighborhood_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        trace_symbol_neighborhood_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
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

    pub fn read_symbol_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
    ) -> Result<SymbolReadResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_at_position_with_overrides(&workspace_root, &overrides, file_path, position)
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

    pub fn read_symbol_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
        )
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

    pub fn read_symbol_neighborhood_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_neighborhood_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn read_symbol_discovery_context(
        &mut self,
        workspace_root: &Path,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_discovery_context_with_overrides(
            &workspace_root,
            &overrides,
            symbol_path,
            direction,
            max_depth,
            max_nodes,
        )
    }

    pub fn read_symbol_discovery_context_at_position(
        &mut self,
        workspace_root: &Path,
        file_path: &Path,
        position: &crate::model::Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        let workspace_root = normalize_absolute_path(workspace_root)?;
        let overrides = self.virtual_overrides_for_workspace(&workspace_root)?;
        read_symbol_discovery_context_at_position_with_overrides(
            &workspace_root,
            &overrides,
            file_path,
            position,
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
            if path_is_inside_workspace(workspace_root_path, &file_path)? {
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
            if path_is_inside_workspace(workspace_root, &absolute_path)? {
                overrides.insert(normalize_path(&absolute_path), entry.source.clone());
            }
        }

        Ok(overrides)
    }

    fn trace_backed_patch_result(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
    ) -> Result<TraceBackedPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            let result = TraceBackedPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
                ),
            };
            validate_trace_backed_patch_result(&result)?;
            return Ok(result);
        }

        if !patch.applied {
            let result = TraceBackedPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_trace_backed_patch_result(&result)?;
            return Ok(result);
        }

        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
        )?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = TraceBackedPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        validate_trace_backed_patch_result(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn graph_backed_patch_result(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<GraphBackedPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            let result = GraphBackedPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                neighborhood: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
                ),
            };
            validate_graph_backed_patch_result(&result)?;
            return Ok(result);
        }

        if !patch.applied {
            let result = GraphBackedPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                neighborhood: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_graph_backed_patch_result(&result)?;
            return Ok(result);
        }

        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
        )?;
        let neighborhood = trace_symbol_neighborhood_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
        )?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = GraphBackedPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            neighborhood: Some(neighborhood),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        validate_graph_backed_patch_result(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn neighborhood_context_patch_result(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<NeighborhoodContextPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            let result = NeighborhoodContextPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                neighborhood_context: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
                ),
            };
            validate_neighborhood_context_patch_result(&result)?;
            return Ok(result);
        }

        if !patch.applied {
            let result = NeighborhoodContextPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                neighborhood_context: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_neighborhood_context_patch_result(&result)?;
            return Ok(result);
        }

        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
        )?;
        let neighborhood_context = read_symbol_neighborhood_context_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
        )?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = NeighborhoodContextPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            neighborhood_context: Some(neighborhood_context),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        validate_neighborhood_context_patch_result(&result)?;
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    fn discovery_context_patch_result(
        &mut self,
        workspace_root: &Path,
        patch: &PatchAstNodeResult,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<DiscoveryContextPatchResult> {
        let trace_target = patch.resolved_symbol_id.clone();
        if !patch.validation.syntax_errors.is_empty() {
            let result = DiscoveryContextPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                read: None,
                neighborhood_context: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_syntax_errors().to_string(),
                ),
            };
            validate_discovery_context_patch_result(&result)?;
            return Ok(result);
        }

        if !patch.applied {
            let result = DiscoveryContextPatchResult {
                patch: patch.clone(),
                trace_target,
                trace: None,
                read: None,
                neighborhood_context: None,
                trace_validation: None,
                trace_error: Some(
                    TraceBackedPatchResult::trace_skip_reason_for_patch_gate_rejection()
                        .to_string(),
                ),
            };
            validate_discovery_context_patch_result(&result)?;
            return Ok(result);
        }

        let mut overrides = self.virtual_overrides_for_workspace(workspace_root)?;
        overrides.insert(patch.file.clone(), patch.updated_source.clone());
        let trace = trace_symbol_graph_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
        )?;
        let read = read_symbol_with_overrides(workspace_root, &overrides, &trace_target)?;
        let neighborhood_context = read_symbol_neighborhood_context_with_overrides(
            workspace_root,
            &overrides,
            &trace_target,
            direction,
            max_depth,
            max_nodes,
        )?;
        let trace_validation = validate_patch_commit_with_trace(patch, &trace)?;
        let result = DiscoveryContextPatchResult {
            patch: patch.clone(),
            trace_target,
            trace: Some(trace),
            read: Some(read),
            neighborhood_context: Some(neighborhood_context),
            trace_validation: Some(trace_validation),
            trace_error: None,
        };
        validate_discovery_context_patch_result(&result)?;
        Ok(result)
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
mod tests;
