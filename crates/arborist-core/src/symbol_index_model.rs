use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReferenceFact {
    pub(crate) spelling: String,
    pub(crate) call_arities: Option<BTreeSet<usize>>,
    pub(crate) language_details: ReferenceLanguageDetails,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReferenceLanguageDetails {
    None,
    Cpp(CppReferenceDetails),
    Go(GoReferenceDetails),
    Rust(RustReferenceDetails),
    JavaScript(JavaScriptReferenceDetails),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum RustImportRoot {
    Crate,
    SelfModule,
    Super { levels: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GoReferenceDetails {
    #[serde(default)]
    pub(crate) type_conversion: bool,
    #[serde(default)]
    pub(crate) type_assertion: bool,
    #[serde(default)]
    pub(crate) factory_return: bool,
    #[serde(default)]
    pub(crate) factory_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RustReferenceDetails {
    pub(crate) import_root: Option<RustImportRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct JavaScriptReferenceDetails {
    /// Module namespace receiver for member calls such as `ns.helper(...)`,
    /// where `ns` is a local `import * as ns from "..."` binding.
    #[serde(default)]
    pub(crate) namespace_receiver: Option<String>,
    /// Inline `require("./module").member(...)` member call: the static module
    /// specifier and the accessed member name, resolved against the
    /// referencing file at resolution time so overlay/override paths apply.
    #[serde(default)]
    pub(crate) require_member_call: Option<(String, String)>,
    /// Inline bare `require("./module")(...)` namespace-object call: the
    /// static module specifier, resolved against the referencing file at
    /// resolution time. Only CommonJS callable exports resolve.
    #[serde(default)]
    pub(crate) require_object_call: Option<String>,
    /// True when the reference is a `new` constructor expression rather than a
    /// plain call, so namespace-object constructors may resolve class exports
    /// while plain calls stay limited to callable exports.
    #[serde(default)]
    pub(crate) constructor_call: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CppReferenceDetails {
    pub(crate) rvalue_receiver: bool,
    pub(crate) const_receiver: bool,
    pub(crate) explicit_member_receiver: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct IndexedSymbol {
    pub(crate) symbol_id: String,
    pub(crate) semantic_path: String,
    pub(crate) base_name: String,
    pub(crate) scope_path: Option<String>,
    pub(crate) file_path: String,
    pub(crate) node_kind: String,
    pub(crate) byte_range: (usize, usize),
    pub(crate) signature: Option<String>,
    pub(crate) is_overload: bool,
    pub(crate) parameters: Vec<String>,
    pub(crate) return_type: Option<String>,
    pub(crate) docstring: Option<String>,
    pub(crate) extension_receiver: Option<String>,
    pub(crate) reference_facts: Vec<ReferenceFact>,
    // Retained until persisted indexes have fully transitioned to reference_facts_json.
    pub(crate) references_by_name: BTreeSet<String>,
    pub(crate) call_arities_by_name: BTreeMap<String, BTreeSet<usize>>,
}

#[derive(Debug, Clone)]
pub(crate) struct PersistedFileState {
    pub(crate) file_path: String,
    pub(crate) fingerprint: u64,
}

pub(crate) fn symbol_base_name_ref(semantic_path: &str) -> &str {
    semantic_path
        .rsplit("::")
        .next()
        .unwrap_or(semantic_path)
        .rsplit('.')
        .next()
        .unwrap_or(semantic_path)
}

pub(crate) fn symbol_base_name(semantic_path: &str) -> String {
    symbol_base_name_ref(semantic_path).to_string()
}

pub(crate) fn symbol_kind_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" | "function_item" | "function_signature_item" => 3,
        "class_definition" => 3,
        "alias_declaration"
        | "class_specifier"
        | "concept_definition"
        | "enum_specifier"
        | "enumerator"
        | "namespace_alias_definition"
        | "struct_specifier"
        | "template_instantiation"
        | "type_definition"
        | "union_specifier"
        | "using_declaration"
        | "const_item"
        | "enum_item"
        | "mod_item"
        | "static_item"
        | "struct_item"
        | "trait_item"
        | "type_item" => 2,
        "declaration" | "field_declaration" => 1,
        _ => 0,
    }
}
