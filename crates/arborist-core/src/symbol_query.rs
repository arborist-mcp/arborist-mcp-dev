use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::language::{ensure_path_inside_workspace, normalize_absolute_path};
use crate::model::{
    Position, SymbolContextResult, SymbolListContextResult, SymbolListDiscoveryContextResult,
    SymbolListNeighborhoodContextResult, SymbolListResult, SymbolNeighborhoodContextResult,
    SymbolReadDiscoveryContextResult, SymbolReadResult, SymbolSearchContextResult,
    SymbolSearchDiscoveryContextResult, SymbolSearchNeighborhoodContextResult, SymbolSearchResult,
    TraceDirection, TraceSymbolGraphResult, TraceSymbolNeighborhoodResult,
};
use crate::source_overlay::source_override_for_path;
use crate::symbols;

#[derive(Debug, Clone)]
enum SymbolQueryBackend {
    Workspace(PathBuf),
    Index(PathBuf),
}

#[derive(Debug, Clone)]
pub struct SymbolQueryContext {
    backend: SymbolQueryBackend,
    file_overrides: BTreeMap<String, String>,
}

impl SymbolQueryContext {
    pub fn workspace(workspace_root: &Path) -> Result<Self> {
        Ok(Self {
            backend: SymbolQueryBackend::Workspace(normalize_absolute_path(workspace_root)?),
            file_overrides: BTreeMap::new(),
        })
    }

    pub fn index(db_path: &Path) -> Result<Self> {
        Ok(Self {
            backend: SymbolQueryBackend::Index(normalize_absolute_path(db_path)?),
            file_overrides: BTreeMap::new(),
        })
    }

    pub fn with_source_overlay(mut self, file_path: &Path, source: &str) -> Result<Self> {
        self.add_source_overlay(file_path, source)?;
        Ok(self)
    }

    pub fn add_source_overlay(&mut self, file_path: &Path, source: &str) -> Result<()> {
        let (file_path, file_override) = source_override_for_path(file_path, source)?;
        if let SymbolQueryBackend::Workspace(workspace_root) = &self.backend {
            ensure_path_inside_workspace(workspace_root, &file_path)?;
        }
        self.file_overrides.extend(file_override);
        Ok(())
    }

    pub fn trace_symbol_graph(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::trace_symbol_graph_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                )
            },
            |db_path, overrides| {
                symbols::trace_symbol_graph_from_index_with_overrides(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                )
            },
        )
    }

    pub fn trace_symbol_graph_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
    ) -> Result<TraceSymbolGraphResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::trace_symbol_graph_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                )
            },
            |db_path, overrides| {
                symbols::trace_symbol_graph_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction,
                )
            },
        )
    }

    pub fn trace_symbol_neighborhood(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::trace_symbol_neighborhood_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::trace_symbol_neighborhood_from_index_with_overrides(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
        )
    }

    pub fn trace_symbol_neighborhood_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<TraceSymbolNeighborhoodResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::trace_symbol_neighborhood_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::trace_symbol_neighborhood_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction, max_depth, max_nodes,
                )
            },
        )
    }

    pub fn read_symbol(&self, symbol_path: &str) -> Result<SymbolReadResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_with_overrides(workspace_root, overrides, symbol_path)
            },
            |db_path, overrides| {
                symbols::read_symbol_from_index_with_overrides(db_path, overrides, symbol_path)
            },
        )
    }

    pub fn read_symbol_at_position(
        &self,
        file_path: &Path,
        position: &Position,
    ) -> Result<SymbolReadResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position,
                )
            },
        )
    }

    pub fn read_symbol_context(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_context_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_context_from_index_with_overrides(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                )
            },
        )
    }

    pub fn read_symbol_context_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
    ) -> Result<SymbolContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_context_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_context_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction,
                )
            },
        )
    }

    pub fn read_symbol_neighborhood_context(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_neighborhood_context_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_neighborhood_context_from_index_with_overrides(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
        )
    }

    pub fn read_symbol_neighborhood_context_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_neighborhood_context_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_neighborhood_context_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction, max_depth, max_nodes,
                )
            },
        )
    }

    pub fn read_symbol_discovery_context(
        &self,
        symbol_path: &str,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_discovery_context_with_overrides(
                    workspace_root,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_discovery_context_from_index_with_overrides(
                    db_path,
                    overrides,
                    symbol_path,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
        )
    }

    pub fn read_symbol_discovery_context_at_position(
        &self,
        file_path: &Path,
        position: &Position,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<SymbolReadDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::read_symbol_discovery_context_at_position_with_overrides(
                    workspace_root,
                    overrides,
                    file_path,
                    position,
                    direction,
                    max_depth,
                    max_nodes,
                )
            },
            |db_path, overrides| {
                symbols::read_symbol_discovery_context_at_position_from_index_with_overrides(
                    db_path, overrides, file_path, position, direction, max_depth, max_nodes,
                )
            },
        )
    }

    pub fn search_symbols(
        &self,
        query: &str,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::search_symbols_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    query,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::search_symbols_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    query,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    pub fn search_symbols_context(
        &self,
        query: &str,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::search_symbols_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    query,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::search_symbols_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    query,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_symbols_neighborhood_context(
        &self,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::search_symbols_neighborhood_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::search_symbols_neighborhood_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn search_symbols_discovery_context(
        &self,
        query: &str,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolSearchDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::search_symbols_discovery_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::search_symbols_discovery_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    query,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    pub fn list_symbols(
        &self,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::list_symbols_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::list_symbols_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    pub fn list_symbols_context(
        &self,
        limit: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::list_symbols_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::list_symbols_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    limit,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_neighborhood_context(
        &self,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListNeighborhoodContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::list_symbols_neighborhood_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::list_symbols_neighborhood_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn list_symbols_discovery_context(
        &self,
        limit: usize,
        direction: TraceDirection,
        max_depth: usize,
        max_nodes: usize,
        file_path_contains: Option<&str>,
        node_kind: Option<&str>,
    ) -> Result<SymbolListDiscoveryContextResult> {
        self.dispatch(
            |workspace_root, overrides| {
                symbols::list_symbols_discovery_context_with_overrides_filtered(
                    workspace_root,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
            |db_path, overrides| {
                symbols::list_symbols_discovery_context_from_index_with_overrides_filtered(
                    db_path,
                    overrides,
                    limit,
                    direction,
                    max_depth,
                    max_nodes,
                    file_path_contains,
                    node_kind,
                )
            },
        )
    }

    fn dispatch<T>(
        &self,
        workspace: impl FnOnce(&Path, &BTreeMap<String, String>) -> Result<T>,
        index: impl FnOnce(&Path, &BTreeMap<String, String>) -> Result<T>,
    ) -> Result<T> {
        match &self.backend {
            SymbolQueryBackend::Workspace(workspace_root) => {
                workspace(workspace_root, &self.file_overrides)
            }
            SymbolQueryBackend::Index(db_path) => index(db_path, &self.file_overrides),
        }
    }
}
