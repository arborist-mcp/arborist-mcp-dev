use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

use crate::deadline::DeadlineCheck;
use crate::model::SymbolMeta;

pub(crate) fn validate_resolved_symbol_edges_with_deadline(
    symbols: &[SymbolMeta],
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    let mut symbols_by_id = BTreeMap::new();
    for symbol in symbols {
        if let Some(deadline) = deadline {
            deadline.check("indexing persisted symbol edges")?;
        }
        symbols_by_id.insert(symbol.symbol_id.as_str(), symbol);
    }

    for symbol in symbols {
        if let Some(deadline) = deadline {
            deadline.check("validating persisted symbol edges")?;
        }
        for dependency in &symbol.dependencies {
            if let Some(deadline) = deadline {
                deadline.check("validating persisted symbol edges")?;
            }
            let Some(target) = symbols_by_id.get(dependency.as_str()) else {
                return Err(anyhow!(
                    "persisted dependency `{dependency}` for symbol `{}` does not exist",
                    symbol.symbol_id
                ));
            };
            if !target.references.contains(&symbol.symbol_id) {
                return Err(anyhow!(
                    "persisted dependency `{dependency}` for symbol `{}` has no matching reference",
                    symbol.symbol_id
                ));
            }
        }
        for reference in &symbol.references {
            if let Some(deadline) = deadline {
                deadline.check("validating persisted symbol edges")?;
            }
            let Some(source) = symbols_by_id.get(reference.as_str()) else {
                return Err(anyhow!(
                    "persisted reference `{reference}` for symbol `{}` does not exist",
                    symbol.symbol_id
                ));
            };
            if !source.dependencies.contains(&symbol.symbol_id) {
                return Err(anyhow!(
                    "persisted reference `{reference}` for symbol `{}` has no matching dependency",
                    symbol.symbol_id
                ));
            }
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("validating persisted symbol edges")?;
    }
    Ok(())
}
