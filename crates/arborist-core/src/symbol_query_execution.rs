use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{Result, anyhow};

use crate::deadline::DeadlineCheck;
use crate::language::detect_language;
use crate::model::{LanguageId, SymbolMeta, SymbolReadResult};
use crate::symbol_index_model::symbol_kind_rank;
use crate::symbol_read::read_symbol_result_from_meta;

mod list;
mod read;
mod search;
mod trace;

pub(crate) use list::{
    list_context_from_symbols, list_context_from_symbols_with_deadline,
    list_discovery_context_from_symbols, list_discovery_context_from_symbols_with_deadline,
    list_from_symbols, list_from_symbols_with_deadline, list_neighborhood_context_from_symbols,
    list_neighborhood_context_from_symbols_with_deadline,
};
pub(crate) use read::{
    read_symbol_at_position_from_symbols_with_deadline,
    read_symbol_context_at_position_from_symbols_with_deadline,
    read_symbol_context_from_symbols_with_deadline,
    read_symbol_discovery_context_at_position_from_symbols_with_deadline,
    read_symbol_discovery_context_from_symbols_with_deadline,
    read_symbol_from_symbols_with_deadline,
    read_symbol_neighborhood_context_at_position_from_symbols_with_deadline,
    read_symbol_neighborhood_context_from_symbols_with_deadline,
};
pub(crate) use search::{
    search_context_from_symbols, search_context_from_symbols_with_deadline,
    search_discovery_context_from_symbols, search_discovery_context_from_symbols_with_deadline,
    search_from_symbols, search_from_symbols_with_deadline,
    search_neighborhood_context_from_symbols,
    search_neighborhood_context_from_symbols_with_deadline,
};
pub(crate) use trace::{
    trace_from_symbols_with_deadline, trace_neighborhood_from_symbols_with_deadline,
    trace_symbol_graph_at_position_from_symbols_with_deadline,
    trace_symbol_neighborhood_at_position_from_symbols_with_deadline,
};

#[cfg(test)]
pub(crate) use trace::trace_from_symbols_with_timeout;

pub(crate) fn read_symbol_from_meta(
    symbol: &SymbolMeta,
    indexed_files: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<SymbolReadResult> {
    read_symbol_result_from_meta(symbol, indexed_files, file_overrides)
}

fn validate_trace_symbol_path(symbol_path: &str) -> Result<()> {
    if symbol_path.trim().is_empty() {
        return Err(anyhow!("invalid symbol_path: selector must not be blank"));
    }

    Ok(())
}

pub(crate) fn choose_trace_symbol_with_deadline<'a>(
    symbols: &'a [SymbolMeta],
    symbol_path: &str,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<&'a SymbolMeta>> {
    let exact_candidates =
        collect_matching_symbols(symbols, |symbol| symbol.symbol_id == symbol_path, deadline)?;
    let semantic_candidates = collect_matching_symbols(
        symbols,
        |symbol| symbol.semantic_path == symbol_path,
        deadline,
    )?;
    let mut python_candidates = Vec::new();
    for symbol in semantic_candidates.iter().copied() {
        check_trace_selection_deadline(deadline)?;
        if detect_language(Path::new(&symbol.file_path)).ok() == Some(LanguageId::Python) {
            python_candidates.push(symbol);
        }
    }

    if python_candidates.len() > 1 {
        let exact_overload_candidates = exact_candidates
            .iter()
            .copied()
            .filter(|symbol| symbol.symbol_id != symbol.semantic_path);
        if let Some(symbol) = choose_best_trace_candidate(exact_overload_candidates, deadline)? {
            return Ok(Some(symbol));
        }
        let candidate_ids = python_candidates
            .iter()
            .map(|symbol| symbol.symbol_id.as_str())
            .collect::<BTreeSet<_>>();
        return Err(anyhow!(
            "ambiguous Python semantic path `{symbol_path}`; use one of these symbol_id candidates: {}{}",
            candidate_ids.iter().copied().collect::<Vec<_>>().join(", "),
            if candidate_ids.len() < python_candidates.len() {
                "; rebuild the symbol index to materialize unique overload IDs"
            } else {
                ""
            }
        ));
    }

    if let Some(symbol) = choose_best_trace_candidate(exact_candidates, deadline)? {
        return Ok(Some(symbol));
    }
    choose_best_trace_candidate(semantic_candidates, deadline)
}

