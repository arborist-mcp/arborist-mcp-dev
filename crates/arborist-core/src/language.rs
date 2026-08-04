mod c;
mod go;
mod io;
mod javascript;
mod parser;
mod paths;
mod positions;
mod registry;
mod rust;
mod tree;

pub use c::{
    C_FAMILY_HEADER_EXTENSIONS, C_LANGUAGE_EXTENSIONS, CPP_LANGUAGE_EXTENSIONS,
    c_companion_source_path, c_include_targets, is_c_header_path, resolve_local_c_include,
};
pub(crate) use c::{
    c_include_targets_before, c_local_include_dependency_paths, extension_case_candidates,
};
pub(crate) use go::{go_local_package_dependency_paths, go_local_package_imports};
pub(crate) use javascript::{
    javascript_local_module_dependency_paths,
    javascript_named_import_module_paths_with_overrides_and_check,
    javascript_named_reexport_module_paths_with_overrides_and_check,
};
pub use parser::{
    ParsedDocument, detect_language, language_for_id, parse_document, parse_document_with_timeout,
    parser_for_language, supported_languages,
};
pub(crate) use parser::{validate_source_length, validate_source_size};
pub(crate) use paths::{ensure_path_inside_workspace, path_is_inside_workspace};
pub use paths::{normalize_absolute_path, normalize_path};
pub use positions::{offset_for_position, point_for_offset, position_from};
pub use registry::{
    LanguageCapabilities, LanguageDescriptor, LanguageRegistry, builtin_language_registry,
};
pub(crate) use rust::rust_local_module_dependency_paths;
pub use tree::*;

pub(crate) use io::write_source_atomic;
pub use io::{MAX_SOURCE_FILE_BYTES, read_source};

#[cfg(test)]
mod tests;
