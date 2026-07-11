use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::super::{
    SymbolSummary, ensure_nonblank, ensure_nonblank_strings, ensure_unique_strings,
    ensure_unique_symbol_evidence_keys,
};
use super::{SymbolNeighborhoodContextResult, SymbolReadResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolListResult {
    pub indexed_files: usize,
    pub total_symbols: usize,
    pub truncated: bool,
    pub symbols: Vec<SymbolSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolListContextResult {
    pub list: SymbolListResult,
    pub reads: Vec<SymbolReadResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolListNeighborhoodContextResult {
    pub list: SymbolListResult,
    pub contexts: Vec<SymbolNeighborhoodContextResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolListDiscoveryContextResult {
    pub list: SymbolListResult,
    pub reads: Vec<SymbolReadResult>,
    pub contexts: Vec<SymbolNeighborhoodContextResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchResult {
    pub query: String,
    pub indexed_files: usize,
    pub total_matches: usize,
    pub truncated: bool,
    pub matches: Vec<SymbolSummary>,
    pub match_details: Vec<SymbolSearchMatchDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchContextResult {
    pub search: SymbolSearchResult,
    pub reads: Vec<SymbolReadResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchNeighborhoodContextResult {
    pub search: SymbolSearchResult,
    pub contexts: Vec<SymbolNeighborhoodContextResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchDiscoveryContextResult {
    pub search: SymbolSearchResult,
    pub reads: Vec<SymbolReadResult>,
    pub contexts: Vec<SymbolNeighborhoodContextResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSearchMatchDetail {
    pub symbol_id: String,
    pub score: usize,
    pub matched_fields: Vec<String>,
}

impl SymbolListResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        if self.total_symbols < self.symbols.len() {
            bail!(
                "invalid symbol_list.total_symbols: expected total_symbols to be at least symbols.len()"
            );
        }
        if self.truncated != (self.total_symbols > self.symbols.len()) {
            bail!(
                "invalid symbol_list.truncated: expected truncated to match whether total_symbols exceeds symbols.len()"
            );
        }
        for (index, item) in self.symbols.iter().enumerate() {
            item.validate_trace_replay_input(&format!("symbol_list.symbols[{index}]"))?;
        }
        ensure_unique_symbol_evidence_keys(&self.symbols, "symbol_list.symbols")
    }
}

impl SymbolSearchResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.query, "symbol_search.query")?;
        if self.total_matches < self.matches.len() {
            bail!(
                "invalid symbol_search.total_matches: expected total_matches to be at least matches.len()"
            );
        }
        if self.truncated != (self.total_matches > self.matches.len()) {
            bail!(
                "invalid symbol_search.truncated: expected truncated to match whether total_matches exceeds matches.len()"
            );
        }
        if self.matches.len() != self.match_details.len() {
            bail!(
                "invalid symbol_search.match_details: expected match_details to align with matches"
            );
        }
        for (index, item) in self.matches.iter().enumerate() {
            item.validate_trace_replay_input(&format!("symbol_search.matches[{index}]"))?;
            self.match_details[index].validate_public_output(index, &item.symbol_id)?;
        }
        ensure_unique_symbol_evidence_keys(&self.matches, "symbol_search.matches")
    }
}

impl SymbolSearchContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.search.validate_public_output()?;
        if self.reads.len() != self.search.matches.len() {
            bail!(
                "invalid symbol_search_context.reads: expected reads to align with search.matches"
            );
        }

        for (index, read) in self.reads.iter().enumerate() {
            read.validate_public_output()?;
            let symbol = &self.search.matches[index];
            if read.indexed_files != self.search.indexed_files {
                bail!(
                    "invalid symbol_search_context.reads[{index}].indexed_files: expected indexed_files to match search.indexed_files"
                );
            }
            if read.symbol.symbol_id != symbol.symbol_id {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.symbol_id: expected reads to align with search.matches"
                );
            }
            if read.symbol.semantic_path != symbol.semantic_path {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.semantic_path: expected reads to align with search.matches"
                );
            }
            if read.symbol.file_path != symbol.file_path {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.file_path: expected reads to align with search.matches"
                );
            }
            if read.symbol.node_kind != symbol.node_kind {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.node_kind: expected reads to align with search.matches"
                );
            }
            if read.symbol.byte_range != symbol.byte_range {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.byte_range: expected reads to align with search.matches"
                );
            }
            if read.symbol.signature != symbol.signature {
                bail!(
                    "invalid symbol_search_context.reads[{index}].symbol.signature: expected reads to align with search.matches"
                );
            }
        }

        Ok(())
    }
}

impl SymbolSearchNeighborhoodContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.search.validate_public_output()?;
        if self.contexts.len() != self.search.matches.len() {
            bail!(
                "invalid symbol_search_neighborhood_context.contexts: expected contexts to align with search.matches"
            );
        }

        for (index, context) in self.contexts.iter().enumerate() {
            context.validate_public_output()?;
            let symbol = &self.search.matches[index];
            let root = &context.neighborhood.symbol;
            if context.neighborhood.indexed_files != self.search.indexed_files {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.indexed_files: expected indexed_files to match search.indexed_files"
                );
            }
            if root.symbol_id != symbol.symbol_id {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.symbol_id: expected contexts to align with search.matches"
                );
            }
            if root.semantic_path != symbol.semantic_path {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.semantic_path: expected contexts to align with search.matches"
                );
            }
            if root.file_path != symbol.file_path {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.file_path: expected contexts to align with search.matches"
                );
            }
            if root.node_kind != symbol.node_kind {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.node_kind: expected contexts to align with search.matches"
                );
            }
            if root.byte_range != symbol.byte_range {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.byte_range: expected contexts to align with search.matches"
                );
            }
            if root.signature != symbol.signature {
                bail!(
                    "invalid symbol_search_neighborhood_context.contexts[{index}].neighborhood.symbol.signature: expected contexts to align with search.matches"
                );
            }
        }

        Ok(())
    }
}

