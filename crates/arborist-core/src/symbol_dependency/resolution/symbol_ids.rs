use std::collections::HashMap;
use std::path::Path;

use anyhow::Result;

use super::super::c::c_symbol_family_anchor;
use crate::language::{detect_language, is_c_header_path};
use crate::model::LanguageId;
use crate::semantic::{PythonSymbolIdentity, cpp_callable_symbol_id, python_symbol_ids};
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

pub(crate) fn assign_symbol_ids(raw_symbols: &mut [IndexedSymbol]) -> Result<()> {
    assign_symbol_ids_with_deadline(raw_symbols, None)
}

pub(crate) fn assign_symbol_ids_with_deadline(
    raw_symbols: &mut [IndexedSymbol],
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<()> {
    let mut languages_by_file: HashMap<&str, Option<LanguageId>> = HashMap::new();
    let mut languages = Vec::with_capacity(raw_symbols.len());
    for symbol in raw_symbols.iter() {
        if let Some(deadline) = deadline {
            deadline.check("assigning symbol identities")?;
        }
        let language = *languages_by_file
            .entry(symbol.file_path.as_str())
            .or_insert_with(|| detect_language(Path::new(&symbol.file_path)).ok());
        languages.push(language);
    }

    let python_indices = languages
        .iter()
        .enumerate()
        .filter_map(|(index, language)| (*language == Some(LanguageId::Python)).then_some(index))
        .collect::<Vec<_>>();
    let python_entries = python_indices
        .iter()
        .map(|index| {
            let symbol = &raw_symbols[*index];
            PythonSymbolIdentity {
                file_path: &symbol.file_path,
                semantic_path: &symbol.semantic_path,
                is_overload: symbol.is_overload,
                byte_range: symbol.byte_range,
            }
        })
        .collect::<Vec<_>>();
    let python_ids = python_symbol_ids(&python_entries);
    let mut python_ids_by_index = HashMap::new();
    for (index, symbol_id) in python_indices.into_iter().zip(python_ids) {
        python_ids_by_index.insert(index, symbol_id);
    }

    let javascript_indices = languages
        .iter()
        .enumerate()
        .filter_map(|(index, language)| {
            matches!(
                language,
                Some(LanguageId::JavaScript | LanguageId::TypeScript | LanguageId::Tsx)
            )
            .then_some(index)
        })
        .collect::<Vec<_>>();
    let javascript_ids_by_index = javascript_symbol_ids(raw_symbols, &javascript_indices);

    let mut symbol_ids = Vec::with_capacity(raw_symbols.len());
    for (index, language) in languages.into_iter().enumerate() {
        if let Some(deadline) = deadline {
            deadline.check("assigning symbol identities")?;
        }
        match python_ids_by_index.remove(&index) {
            Some(symbol_id) => symbol_ids.push(symbol_id),
            None => match javascript_ids_by_index.get(&index) {
                Some(symbol_id) => symbol_ids.push(symbol_id.clone()),
                None => symbol_ids.push(symbol_id_for_index(index, raw_symbols, language)?),
            },
        }
    }

    for (symbol, symbol_id) in raw_symbols.iter_mut().zip(symbol_ids) {
        if let Some(deadline) = deadline {
            deadline.check("assigning symbol identities")?;
        }
        symbol.symbol_id = symbol_id;
    }

    if let Some(deadline) = deadline {
        deadline.check("assigning symbol identities")?;
    }
    Ok(())
}

fn javascript_symbol_ids(
    raw_symbols: &[IndexedSymbol],
    indices: &[usize],
) -> HashMap<usize, String> {
    let mut path_counts: HashMap<&str, usize> = HashMap::new();
    let mut groups: std::collections::BTreeMap<(&str, &str), Vec<usize>> =
        std::collections::BTreeMap::new();
    for index in indices {
        let symbol = &raw_symbols[*index];
        *path_counts.entry(&symbol.semantic_path).or_default() += 1;
        groups
            .entry((&symbol.file_path, &symbol.semantic_path))
            .or_default()
            .push(*index);
    }

    let mut ids = HashMap::new();
    for ((file_path, semantic_path), indexes) in &mut groups {
        indexes.sort_by_key(|index| raw_symbols[*index].byte_range);
        let identity_path = format!("{file_path}::{semantic_path}");
        if indexes.len() == 1 {
            let index = indexes[0];
            let symbol_id = if path_counts[semantic_path] > 1 {
                identity_path
            } else {
                (*semantic_path).to_string()
            };
            ids.insert(index, symbol_id);
            continue;
        }

        for (ordinal, index) in indexes.iter().enumerate() {
            ids.insert(
                *index,
                format!("{identity_path}#definition[{}]", ordinal + 1),
            );
        }
    }
    ids
}

fn symbol_id_for_index(
    index: usize,
    raw_symbols: &[IndexedSymbol],
    language: Option<LanguageId>,
) -> Result<String> {
    let symbol = &raw_symbols[index];
    let path = Path::new(&symbol.file_path);
    if language == Some(LanguageId::Cpp)
        && matches!(
            symbol.node_kind.as_str(),
            "function_definition" | "declaration" | "field_declaration"
        )
    {
        return Ok(cpp_callable_symbol_id(
            &symbol.semantic_path,
            &symbol.parameters,
            symbol.signature.as_deref(),
        ));
    }
    if !matches!(language, Some(LanguageId::C | LanguageId::Cpp))
        || symbol.semantic_path.contains("::")
    {
        return Ok(symbol.semantic_path.clone());
    }

    let anchor = if is_c_header_path(path) {
        symbol.file_path.clone()
    } else {
        c_symbol_family_anchor(symbol, raw_symbols)?
    };

    Ok(format!("{anchor}::{}", symbol.base_name))
}
