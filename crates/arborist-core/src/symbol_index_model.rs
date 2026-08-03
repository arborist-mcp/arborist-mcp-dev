use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub(crate) const CPP_RVALUE_THIS_CALL_PREFIX: &str = "\u{1f}arborist-rvalue-this:";
pub(crate) const CPP_CONST_LVALUE_THIS_CALL_PREFIX: &str = "\u{1f}arborist-const-lvalue-this:";
pub(crate) const CPP_CONST_RVALUE_THIS_CALL_PREFIX: &str = "\u{1f}arborist-const-rvalue-this:";
pub(crate) const CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX: &str =
    "\u{1f}arborist-rvalue-temporary-member:";
pub(crate) const CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX: &str =
    "\u{1f}arborist-const-lvalue-temporary-member:";
pub(crate) const CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX: &str =
    "\u{1f}arborist-const-rvalue-temporary-member:";
pub(crate) const CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX: &str =
    "\u{1f}arborist-lvalue-variable-member:";
pub(crate) const CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX: &str =
    "\u{1f}arborist-const-lvalue-variable-member:";
pub(crate) const CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX: &str =
    "\u{1f}arborist-rvalue-variable-member:";
pub(crate) const CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX: &str =
    "\u{1f}arborist-const-rvalue-variable-member:";
pub(crate) const CPP_TEMPORARY_MEMBER_CALL_SEPARATOR: &str = "\u{1e}";

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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CppReferenceDetails {
    pub(crate) rvalue_receiver: bool,
    pub(crate) const_receiver: bool,
    pub(crate) explicit_member_receiver: bool,
}

pub(crate) fn reference_facts_from_legacy(
    reference_names: &BTreeSet<String>,
    call_arities_by_name: &BTreeMap<String, BTreeSet<usize>>,
) -> Vec<ReferenceFact> {
    reference_names
        .iter()
        .map(|encoded_reference_name| {
            let (spelling, language_details) =
                decode_legacy_reference_name(encoded_reference_name.as_str());
            ReferenceFact {
                spelling: spelling.to_string(),
                call_arities: call_arities_by_name.get(encoded_reference_name).cloned(),
                language_details,
            }
        })
        .collect()
}

fn decode_legacy_reference_name(encoded_reference_name: &str) -> (&str, ReferenceLanguageDetails) {
    encoded_reference_name
        .strip_prefix(CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX)
        .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
        .map(|(_, name)| (name, cpp_member_reference_details(false, false)))
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX)
                .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                .map(|(_, name)| (name, cpp_member_reference_details(false, true)))
        })
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX)
                .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                .map(|(_, name)| (name, cpp_member_reference_details(true, false)))
        })
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX)
                .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                .map(|(_, name)| (name, cpp_member_reference_details(true, true)))
        })
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX)
                .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                .map(|(_, name)| (name, cpp_member_reference_details(true, false)))
        })
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX)
                .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                .map(|(_, name)| (name, cpp_member_reference_details(true, true)))
        })
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX)
                .and_then(|value| value.split_once(CPP_TEMPORARY_MEMBER_CALL_SEPARATOR))
                .map(|(_, name)| (name, cpp_member_reference_details(false, true)))
        })
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_CONST_RVALUE_THIS_CALL_PREFIX)
                .map(|name| (name, cpp_member_reference_details(true, true)))
        })
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_CONST_LVALUE_THIS_CALL_PREFIX)
                .map(|name| (name, cpp_member_reference_details(false, true)))
        })
        .or_else(|| {
            encoded_reference_name
                .strip_prefix(CPP_RVALUE_THIS_CALL_PREFIX)
                .map(|name| (name, cpp_member_reference_details(true, false)))
        })
        .unwrap_or((encoded_reference_name, ReferenceLanguageDetails::None))
}

fn cpp_member_reference_details(
    rvalue_receiver: bool,
    const_receiver: bool,
) -> ReferenceLanguageDetails {
    ReferenceLanguageDetails::Cpp(CppReferenceDetails {
        rvalue_receiver,
        const_receiver,
        explicit_member_receiver: true,
    })
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
        "function_definition" => 3,
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
        | "using_declaration" => 2,
        "declaration" | "field_declaration" => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::{
        CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_TEMPORARY_MEMBER_CALL_SEPARATOR,
        CppReferenceDetails, ReferenceLanguageDetails, reference_facts_from_legacy,
    };

    #[test]
    fn legacy_reference_facts_preserve_plain_references_and_missing_call_context() {
        let facts =
            reference_facts_from_legacy(&BTreeSet::from(["helper".to_string()]), &BTreeMap::new());

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].spelling, "helper");
        assert_eq!(facts[0].call_arities, None);
        assert_eq!(facts[0].language_details, ReferenceLanguageDetails::None);
    }

    #[test]
    fn legacy_reference_facts_decode_cpp_member_receiver_metadata() {
        let encoded_name = format!(
            "{CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX}Counter{CPP_TEMPORARY_MEMBER_CALL_SEPARATOR}Counter::adjust"
        );
        let reference_names = BTreeSet::from([encoded_name.clone()]);
        let call_arities_by_name =
            BTreeMap::from([(encoded_name, BTreeSet::from([1_usize, 2_usize]))]);

        let facts = reference_facts_from_legacy(&reference_names, &call_arities_by_name);

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].spelling, "Counter::adjust");
        assert_eq!(facts[0].call_arities, Some(BTreeSet::from([1, 2])));
        assert_eq!(
            facts[0].language_details,
            ReferenceLanguageDetails::Cpp(CppReferenceDetails {
                rvalue_receiver: true,
                const_receiver: true,
                explicit_member_receiver: true,
            })
        );
    }
}
