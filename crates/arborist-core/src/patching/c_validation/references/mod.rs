mod bindings;

mod call_arities;
mod member_call_names;
mod name_collection;
mod receivers;
mod std_get;
mod type_qualifiers;
mod types;

pub(super) use bindings::collect_cpp_local_bindings;
pub(crate) use call_arities::{collect_c_call_arities, collect_cpp_call_arities};
pub(super) use name_collection::collect_c_local_definitions;
pub(crate) use name_collection::{collect_c_graph_references, collect_c_references};
use receivers::*;
pub(super) use receivers::{
    cpp_local_member_receiver_type, cpp_standard_sequence_at_receiver, cpp_subscript_receiver,
    cpp_temporary_type_from_expression, cpp_this_receiver_from_expression,
    cpp_visible_local_binding,
};

#[cfg(test)]
mod tests;
