pub(super) use std::fs;
pub(super) use std::path::Path;

pub(super) use super::support::temporary_dir;
pub(super) use super::{
    patch_ast_node, patch_ast_node_from_path, preview_patch_ast_node_from_path,
};

mod class_closure;
mod core;
mod expr_bindings;
mod imports;
mod io_bypass;
mod match_case;
mod replacement;
mod scope_bindings;
