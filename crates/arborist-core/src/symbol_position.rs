use std::collections::BTreeMap;
use std::path::Path;

use crate::deadline::DeadlineCheck;
use crate::language::{
    builtin_language_registry, normalize_path, offset_for_position, parse_document, read_source,
};
use crate::model::{Position, SymbolMeta};
use crate::semantic::ascend_to_symbol;
use anyhow::{Result, anyhow};

mod selection;

pub(crate) fn resolve_symbol_at_position_with_deadline<'a>(
    resolved_symbols: &'a [SymbolMeta],
    file_path: &Path,
    position: &Position,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<&'a SymbolMeta> {
    check_position_deadline(deadline, "resolving symbol at position")?;
    let normalized_file_path = normalize_path(file_path);
    let source = match file_overrides.and_then(|overrides| overrides.get(&normalized_file_path)) {
        Some(source) => source.clone(),
        None => read_source(file_path)?,
    };
    check_position_deadline(deadline, "parsing symbol position")?;
    let document = parse_document(file_path, &source)?;
    check_position_deadline(deadline, "resolving symbol position")?;
    let byte_offset = offset_for_position(&source, position)?;
    let node = selection::node_at_byte_offset(document.tree.root_node(), &source, byte_offset)
        .ok_or_else(|| {
            anyhow!(
                "position {}:{} does not resolve to a syntax node in {}",
                position.row,
                position.column,
                file_path.display()
            )
        })?;
    check_position_deadline(deadline, "resolving symbol position")?;
    let symbol_node = ascend_to_symbol(document.language_id, node).ok_or_else(|| {
        anyhow!(
            "position {}:{} does not resolve to a semantic symbol in {}",
            position.row,
            position.column,
            file_path.display()
        )
    })?;
    check_position_deadline(deadline, "resolving symbol position")?;

    let identity = builtin_language_registry()
        .adapter(document.language_id)
        .expect("every LanguageId must have a builtin language adapter")
        .position_symbol_identity(file_path, symbol_node, &source)?;
    let (symbol_id, semantic_path, byte_range) = (
        identity.symbol_id,
        identity.semantic_path,
        identity.byte_range,
    );
    check_position_deadline(deadline, "resolving symbol position")?;

    selection::choose_symbol_at_location_with_deadline(
        resolved_symbols,
        &normalized_file_path,
        &symbol_id,
        &semantic_path,
        byte_range,
        deadline,
    )?
    .ok_or_else(|| {
        anyhow!(
            "symbol at {}:{} not found in workspace index: {}",
            position.row,
            position.column,
            normalized_file_path
        )
    })
}

fn check_position_deadline(deadline: Option<&dyn DeadlineCheck>, phase: &str) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check(phase)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::resolve_symbol_at_position_with_deadline;
    use crate::model::Position;
    use crate::symbol_trace::TraceQueryDeadline;

    #[test]
    fn resolve_symbol_at_position_checks_deadline_before_source_io() {
        let deadline = TraceQueryDeadline::expired_for_tests(1);

        let error = resolve_symbol_at_position_with_deadline(
            &[],
            Path::new("missing.py"),
            &Position { row: 0, column: 0 },
            None,
            Some(&deadline),
        )
        .expect_err("position resolution should honor an expired deadline");

        assert!(error.to_string().contains("resolving symbol at position"));
    }
}
