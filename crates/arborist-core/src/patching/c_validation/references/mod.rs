mod bindings;

mod call_arities;
mod member_call_names;
mod name_collection;
mod receivers;
mod std_get;
mod type_qualifiers;
mod types;

pub(super) use bindings::collect_cpp_local_bindings;
pub(crate) use call_arities::{
    collect_c_call_arities, collect_c_call_arities_with_deadline, collect_cpp_call_arities,
    collect_cpp_call_arities_with_deadline,
};
pub(crate) use member_call_names::{
    CPP_CONST_LVALUE_TEMPORARY_MEMBER_CALL_PREFIX, CPP_CONST_LVALUE_THIS_CALL_PREFIX,
    CPP_CONST_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_CONST_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_CONST_RVALUE_THIS_CALL_PREFIX, CPP_CONST_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_LVALUE_VARIABLE_MEMBER_CALL_PREFIX, CPP_RVALUE_TEMPORARY_MEMBER_CALL_PREFIX,
    CPP_RVALUE_THIS_CALL_PREFIX, CPP_RVALUE_VARIABLE_MEMBER_CALL_PREFIX,
    CPP_TEMPORARY_MEMBER_CALL_SEPARATOR,
};
pub(crate) use name_collection::{
    collect_c_graph_references, collect_c_graph_references_with_deadline, collect_c_references,
    collect_c_references_with_deadline,
};
pub(super) use name_collection::{
    collect_c_local_definitions, collect_c_local_definitions_with_deadline,
    collect_c_scope_escaped_local_definition_names,
    collect_c_scope_escaped_local_definition_names_with_deadline,
};
use receivers::*;
pub(super) use receivers::{
    cpp_local_member_receiver_type, cpp_standard_sequence_at_receiver, cpp_subscript_receiver,
    cpp_temporary_type_from_expression, cpp_this_receiver_from_expression,
    cpp_visible_local_binding,
};

#[cfg(test)]
mod tests;
