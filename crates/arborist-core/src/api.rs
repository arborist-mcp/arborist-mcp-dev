use std::path::Path;

use anyhow::{Result, bail};

pub use crate::api_patch_validation::*;
pub use crate::api_source_query::*;
use crate::language::read_source;
use crate::model::SemanticSkeleton;
use crate::{language, semantic};

pub fn get_semantic_skeleton_from_path(
    path: &Path,
    depth_limit: usize,
    expand_nodes: &[String],
) -> Result<SemanticSkeleton> {
    let path = language::normalize_absolute_path(path)?;
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

fn validate_expand_nodes(expand_nodes: &[String]) -> Result<()> {
    if let Some(index) = expand_nodes
        .iter()
        .position(|selector| selector.trim().is_empty())
    {
        bail!("invalid expand_nodes selector at index {index}: selector must not be blank");
    }
    Ok(())
}
