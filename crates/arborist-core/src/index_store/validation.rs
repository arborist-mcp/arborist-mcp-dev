use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

use crate::model::SymbolMeta;

pub(crate) fn validate_resolved_symbol_edges(symbols: &[SymbolMeta]) -> Result<()> {
    let symbols_by_id = symbols
        .iter()
        .map(|symbol| (symbol.symbol_id.as_str(), symbol))
        .collect::<BTreeMap<_, _>>();

    for symbol in symbols {
        for dependency in &symbol.dependencies {
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

    Ok(())
}
