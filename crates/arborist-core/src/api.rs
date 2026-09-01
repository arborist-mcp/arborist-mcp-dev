use std::path::Path;

use anyhow::{Result, bail};

pub use crate::api_patch_validation::*;
pub use crate::api_source_query::*;
use crate::deadline::{CooperativeDeadline, DeadlineCheck};
use crate::language::read_source;
use crate::model::{MAX_SEMANTIC_EXPAND_NODES, SemanticSkeleton};
use crate::{language, semantic};

pub const MAX_SEMANTIC_SKELETON_DEPTH: usize = 64;
pub const MAX_SEMANTIC_SKELETON_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

pub fn get_semantic_skeleton_from_path(
    path: &Path,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    get_semantic_skeleton_from_path_with_timeout(path, depth_limit, expand_nodes, None)
}

pub fn get_semantic_skeleton_from_path_with_timeout(
    path: &Path,
    depth_limit: usize,
    expand_nodes: &[String],
    timeout_ms: Option<u64>,
) -> Result<SemanticSkeleton> {
    let deadline = CooperativeDeadline::new(
        timeout_ms,
        MAX_SEMANTIC_SKELETON_TIMEOUT_MS,
        "semantic skeleton",
    )?;
    let path = language::normalize_absolute_path(path)?;
    validate_depth_limit(depth_limit)?;
    validate_expand_nodes(expand_nodes)?;
    deadline.check("source read")?;
    let source = read_source(&path)?;
    deadline.check("source parse")?;
    get_semantic_skeleton_with_deadline(&path, &source, depth_limit, expand_nodes, &deadline)
}

pub fn get_semantic_skeleton(
    path: &Path,
    source: &str,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    get_semantic_skeleton_with_timeout(path, source, depth_limit, expand_nodes, None)
}

pub fn get_semantic_skeleton_with_timeout(
    path: &Path,
    source: &str,
    depth_limit: usize,
    expand_nodes: &[String],
    timeout_ms: Option<u64>,
) -> Result<SemanticSkeleton> {
    let deadline = CooperativeDeadline::new(
        timeout_ms,
        MAX_SEMANTIC_SKELETON_TIMEOUT_MS,
        "semantic skeleton",
    )?;
    let path = language::normalize_absolute_path(path)?;
    validate_depth_limit(depth_limit)?;
    validate_expand_nodes(expand_nodes)?;
    get_semantic_skeleton_with_deadline(&path, source, depth_limit, expand_nodes, &deadline)
}

fn get_semantic_skeleton_with_deadline(
    path: &Path,
    source: &str,
    depth_limit: usize,
    expand_nodes: &[String],
    deadline: &CooperativeDeadline,
) -> Result<SemanticSkeleton> {
    deadline.check("source parse")?;
    let document = language::parse_document_with_timeout(
        path,
        source,
        DeadlineCheck::remaining_timeout_micros(deadline, "source parse")?.unwrap_or(0),
    )?;
    deadline.check("semantic traversal")?;
    semantic::get_semantic_skeleton_with_deadline(
        path,
        document.language_id,
        source,
        &document.tree,
        depth_limit,
        expand_nodes,
        Some(deadline),
    )
}

fn validate_depth_limit(depth_limit: usize) -> Result<()> {
    if depth_limit > MAX_SEMANTIC_SKELETON_DEPTH {
        bail!("invalid depth_limit: expected at most {MAX_SEMANTIC_SKELETON_DEPTH}");
    }
    Ok(())
}

fn validate_expand_nodes(expand_nodes: &[String]) -> Result<()> {
    if expand_nodes.len() > MAX_SEMANTIC_EXPAND_NODES {
        bail!("invalid expand_nodes: expected at most {MAX_SEMANTIC_EXPAND_NODES} selectors");
    }
    if let Some(index) = expand_nodes
        .iter()
        .position(|selector| selector.trim().is_empty())
    {
        bail!("invalid expand_nodes selector at index {index}: selector must not be blank");
    }
    Ok(())
}
