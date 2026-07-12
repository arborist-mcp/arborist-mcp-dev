use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::{ensure_nonblank, ensure_nonblank_strings, point_is_after};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageId {
    Python,
    C,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub row: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PositionEdit {
    pub start: Position,
    pub end: Position,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SemanticSkeleton {
    pub file: String,
    pub skeleton: String,
    pub available_paths: Vec<String>,
    pub available_symbols: Vec<SemanticSkeletonSymbol>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(default, deny_unknown_fields)]
pub struct SemanticSkeletonSymbol {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub node_kind: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct QueryCaptureResult {
    pub capture_name: String,
    pub node_kind: String,
    pub text: String,
    pub owner_symbol_id: Option<String>,
    pub owner_semantic_path: Option<String>,
    pub owner_scope_path: Option<String>,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_point: Position,
    pub end_point: Position,
}

impl SemanticSkeleton {
    pub(crate) fn validate_public_output(&self) -> Result<()> {
        ensure_nonblank(&self.file, "skeleton.file")?;
        ensure_nonblank_strings(&self.available_paths, "skeleton.available_paths")?;
        if self.available_paths.len() != self.available_symbols.len() {
            bail!(
                "invalid skeleton.available_symbols: expected available_symbols to align with skeleton.available_paths"
            );
        }

        for (index, symbol) in self.available_symbols.iter().enumerate() {
            symbol.validate_public_output(index)?;
            if self.available_paths[index] != symbol.semantic_path {
                bail!(
                    "invalid skeleton.available_paths[{index}]: expected available_paths to match skeleton.available_symbols semantic paths"
                );
            }
        }

        Ok(())
    }
}

impl SemanticSkeletonSymbol {
    fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("skeleton.available_symbols[{index}]");
        ensure_nonblank(&self.symbol_id, &format!("{prefix}.symbol_id"))?;
        ensure_nonblank(&self.semantic_path, &format!("{prefix}.semantic_path"))?;
        if let Some(scope_path) = &self.scope_path {
            ensure_nonblank(scope_path, &format!("{prefix}.scope_path"))?;
        }
        ensure_nonblank(&self.node_kind, &format!("{prefix}.node_kind"))?;
        if self.byte_range.0 > self.byte_range.1 {
            bail!("invalid {prefix}.byte_range: start byte is after end byte");
        }
        if let Some(signature) = &self.signature {
            ensure_nonblank(signature, &format!("{prefix}.signature"))?;
        }
        ensure_nonblank_strings(&self.parameters, &format!("{prefix}.parameters"))?;
        if let Some(return_type) = &self.return_type {
            ensure_nonblank(return_type, &format!("{prefix}.return_type"))?;
        }
        if let Some(docstring) = &self.docstring {
            ensure_nonblank(docstring, &format!("{prefix}.docstring"))?;
        }
        Ok(())
    }
}

impl QueryCaptureResult {
    pub(crate) fn validate_public_output(&self, index: usize) -> Result<()> {
        let prefix = format!("query.captures[{index}]");
        ensure_nonblank(&self.capture_name, &format!("{prefix}.capture_name"))?;
        ensure_nonblank(&self.node_kind, &format!("{prefix}.node_kind"))?;
        if self.start_byte > self.end_byte {
            bail!("invalid {prefix}: start byte is after end byte");
        }
        if point_is_after(&self.start_point, &self.end_point) {
            bail!("invalid {prefix}: start point is after end point");
        }

        match (&self.owner_symbol_id, &self.owner_semantic_path) {
            (Some(owner_symbol_id), Some(owner_semantic_path)) => {
                ensure_nonblank(owner_symbol_id, &format!("{prefix}.owner_symbol_id"))?;
                ensure_nonblank(
                    owner_semantic_path,
                    &format!("{prefix}.owner_semantic_path"),
                )?;
            }
            (None, None) => {}
            _ => {
                bail!(
                    "invalid {prefix}: expected owner_symbol_id and owner_semantic_path to either both be present or both be absent"
                );
            }
        }

        if let Some(owner_scope_path) = &self.owner_scope_path {
            ensure_nonblank(owner_scope_path, &format!("{prefix}.owner_scope_path"))?;
            if self.owner_semantic_path.is_none() {
                bail!(
                    "invalid {prefix}.owner_scope_path: expected owner_scope_path only when owner_semantic_path is present"
                );
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TraceDirection {
    Callers,
    Callees,
    Both,
}