fn collect_matching_symbols<'a, F>(
    symbols: &'a [SymbolMeta],
    mut predicate: F,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<&'a SymbolMeta>>
where
    F: FnMut(&SymbolMeta) -> bool,
{
    let mut matches = Vec::new();
    for symbol in symbols {
        check_trace_selection_deadline(deadline)?;
        if predicate(symbol) {
            matches.push(symbol);
        }
    }
    Ok(matches)
}

fn choose_best_trace_candidate<'a>(
    candidates: impl IntoIterator<Item = &'a SymbolMeta>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Option<&'a SymbolMeta>> {
    let mut best: Option<&'a SymbolMeta> = None;
    for candidate in candidates {
        check_trace_selection_deadline(deadline)?;
        let replace = best.is_none_or(|current| {
            symbol_kind_rank(&candidate.node_kind)
                .cmp(&symbol_kind_rank(&current.node_kind))
                .then_with(|| current.file_path.cmp(&candidate.file_path))
                .then_with(|| current.byte_range.cmp(&candidate.byte_range))
                .then_with(|| current.symbol_id.cmp(&candidate.symbol_id))
                .is_gt()
        });
        if replace {
            best = Some(candidate);
        }
    }
    Ok(best)
}

fn check_trace_selection_deadline(deadline: Option<&dyn DeadlineCheck>) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("selecting trace symbol")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::choose_trace_symbol_with_deadline;
    use crate::model::{SymbolMeta, SymbolMetaInit};
    use crate::symbol_trace::TraceQueryDeadline;

    fn symbol(symbol_id: &str, file_path: &str, byte_range: (usize, usize)) -> SymbolMeta {
        SymbolMeta::new(SymbolMetaInit {
            symbol_id: symbol_id.to_string(),
            semantic_path: "overloaded".to_string(),
            scope_path: None,
            file_path: file_path.to_string(),
            node_kind: "function_definition".to_string(),
            origin_type: "workspace_symbol".to_string(),
            byte_range,
            signature: None,
            parameters: Vec::new(),
            return_type: None,
            docstring: None,
            dependencies: Vec::new(),
            references: Vec::new(),
        })
    }

    #[test]
    fn choose_trace_symbol_checks_deadlines_before_scanning_candidates() {
        let symbols = vec![symbol("helper", "sample.py", (0, 6))];
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = choose_trace_symbol_with_deadline(&symbols, "helper", Some(&deadline))
            .expect_err("symbol selection should honor an expired deadline");

        assert!(error.to_string().contains("selecting trace symbol"));
    }

    #[test]
    fn choose_trace_symbol_is_stable_for_equal_rank_candidates() {
        let symbols = vec![
            symbol("z", "z.cpp", (20, 21)),
            symbol("a", "a.cpp", (40, 41)),
            symbol("b", "a.cpp", (10, 11)),
        ];

        let selected = choose_trace_symbol_with_deadline(&symbols, "overloaded", None)
            .expect("selection should succeed")
            .expect("semantic path should select a candidate");
        assert_eq!(selected.symbol_id, "b");
    }

    #[test]
    fn choose_trace_symbol_rejects_ambiguous_python_overload_paths() {
        let symbols = vec![
            symbol("overloaded#overload[1]", "sample.py", (10, 20)),
            symbol("overloaded#implementation", "sample.py", (30, 40)),
        ];

        let error = choose_trace_symbol_with_deadline(&symbols, "overloaded", None)
            .expect_err("ambiguous Python overload paths should be rejected");
        assert!(error.to_string().contains("ambiguous Python semantic path"));
        assert!(error.to_string().contains("overloaded#overload[1]"));
        assert!(error.to_string().contains("overloaded#implementation"));
    }
}
