mod c;
mod csharp;
mod go;
mod io;
mod java;
mod javascript;
mod kotlin;
mod parser;
mod paths;
mod positions;
mod python;
mod registry;
mod rust;
mod tree;

pub use c::{
    C_FAMILY_HEADER_EXTENSIONS, C_LANGUAGE_EXTENSIONS, CPP_LANGUAGE_EXTENSIONS,
    c_companion_source_path, c_include_targets, is_c_header_path, resolve_local_c_include,
};
pub(crate) use c::{
    c_include_targets_with_offsets, c_local_include_dependency_paths, extension_case_candidates,
};
pub(crate) use csharp::{
    csharp_file_base_types, csharp_file_interface_parents, csharp_file_namespace_imports,
    csharp_file_static_type_imports, csharp_file_type_alias_imports, csharp_generic_type_arguments,
    csharp_generic_type_arguments_per_segment, csharp_generic_type_semantic_path,
    csharp_global_namespace_imports, csharp_global_static_type_imports,
    csharp_global_type_alias_imports, csharp_local_file_dependency_paths,
};
pub(crate) use go::{
    go_local_import_binding_statuses, go_local_package_dependency_paths,
    go_local_package_dependency_paths_with_deadline, go_local_package_imports_with_deadline,
    go_source_package_name,
};
pub(crate) use java::{
    JavaDirectSuperclassReference, java_direct_interface_references_for_declaration,
    java_direct_superclass_reference, java_local_explicit_static_member_imports,
    java_local_explicit_type_imports, java_local_file_dependency_paths,
};
pub(crate) use javascript::{
    JavaScriptModuleExportKind, JavaScriptModuleValuedExport, direct_require_specifier,
    javascript_cjs_object_default_member_local_name, javascript_export_local_names,
    javascript_local_module_dependency_paths, javascript_module_callable_export_local_name,
    javascript_module_constructible_export_local_name, javascript_module_default_export_local_name,
    javascript_module_reexport_module_paths_with_overrides_and_check,
    javascript_module_spread_specifiers, javascript_module_valued_export_members,
    javascript_named_export_names, javascript_named_import_module_paths_with_overrides_and_check,
    javascript_named_reexport_module_paths_with_overrides_and_check,
    javascript_star_reexport_module_paths_with_overrides_and_check,
    resolve_local_javascript_module_path_with_overrides,
};
pub(crate) use kotlin::kotlin_local_file_dependency_paths;
pub use parser::{
    ParsedDocument, detect_language, language_for_id, parse_document, parse_document_with_timeout,
    parser_for_language, supported_languages,
};
pub(crate) use parser::{validate_source_length, validate_source_size};
pub(crate) use paths::{ensure_path_inside_workspace, path_is_inside_workspace};
pub use paths::{normalize_absolute_path, normalize_path};
pub use positions::{offset_for_position, point_for_offset, position_from};
pub(crate) use python::{python_local_file_dependency_paths, resolve_local_python_module_path};
pub use registry::{
    LanguageCapabilities, LanguageDescriptor, LanguageRegistry, builtin_language_registry,
};
pub(crate) use rust::{rust_direct_module_candidate_paths, rust_local_module_dependency_paths};
pub use tree::*;

pub(crate) use io::write_source_atomic;
pub use io::{MAX_SOURCE_FILE_BYTES, read_source};

#[cfg(test)]
mod tests;
