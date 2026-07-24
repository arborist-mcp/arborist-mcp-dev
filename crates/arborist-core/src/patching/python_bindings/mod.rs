mod imports;
mod local;
mod path;
mod scope;
mod summary;
mod targets;
mod types;

// Re-export only the patching-facing surface (was pub(super) on the monolith).
pub(super) use local::collect_python_local_bindings;
pub(super) use path::python_scope_declares_external_name;
pub(super) use scope::collect_python_scope_symbols;
pub(super) use summary::python_symbol_summary;
pub(super) use types::{PythonAccessibleSymbol, PythonSymbolVisibility};
