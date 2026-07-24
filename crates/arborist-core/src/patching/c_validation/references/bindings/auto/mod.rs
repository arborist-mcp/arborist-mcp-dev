mod alias;
mod constructor;
mod copy;

// Re-export at references visibility so bindings can pub(super) use auto::*.
pub(in super::super) use alias::*;
pub(in super::super) use constructor::*;

// These two are re-exported from bindings into c_validation; keep that path.
pub(in super::super::super) use alias::{cpp_address_binding, cpp_named_reference_alias_receiver};
