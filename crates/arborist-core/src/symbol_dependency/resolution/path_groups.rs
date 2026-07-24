use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::super::c::{CIncludeContext, c_include_context_for_file_before_with_overrides};
use crate::symbol_index_model::IndexedSymbol;

pub(super) fn cpp_qualified_reference_path_groups(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<Vec<String>> {
    cpp_lexical_qualified_reference_paths(reference_name, source_symbol)
        .into_iter()
        .map(|reference_path| {
            cpp_qualified_reference_path_group(
                reference_path,
                raw_symbols,
                source_symbol,
                file_overrides,
            )
        })
        .collect()
}

pub(super) fn cpp_unqualified_call_candidate_groups(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<Vec<String>> {
    let include_context = c_include_context_for_file_before_with_overrides(
        &source_symbol.file_path,
        source_symbol.byte_range.0,
        file_overrides,
    )
    .ok();
    let scopes = source_symbol
        .scope_path
        .as_deref()
        .map(|scope_path| {
            let components = scope_path.split("::").collect::<Vec<_>>();
            (1..=components.len())
                .rev()
                .map(|length| components[..length].join("::"))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![String::new()]);
    scopes
        .into_iter()
        .map(|length| {
            let scope = length;
            let scoped_reference_path = if scope.is_empty() {
                reference_name.to_string()
            } else {
                format!("{scope}::{reference_name}")
            };
            let mut paths = if scope.is_empty() {
                vec![reference_name.to_string()]
            } else {
                vec![format!("{scope}::{reference_name}")]
            };
            for directive in raw_symbols.iter().filter(|symbol| {
                symbol.node_kind == "using_declaration"
                    && if scope.is_empty() {
                        symbol.scope_path.is_none()
                    } else {
                        symbol.scope_path.as_deref() == Some(scope.as_str())
                    }
                    && cpp_symbol_is_visible_before(symbol, source_symbol, include_context.as_ref())
            }) {
                let Some(target) = cpp_using_namespace_target(directive) else {
                    if directive.semantic_path != scoped_reference_path {
                        continue;
                    }
                    let Some(target) = cpp_using_declaration_target(directive) else {
                        continue;
                    };
                    paths.extend(
                        cpp_lexical_qualified_reference_paths(&target, directive)
                            .into_iter()
                            .flat_map(|path| {
                                cpp_qualified_reference_path_group(
                                    path,
                                    raw_symbols,
                                    directive,
                                    file_overrides,
                                )
                            }),
                    );
                    continue;
                };
                paths.extend(
                    cpp_lexical_qualified_reference_paths(&target, directive)
                        .into_iter()
                        .flat_map(|path| {
                            cpp_qualified_reference_path_group(
                                path,
                                raw_symbols,
                                directive,
                                file_overrides,
                            )
                        })
                        .map(|path| format!("{path}::{reference_name}")),
                );
            }
            paths
        })
        .collect()
}

pub(super) fn cpp_symbol_is_visible_before(
    symbol: &IndexedSymbol,
    source_symbol: &IndexedSymbol,
    include_context: Option<&CIncludeContext>,
) -> bool {
    (symbol.file_path == source_symbol.file_path
        && symbol.byte_range.0 < source_symbol.byte_range.0)
        || include_context.is_some_and(|context| context.include_paths.contains(&symbol.file_path))
}

pub(super) fn cpp_qualified_reference_path_group(
    reference_path: String,
    raw_symbols: &[IndexedSymbol],
    visibility_source: &IndexedSymbol,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Vec<String> {
    let include_context = c_include_context_for_file_before_with_overrides(
        &visibility_source.file_path,
        visibility_source.byte_range.0,
        file_overrides,
    )
    .ok();
    let mut pending = VecDeque::from([reference_path]);
    let mut paths = Vec::new();
    let mut visited = BTreeSet::new();

    while let Some(path) = pending.pop_front() {
        if !visited.insert(path.clone()) {
            continue;
        }
        paths.push(path.clone());
        for using_path in cpp_using_declaration_paths(
            &path,
            raw_symbols,
            visibility_source,
            include_context.as_ref(),
        )
        .into_iter()
        .rev()
        {
            pending.push_front(using_path);
        }
        for alias_path in cpp_namespace_alias_paths(
            &path,
            raw_symbols,
            visibility_source,
            include_context.as_ref(),
        )
        .into_iter()
        .rev()
        {
            pending.push_front(alias_path);
        }
    }

    paths
}

pub(super) fn cpp_lexical_qualified_reference_paths(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
) -> Vec<String> {
    let absolute = reference_name.starts_with("::");
    let reference_name = reference_name.trim_start_matches("::");
    if absolute {
        return vec![reference_name.to_string()];
    }

    let mut paths = Vec::new();
    if let Some(scope_path) = &source_symbol.scope_path {
        let scope_components = scope_path.split("::").collect::<Vec<_>>();
        for length in (1..=scope_components.len()).rev() {
            paths.push(format!(
                "{}::{reference_name}",
                scope_components[..length].join("::")
            ));
        }
    }
    paths.push(reference_name.to_string());
    paths
}

pub(super) fn cpp_namespace_alias_paths(
    reference_path: &str,
    raw_symbols: &[IndexedSymbol],
    visibility_source: &IndexedSymbol,
    include_context: Option<&CIncludeContext>,
) -> Vec<String> {
    let components = reference_path.split("::").collect::<Vec<_>>();
    for length in (1..=components.len()).rev() {
        let alias_path = components[..length].join("::");
        let suffix = components[length..].join("::");
        let Some(alias) = raw_symbols.iter().find(|symbol| {
            symbol.node_kind == "namespace_alias_definition"
                && symbol.semantic_path == alias_path
                && cpp_symbol_is_visible_before(symbol, visibility_source, include_context)
        }) else {
            continue;
        };
        let Some(target) = cpp_namespace_alias_target(alias) else {
            continue;
        };

        return cpp_lexical_qualified_reference_paths(&target, alias)
            .into_iter()
            .map(|target_path| {
                if suffix.is_empty() {
                    target_path
                } else {
                    format!("{target_path}::{suffix}")
                }
            })
            .collect();
    }
    Vec::new()
}

pub(super) fn cpp_using_declaration_paths(
    reference_path: &str,
    raw_symbols: &[IndexedSymbol],
    visibility_source: &IndexedSymbol,
    include_context: Option<&CIncludeContext>,
) -> Vec<String> {
    raw_symbols
        .iter()
        .filter(|symbol| {
            symbol.node_kind == "using_declaration"
                && symbol.semantic_path == reference_path
                && cpp_symbol_is_visible_before(symbol, visibility_source, include_context)
        })
        .filter_map(|declaration| {
            let target = cpp_using_declaration_target(declaration)?;
            Some(cpp_lexical_qualified_reference_paths(&target, declaration))
        })
        .flatten()
        .collect()
}

pub(super) fn cpp_using_declaration_target(declaration: &IndexedSymbol) -> Option<String> {
    let declaration = declaration.signature.as_deref()?.trim();
    let target = declaration.strip_prefix("using")?.trim();
    let target = target.trim_end_matches(';').trim();
    (target.contains("::") && !target.starts_with("namespace ")).then_some(target.to_string())
}

pub(super) fn cpp_using_namespace_target(declaration: &IndexedSymbol) -> Option<String> {
    let declaration = declaration.signature.as_deref()?.trim();
    let target = declaration.strip_prefix("using namespace")?.trim();
    let target = target.trim_end_matches(';').trim();
    (!target.is_empty()).then_some(target.to_string())
}

pub(super) fn cpp_namespace_alias_target(alias: &IndexedSymbol) -> Option<String> {
    let declaration = alias.signature.as_deref()?;
    let (_, target) = declaration.split_once('=')?;
    let target = target.trim().trim_end_matches(';').trim();
    (!target.is_empty()).then_some(target.to_string())
}
