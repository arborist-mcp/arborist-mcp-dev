use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use crate::patching::c_validation::{
    CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_CONST_LVALUE_THIS_CALL_PREFIX,
    CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_CONST_RVALUE_THIS_CALL_PREFIX, CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_RVALUE_THIS_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_TEMPORARY_MEMBER_CALL_SEPARATOR,
};
use crate::symbol_index_model::{
    CppReferenceDetails, IndexedSymbol, ReferenceFact, ReferenceLanguageDetails,
};

/// Decodes legacy C++ reference strings stored by schema versions before v5.
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

pub(crate) fn effective_reference_facts(symbol: &IndexedSymbol) -> Cow<'_, [ReferenceFact]> {
    if symbol.reference_facts.is_empty() && !symbol.references_by_name.is_empty() {
        return Cow::Owned(reference_facts_from_legacy(
            &symbol.references_by_name,
            &symbol.call_arities_by_name,
        ));
    }

    Cow::Borrowed(&symbol.reference_facts)
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

#[cfg(test)]
mod tests {
    use super::{effective_reference_facts, reference_facts_from_legacy};
    use crate::patching::c_validation::{
        CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_TEMPORARY_MEMBER_CALL_SEPARATOR,
    };
    use crate::symbol_index_model::{
        CppReferenceDetails, IndexedSymbol, ReferenceFact, ReferenceLanguageDetails,
    };
    use std::collections::{BTreeMap, BTreeSet};

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

    #[test]
    fn explicit_reference_facts_take_precedence_over_legacy_fields() {
        let symbol = IndexedSymbol {
            symbol_id: "caller".to_string(),
            semantic_path: "caller".to_string(),
            base_name: "caller".to_string(),
            scope_path: None,
            file_path: "caller.py".to_string(),
            node_kind: "function_definition".to_string(),
            byte_range: (0, 1),
            signature: None,
            is_overload: false,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            reference_facts: vec![ReferenceFact {
                spelling: "structured_helper".to_string(),
                call_arities: None,
                language_details: ReferenceLanguageDetails::None,
            }],
            references_by_name: BTreeSet::from(["legacy_helper".to_string()]),
            call_arities_by_name: BTreeMap::new(),
        };

        let facts = effective_reference_facts(&symbol);

        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].spelling, "structured_helper");
    }
}
