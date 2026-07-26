mod c;
mod io;
mod parser;
mod paths;
mod positions;
mod tree;

pub use c::{
    C_FAMILY_HEADER_EXTENSIONS, C_HEADER_EXTENSIONS, C_SOURCE_EXTENSIONS, CPP_HEADER_EXTENSIONS,
    CPP_SOURCE_EXTENSIONS, c_companion_source_path, c_include_targets, c_local_include_targets,
    is_c_header_path, resolve_local_c_include,
};
pub(crate) use c::{c_include_targets_before, extension_case_candidates};
pub use parser::{
    ParsedDocument, detect_language, language_for_id, parse_document, parse_document_with_timeout,
    parser_for_language, supported_languages,
};
pub(crate) use parser::{validate_source_length, validate_source_size};
pub(crate) use paths::{ensure_path_inside_workspace, path_is_inside_workspace};
pub use paths::{normalize_absolute_path, normalize_path};
pub use positions::{offset_for_position, point_for_offset, position_from};
pub use tree::*;

pub(crate) use io::write_source_atomic;
pub use io::{MAX_SOURCE_FILE_BYTES, read_source};

#[cfg(test)]
mod tests;