impl SymbolListContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.list.validate_public_output()?;
        if self.reads.len() != self.list.symbols.len() {
            bail!("invalid symbol_list_context.reads: expected reads to align with list.symbols");
        }

        for (index, read) in self.reads.iter().enumerate() {
            read.validate_public_output()?;
            let symbol = &self.list.symbols[index];
            if read.indexed_files != self.list.indexed_files {
                bail!(
                    "invalid symbol_list_context.reads[{index}].indexed_files: expected indexed_files to match list.indexed_files"
                );
            }
            if read.symbol.symbol_id != symbol.symbol_id {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.symbol_id: expected reads to align with list.symbols"
                );
            }
            if read.symbol.semantic_path != symbol.semantic_path {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.semantic_path: expected reads to align with list.symbols"
                );
            }
            if read.symbol.file_path != symbol.file_path {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.file_path: expected reads to align with list.symbols"
                );
            }
            if read.symbol.node_kind != symbol.node_kind {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.node_kind: expected reads to align with list.symbols"
                );
            }
            if read.symbol.byte_range != symbol.byte_range {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.byte_range: expected reads to align with list.symbols"
                );
            }
            if read.symbol.signature != symbol.signature {
                bail!(
                    "invalid symbol_list_context.reads[{index}].symbol.signature: expected reads to align with list.symbols"
                );
            }
        }

        Ok(())
    }
}

impl SymbolListNeighborhoodContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        self.list.validate_public_output()?;
        if self.contexts.len() != self.list.symbols.len() {
            bail!(
                "invalid symbol_list_neighborhood_context.contexts: expected contexts to align with list.symbols"
            );
        }

        for (index, context) in self.contexts.iter().enumerate() {
            context.validate_public_output()?;
            let symbol = &self.list.symbols[index];
            let root = &context.neighborhood.symbol;
            if context.neighborhood.indexed_files != self.list.indexed_files {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.indexed_files: expected indexed_files to match list.indexed_files"
                );
            }
            if root.symbol_id != symbol.symbol_id {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.symbol_id: expected contexts to align with list.symbols"
                );
            }
            if root.semantic_path != symbol.semantic_path {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.semantic_path: expected contexts to align with list.symbols"
                );
            }
            if root.file_path != symbol.file_path {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.file_path: expected contexts to align with list.symbols"
                );
            }
            if root.node_kind != symbol.node_kind {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.node_kind: expected contexts to align with list.symbols"
                );
            }
            if root.byte_range != symbol.byte_range {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.byte_range: expected contexts to align with list.symbols"
                );
            }
            if root.signature != symbol.signature {
                bail!(
                    "invalid symbol_list_neighborhood_context.contexts[{index}].neighborhood.symbol.signature: expected contexts to align with list.symbols"
                );
            }
        }

        Ok(())
    }
}

impl SymbolSearchDiscoveryContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        SymbolSearchContextResult {
            search: self.search.clone(),
            reads: self.reads.clone(),
        }
        .validate_public_output()?;
        SymbolSearchNeighborhoodContextResult {
            search: self.search.clone(),
            contexts: self.contexts.clone(),
        }
        .validate_public_output()?;
        Ok(())
    }
}

impl SymbolListDiscoveryContextResult {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        SymbolListContextResult {
            list: self.list.clone(),
            reads: self.reads.clone(),
        }
        .validate_public_output()?;
        SymbolListNeighborhoodContextResult {
            list: self.list.clone(),
            contexts: self.contexts.clone(),
        }
        .validate_public_output()?;
        Ok(())
    }
}

impl SymbolSearchMatchDetail {
    fn validate_public_output(&self, index: usize, expected_symbol_id: &str) -> Result<()> {
        let prefix = format!("symbol_search.match_details[{index}]");
        ensure_nonblank(&self.symbol_id, &format!("{prefix}.symbol_id"))?;
        if self.symbol_id != expected_symbol_id {
            bail!(
                "invalid {prefix}.symbol_id: expected match_details to align with matches symbol ids"
            );
        }
        if self.score == 0 {
            bail!("invalid {prefix}.score: expected score to be greater than zero");
        }
        ensure_nonblank_strings(&self.matched_fields, &format!("{prefix}.matched_fields"))?;
        ensure_unique_strings(&self.matched_fields, &format!("{prefix}.matched_fields"))?;
        for field in &self.matched_fields {
            match field.as_str() {
                "base_name" | "symbol_id" | "semantic_path" | "scope_path" | "file_path"
                | "node_kind" | "signature" | "parameters" | "return_type" | "docstring" => {}
                other => {
                    bail!("invalid {prefix}.matched_fields: unsupported field `{other}`");
                }
            }
        }
        Ok(())
    }
}
