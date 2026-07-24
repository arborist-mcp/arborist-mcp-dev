use anyhow::{Result, bail};

use super::{
    DEFAULT_TREE_QUERY_MAX_BYTES, DEFAULT_TREE_QUERY_TIMEOUT_MICROS, MAX_TREE_QUERY_TIMEOUT_MS,
};

pub(super) fn validate_tree_query(query: &str) -> Result<()> {
    if query.trim().is_empty() {
        bail!("invalid Tree-sitter query: query must not be blank");
    }
    if query.len() > DEFAULT_TREE_QUERY_MAX_BYTES {
        bail!(
            "invalid Tree-sitter query: query exceeds max query bytes ({})",
            DEFAULT_TREE_QUERY_MAX_BYTES
        );
    }

    Ok(())
}

pub(super) fn validate_max_captures(max_captures: usize) -> Result<()> {
    if max_captures == 0 {
        bail!("invalid Tree-sitter query max_captures: value must be greater than zero");
    }

    Ok(())
}

pub(super) fn validate_timeout(timeout_ms: Option<u64>) -> Result<u64> {
    let timeout_ms = timeout_ms.unwrap_or(DEFAULT_TREE_QUERY_TIMEOUT_MICROS / 1_000);
    if timeout_ms == 0 {
        bail!("invalid Tree-sitter query timeout_ms: value must be greater than zero");
    }
    if timeout_ms > MAX_TREE_QUERY_TIMEOUT_MS {
        bail!(
            "invalid Tree-sitter query timeout_ms: value must not exceed {}",
            MAX_TREE_QUERY_TIMEOUT_MS
        );
    }
    Ok(timeout_ms.saturating_mul(1_000))
}
