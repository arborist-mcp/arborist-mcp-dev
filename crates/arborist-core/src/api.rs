use std::path::Path;

use anyhow::{Result, bail};

pub use crate::api_patch_validation::*;
pub use crate::api_source_query::*;
use crate::language::read_source;
use crate::model::{MAX_SEMANTIC_EXPAND_NODES, SemanticSkeleton};
use crate::{language, semantic};

pub const MAX_SEMANTIC_SKELETON_DEPTH: usize = 64;

pub fn get_semantic_skeleton_from_path(
    path: &Path,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    let path = language::normalize_absolute_path(path)?;
    validate_depth_limit(depth_limit)?;
    validate_expand_nodes(expand_nodes)?;
    let source = read_source(&path)?;
    get_semantic_skeleton(&path, &source, depth_limit, expand_nodes)
}

pub fn get_semantic_skeleton(
    path: &Path,
    source: &str,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    let path = language::normalize_absolute_path(path)?;
    validate_depth_limit(depth_limit)?;
    validate_expand_nodes(expand_nodes)?;
    let document = language::parse_document(&path, source)?;
    semantic::get_semantic_skeleton(
        &path,
        document.language_id,
        source,
        &document.tree,
        depth_limit,
        expand_nodes,
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
