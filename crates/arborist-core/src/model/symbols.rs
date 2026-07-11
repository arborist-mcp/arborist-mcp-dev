use std::collections::BTreeSet;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::ensure_nonblank;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolMeta {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub evidence_key: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
    pub dependencies: Vec<String>,
    pub references: Vec<String>,
}

pub struct SymbolMetaInit {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
    pub dependencies: Vec<String>,
    pub references: Vec<String>,
}

impl SymbolMeta {
    pub fn new(init: SymbolMetaInit) -> Self {
        let evidence_key = symbol_evidence_key(
            &init.symbol_id,
            &init.file_path,
            &init.node_kind,
            &init.origin_type,
            init.byte_range,
            init.signature.as_deref(),
        );

        Self {
            symbol_id: init.symbol_id,
            semantic_path: init.semantic_path,
            scope_path: init.scope_path,
            file_path: init.file_path,
            node_kind: init.node_kind,
            origin_type: init.origin_type,
            evidence_key,
            byte_range: init.byte_range,
            signature: init.signature,
            parameters: init.parameters,
            return_type: init.return_type,
            docstring: init.docstring,
            dependencies: init.dependencies,
            references: init.references,
        }
    }

    pub fn with_origin_type(&self, origin_type: &str) -> Self {
        Self::new(SymbolMetaInit {
            symbol_id: self.symbol_id.clone(),
            semantic_path: self.semantic_path.clone(),
            scope_path: self.scope_path.clone(),
            file_path: self.file_path.clone(),
            node_kind: self.node_kind.clone(),
            origin_type: origin_type.to_string(),
            byte_range: self.byte_range,
            signature: self.signature.clone(),
            parameters: self.parameters.clone(),
            return_type: self.return_type.clone(),
            docstring: self.docstring.clone(),
            dependencies: self.dependencies.clone(),
            references: self.references.clone(),
        })
    }

    pub fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        validate_symbol_identity(
            SymbolIdentityRef {
                symbol_id: &self.symbol_id,
                semantic_path: &self.semantic_path,
                file_path: &self.file_path,
                node_kind: &self.node_kind,
                origin_type: &self.origin_type,
                evidence_key: &self.evidence_key,
                byte_range: self.byte_range,
                signature: self.signature.as_deref(),
            },
            field,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct SymbolSummary {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub evidence_key: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

pub struct SymbolSummaryInit {
    pub symbol_id: String,
    pub semantic_path: String,
    pub scope_path: Option<String>,
    pub file_path: String,
    pub node_kind: String,
    pub origin_type: String,
    pub byte_range: (usize, usize),
    pub signature: Option<String>,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
}

impl SymbolSummary {
    pub fn new(init: SymbolSummaryInit) -> Self {
        let evidence_key = symbol_evidence_key(
            &init.symbol_id,
            &init.file_path,
            &init.node_kind,
            &init.origin_type,
            init.byte_range,
            init.signature.as_deref(),
        );

        Self {
            symbol_id: init.symbol_id,
            semantic_path: init.semantic_path,
            scope_path: init.scope_path,
            file_path: init.file_path,
            node_kind: init.node_kind,
            origin_type: init.origin_type,
            evidence_key,
            byte_range: init.byte_range,
            signature: init.signature,
            parameters: init.parameters,
            return_type: init.return_type,
            docstring: init.docstring,
        }
    }

    pub fn validate_trace_replay_input(&self, field: &str) -> Result<()> {
        validate_symbol_identity(
            SymbolIdentityRef {
                symbol_id: &self.symbol_id,
                semantic_path: &self.semantic_path,
                file_path: &self.file_path,
                node_kind: &self.node_kind,
                origin_type: &self.origin_type,
                evidence_key: &self.evidence_key,
                byte_range: self.byte_range,
                signature: self.signature.as_deref(),
            },
            field,
        )
    }
}

pub(crate) fn ensure_unique_symbol_evidence_keys(
    symbols: &[SymbolSummary],
    field: &str,
) -> Result<()> {
    let mut seen = BTreeSet::new();
    for (index, symbol) in symbols.iter().enumerate() {
        if !seen.insert(symbol.evidence_key.clone()) {
            bail!("invalid {field}[{index}].evidence_key: duplicate evidence keys are not allowed");
        }
    }
    Ok(())
}

fn symbol_evidence_key(
    symbol_id: &str,
    file_path: &str,
    node_kind: &str,
    origin_type: &str,
    byte_range: (usize, usize),
    signature: Option<&str>,
) -> String {
    format!(
        "{symbol_id}|{file_path}|{node_kind}|{origin_type}|{}..{}|{}",
        byte_range.0,
        byte_range.1,
        signature.unwrap_or("")
    )
}

struct SymbolIdentityRef<'a> {
    symbol_id: &'a str,
    semantic_path: &'a str,
    file_path: &'a str,
    node_kind: &'a str,
    origin_type: &'a str,
    evidence_key: &'a str,
    byte_range: (usize, usize),
    signature: Option<&'a str>,
}

fn validate_symbol_identity(identity: SymbolIdentityRef<'_>, field: &str) -> Result<()> {
    ensure_nonblank(identity.symbol_id, &format!("{field}.symbol_id"))?;
    ensure_nonblank(identity.semantic_path, &format!("{field}.semantic_path"))?;
    ensure_nonblank(identity.file_path, &format!("{field}.file_path"))?;
    ensure_nonblank(identity.node_kind, &format!("{field}.node_kind"))?;
    ensure_nonblank(identity.origin_type, &format!("{field}.origin_type"))?;
    ensure_nonblank(identity.evidence_key, &format!("{field}.evidence_key"))?;
    if identity.byte_range.0 > identity.byte_range.1 {
        bail!("invalid {field}.byte_range: start byte is after end byte");
    }

    let expected = symbol_evidence_key(
        identity.symbol_id,
        identity.file_path,
        identity.node_kind,
        identity.origin_type,
        identity.byte_range,
        identity.signature,
    );
    if identity.evidence_key != expected {
        bail!("invalid {field}.evidence_key: expected evidence key to match symbol identity");
    }

    Ok(())
}
