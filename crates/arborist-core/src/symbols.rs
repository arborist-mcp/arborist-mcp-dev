use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rusqlite::{Connection, params};
use tree_sitter::Node;

use crate::language::{
    c_include_targets, c_local_include_targets, contains_kind, detect_language, node_text,
    normalize_absolute_path, normalize_path, parse_document, read_source, resolve_local_c_include,
    visit_tree,
};
use crate::model::LanguageId;
use crate::model::{
    SymbolIndexStats, SymbolMeta, SymbolSummary, TraceDirection, TraceEvidenceKeys,
    TraceSymbolGraphResult,
};
use crate::patching::{
    collect_c_references, collect_python_references, resolve_local_python_imported_symbol,
    resolve_local_python_module_path,
};
use crate::semantic::{
    c_function_header, c_parameters, c_return_type, c_semantic_path, python_display_byte_range,
    python_display_header, python_docstring, python_parameters, python_return_type,
    semantic_parent_path, semantic_path,
};

#[derive(Debug, Clone)]
struct IndexedSymbol {
    symbol_id: String,
    semantic_path: String,
    base_name: String,
    scope_path: Option<String>,
    file_path: String,
    node_kind: String,
    byte_range: (usize, usize),
    signature: Option<String>,
    parameters: Vec<String>,
    return_type: Option<String>,
    docstring: Option<String>,
    references_by_name: BTreeSet<String>,
}

#[derive(Debug, Clone)]
struct PersistedFileState {
    file_path: String,
    fingerprint: u64,
}

#[derive(Debug, Default)]
struct CIncludeContext {
    include_paths: BTreeSet<String>,
    companion_source_paths: BTreeSet<String>,
}

pub fn trace_symbol_graph(
    workspace_root: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let (resolved_symbols, indexed_files) = resolve_workspace_symbols(&workspace_root)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn trace_symbol_graph_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let (resolved_symbols, indexed_files) =
        resolve_workspace_symbols_with_overrides(workspace_root, file_overrides)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn rebuild_symbol_index(workspace_root: &Path, db_path: &Path) -> Result<SymbolIndexStats> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    let (raw_symbols, resolved_symbols, file_states, indexed_files, rebuilt_files, reused_files) =
        resolve_workspace_symbols_incremental(&workspace_root, &db_path)?;
    persist_symbol_index(
        &db_path,
        &workspace_root,
        &raw_symbols,
        &resolved_symbols,
        &file_states,
        indexed_files,
    )?;

    Ok(SymbolIndexStats {
        db_path: normalize_path(&db_path),
        indexed_files,
        indexed_symbols: resolved_symbols.len(),
        rebuilt_files,
        reused_files,
    })
}

pub fn trace_symbol_graph_from_index(
    db_path: &Path,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let db_path = normalize_absolute_path(db_path)?;
    let (resolved_symbols, indexed_files) = load_symbol_index(&db_path)?;
    trace_from_symbols(&resolved_symbols, indexed_files, symbol_path, direction)
}

pub fn refresh_symbol_index_for_file(
    workspace_root: &Path,
    db_path: &Path,
    file_path: &Path,
) -> Result<SymbolIndexStats> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let db_path = normalize_absolute_path(db_path)?;
    let file_path = normalize_absolute_path(file_path)?;

    if !file_path.starts_with(&workspace_root) {
        return Err(anyhow!(
            "file {} is outside workspace {}",
            file_path.display(),
            workspace_root.display()
        ));
    }

    if !db_path.exists() {
        return rebuild_symbol_index(&workspace_root, &db_path);
    }

    let connection = Connection::open(&db_path)?;
    ensure_symbol_tables(&connection)?;

    let old_resolved_symbols = load_symbols_from_connection(&connection)?.0;
    let old_resolved_map = resolved_symbol_map(&old_resolved_symbols);
    let mut grouped_symbols = load_indexed_symbols_grouped_by_file(&connection)?;
    let refresh_paths = expanded_refresh_file_paths(&workspace_root, &file_path)?;

    let mut file_states = load_file_states(&connection)?;
    let mut old_changed_symbols = Vec::new();
    let mut changed_file_paths = BTreeSet::new();

    for refresh_path in &refresh_paths {
        let normalized_refresh_path = normalize_path(refresh_path);
        old_changed_symbols.extend(
            grouped_symbols
                .get(&normalized_refresh_path)
                .cloned()
                .unwrap_or_default(),
        );

        if refresh_path.exists() {
            let source = read_source(refresh_path)?;
            let document = parse_document(refresh_path, &source)?;
            let fresh_symbols = index_symbols_from_document(refresh_path, &source, &document)?;

            file_states.insert(normalized_refresh_path.clone(), source_fingerprint(&source));
            grouped_symbols.insert(normalized_refresh_path.clone(), fresh_symbols);
        } else {
            file_states.remove(&normalized_refresh_path);
            grouped_symbols.remove(&normalized_refresh_path);
        }
        changed_file_paths.insert(normalized_refresh_path);
    }

    let rebuilt_files = refresh_paths.len();

    let mut raw_symbols = grouped_symbols
        .into_values()
        .flat_map(|symbols| symbols.into_iter())
        .collect::<Vec<_>>();
    assign_symbol_ids(&mut raw_symbols)?;
    let new_changed_symbols = raw_symbols
        .iter()
        .filter(|symbol| changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect::<Vec<_>>();
    let (resolved_map, impacted_paths) = refresh_resolved_symbol_subgraph(
        &raw_symbols,
        &old_resolved_map,
        &old_changed_symbols,
        &new_changed_symbols,
        &changed_file_paths,
    );
    let resolved_symbols = materialize_resolved_symbol_rows(&raw_symbols, &resolved_map);
    let indexed_files = file_states.len();
    let reused_files = indexed_files.saturating_sub(rebuilt_files);

    persist_symbol_refresh(
        &db_path,
        &workspace_root,
        &raw_symbols,
        &resolved_symbols,
        &file_states,
        &changed_file_paths,
        &impacted_paths,
        indexed_files,
    )?;

    Ok(SymbolIndexStats {
        db_path: normalize_path(&db_path),
        indexed_files,
        indexed_symbols: resolved_symbols.len(),
        rebuilt_files,
        reused_files,
    })
}

fn expanded_refresh_file_paths(workspace_root: &Path, file_path: &Path) -> Result<Vec<PathBuf>> {
    let mut refresh_paths = BTreeSet::new();
    refresh_paths.insert(file_path.to_path_buf());

    if matches!(detect_language(file_path)?, LanguageId::C) {
        refresh_paths.extend(transitive_c_include_dependents(workspace_root, file_path)?);
    }

    Ok(refresh_paths.into_iter().collect())
}

fn transitive_c_include_dependents(
    workspace_root: &Path,
    target_path: &Path,
) -> Result<BTreeSet<PathBuf>> {
    let reverse_index = reverse_local_c_include_index(workspace_root)?;
    let normalized_target = normalize_path(target_path);
    let mut queue = vec![normalized_target.clone()];
    let mut visited = BTreeSet::from([normalized_target]);
    let mut dependents = BTreeSet::new();

    while let Some(current_path) = queue.pop() {
        let Some(children) = reverse_index.get(&current_path) else {
            continue;
        };

        for dependent_path in children {
            let normalized_dependent = normalize_path(dependent_path);
            if visited.insert(normalized_dependent.clone()) {
                dependents.insert(dependent_path.clone());
                queue.push(normalized_dependent);
            }
        }
    }

    Ok(dependents)
}

fn reverse_local_c_include_index(
    workspace_root: &Path,
) -> Result<BTreeMap<String, BTreeSet<PathBuf>>> {
    let mut reverse_index = BTreeMap::new();

    for path in collect_source_files(workspace_root)? {
        if !matches!(detect_language(&path), Ok(LanguageId::C)) {
            continue;
        }

        let source = read_source(&path)?;
        let document = parse_document(&path, &source)?;
        let local_include_targets = c_local_include_targets(document.tree.root_node(), &source)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        for include_target in c_include_targets(document.tree.root_node(), &source)? {
            let Some(include_path) =
                resolve_local_c_include(&path, &include_target).or_else(|| {
                    local_include_targets
                        .contains(&include_target)
                        .then(|| unresolved_local_c_include_path(&path, &include_target))
                        .flatten()
                })
            else {
                continue;
            };
            if !include_path.starts_with(workspace_root) {
                continue;
            }

            reverse_index
                .entry(normalize_path(&include_path))
                .or_insert_with(BTreeSet::new)
                .insert(path.clone());
        }
    }

    Ok(reverse_index)
}

fn unresolved_local_c_include_path(current_path: &Path, include_target: &str) -> Option<PathBuf> {
    let parent = current_path.parent()?;
    normalize_absolute_path(&parent.join(include_target)).ok()
}

fn collect_source_files(workspace_root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    walk_workspace(workspace_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk_workspace(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if path.is_dir() {
        if should_skip_dir(path) {
            return Ok(());
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            walk_workspace(&entry.path(), files)?;
        }
        return Ok(());
    }

    if detect_language(path).is_ok() {
        files.push(path.to_path_buf());
    }

    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | ".venv" | "target" | "node_modules" | "dist" | "build"
            )
        })
}

fn build_workspace_index(
    paths: &[PathBuf],
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();

    for path in paths {
        let normalized_path = normalize_path(path);
        let source = match file_overrides.and_then(|overrides| overrides.get(&normalized_path)) {
            Some(source) => source.clone(),
            None => read_source(path)?,
        };
        let document = parse_document(path, &source)?;
        symbols.extend(index_symbols_from_document(path, &source, &document)?);
    }

    assign_symbol_ids(&mut symbols)?;
    Ok(symbols)
}

fn index_symbols_from_document(
    path: &Path,
    source: &str,
    document: &crate::language::ParsedDocument,
) -> Result<Vec<IndexedSymbol>> {
    match document.language_id {
        LanguageId::Python => index_python_symbols(path, source, document.tree.root_node()),
        LanguageId::C => index_c_symbols(path, source, document.tree.root_node()),
    }
}

fn index_python_symbols(path: &Path, source: &str, root: Node<'_>) -> Result<Vec<IndexedSymbol>> {
    let mut symbols = Vec::new();
    let normalized_path = normalize_path(path);

    let mut callback = |node: Node<'_>| {
        if !matches!(node.kind(), "class_definition" | "function_definition") {
            return;
        }

        let mut references = BTreeSet::new();
        let reference_node = python_reference_node(node);
        let _ = collect_python_references(path, reference_node, source, &mut references);
        let signature = python_display_header(node, source).ok();
        let path = match semantic_path(node, source) {
            Ok(path) => path,
            Err(_) => return,
        };
        let scope_path = semantic_parent_path(&path);
        let parameters = python_parameters(node, source).unwrap_or_default();
        let return_type = python_return_type(node, source).ok().flatten();
        let docstring = python_docstring(node, source).ok().flatten();

        symbols.push(IndexedSymbol {
            symbol_id: String::new(),
            base_name: path.rsplit('.').next().unwrap_or(&path).to_string(),
            semantic_path: path,
            scope_path,
            file_path: normalized_path.clone(),
            node_kind: node.kind().to_string(),
            byte_range: python_display_byte_range(node),
            signature,
            parameters,
            return_type,
            docstring,
            references_by_name: references,
        });
    };

    visit_tree(root, &mut callback);
    Ok(symbols)
}

fn python_reference_node(node: Node<'_>) -> Node<'_> {
    node.parent()
        .filter(|parent| parent.kind() == "decorated_definition")
        .unwrap_or(node)
}

fn index_c_symbols(path: &Path, source: &str, root: Node<'_>) -> Result<Vec<IndexedSymbol>> {
    let normalized_path = normalize_path(path);
    let mut symbols = Vec::new();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        match child.kind() {
            "type_definition" => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: name.rsplit("::").next().unwrap_or(&name).to_string(),
                        semantic_path: name,
                        scope_path: None,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(node_text(child, source)?.trim().to_string()),
                        parameters: Vec::new(),
                        return_type: None,
                        docstring: None,
                        references_by_name: BTreeSet::new(),
                    });
                }
            }
            "declaration" if contains_kind(child, "function_declarator") => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    let scope_path = semantic_parent_path(&name);
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: name.rsplit("::").next().unwrap_or(&name).to_string(),
                        semantic_path: name,
                        scope_path,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(node_text(child, source)?.trim().to_string()),
                        parameters: c_parameters(child, source)?,
                        return_type: c_return_type(child, source)?,
                        docstring: None,
                        references_by_name: BTreeSet::new(),
                    });
                }
            }
            "function_definition" => {
                if let Some(name) = c_semantic_path(path, child, source)? {
                    let mut references = BTreeSet::new();
                    collect_c_references(child, source, &mut references)?;
                    let scope_path = semantic_parent_path(&name);
                    symbols.push(IndexedSymbol {
                        symbol_id: String::new(),
                        base_name: name.rsplit("::").next().unwrap_or(&name).to_string(),
                        semantic_path: name,
                        scope_path,
                        file_path: normalized_path.clone(),
                        node_kind: child.kind().to_string(),
                        byte_range: (child.start_byte(), child.end_byte()),
                        signature: Some(c_function_header(child, source)?),
                        parameters: c_parameters(child, source)?,
                        return_type: c_return_type(child, source)?,
                        docstring: None,
                        references_by_name: references,
                    });
                }
            }
            _ => {}
        }
    }

    Ok(symbols)
}

fn resolve_reference_path(
    reference_name: &str,
    source_symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
) -> Option<String> {
    let language_id = detect_language(Path::new(&source_symbol.file_path)).ok();
    let (lookup_name, module_hint) = if language_id == Some(LanguageId::Python) {
        python_reference_lookup(reference_name)
    } else {
        (reference_name, None)
    };
    let candidates = name_index.get(lookup_name)?;
    let visible_candidates: Vec<usize> = candidates
        .iter()
        .copied()
        .filter(|index| {
            let candidate = &raw_symbols[*index];
            candidate.file_path == source_symbol.file_path
                || !candidate.semantic_path.contains("::")
        })
        .collect();
    let candidate_slice = if visible_candidates.is_empty() {
        candidates.as_slice()
    } else {
        visible_candidates.as_slice()
    };
    let hinted_candidates = if let Some(module_hint) = module_hint {
        let imported_summary = resolve_local_python_imported_symbol(
            Path::new(&source_symbol.file_path),
            module_hint,
            lookup_name,
        )
        .ok()
        .flatten();
        let filtered = candidate_slice
            .iter()
            .copied()
            .filter(|index| {
                python_symbol_matches_module_hint(
                    source_symbol,
                    &raw_symbols[*index],
                    module_hint,
                    imported_summary.as_ref(),
                )
            })
            .collect::<Vec<_>>();
        if filtered.is_empty() {
            candidate_slice.to_vec()
        } else {
            filtered
        }
    } else {
        candidate_slice.to_vec()
    };
    let include_context = c_include_context_for_file(&source_symbol.file_path).ok();

    hinted_candidates
        .iter()
        .copied()
        .max_by_key(|index| {
            indexed_symbol_candidate_rank(
                &raw_symbols[*index],
                Some(&source_symbol.file_path),
                include_context.as_ref(),
            )
        })
        .map(|index| raw_symbols[index].symbol_id.clone())
}

fn python_reference_lookup(reference_name: &str) -> (&str, Option<&str>) {
    reference_name
        .rsplit_once('.')
        .map(|(module_hint, symbol_name)| (symbol_name, Some(module_hint)))
        .unwrap_or((reference_name, None))
}

fn python_symbol_matches_module_hint(
    source_symbol: &IndexedSymbol,
    symbol: &IndexedSymbol,
    module_hint: &str,
    imported_summary: Option<&SymbolSummary>,
) -> bool {
    if let Some(imported_summary) = imported_summary {
        return imported_summary.file_path == symbol.file_path
            && imported_summary.semantic_path == symbol.semantic_path;
    }

    let Some(resolved_module_path) =
        resolve_local_python_module_path(Path::new(&source_symbol.file_path), module_hint)
    else {
        return false;
    };

    normalize_path(&resolved_module_path) == symbol.file_path
}

fn summarize_symbols(
    symbols: &[SymbolMeta],
    semantic_paths: &[String],
    context_file: Option<&str>,
) -> Vec<SymbolSummary> {
    let include_context = context_file.and_then(|file| c_include_context_for_file(file).ok());
    semantic_paths
        .iter()
        .filter_map(|semantic_path| {
            choose_symbol_summary(
                symbols,
                semantic_path,
                context_file,
                include_context.as_ref(),
            )
        })
        .collect()
}

fn choose_symbol_summary(
    symbols: &[SymbolMeta],
    symbol_id: &str,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> Option<SymbolSummary> {
    symbols
        .iter()
        .filter(|symbol| symbol.symbol_id == symbol_id)
        .max_by_key(|symbol| symbol_candidate_rank(symbol, context_file, include_context))
        .map(|symbol| {
            SymbolSummary::new(
                symbol.symbol_id.clone(),
                symbol.semantic_path.clone(),
                symbol.scope_path.clone(),
                symbol.file_path.clone(),
                symbol.node_kind.clone(),
                symbol_origin_type(symbol, context_file, include_context).to_string(),
                symbol.byte_range,
                symbol.signature.clone(),
                symbol.parameters.clone(),
                symbol.return_type.clone(),
                symbol.docstring.clone(),
            )
        })
}

fn symbol_origin_type(
    symbol: &SymbolMeta,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> &'static str {
    if context_file.is_some_and(|context_file| symbol.file_path == context_file) {
        return "local_file";
    }

    if include_context.is_some_and(|include_context| {
        include_context
            .companion_source_paths
            .contains(&symbol.file_path)
    }) {
        return "companion_source";
    }

    if include_context
        .is_some_and(|include_context| include_context.include_paths.contains(&symbol.file_path))
    {
        return "include_header";
    }

    "workspace_symbol"
}

fn symbol_candidate_rank(
    symbol: &SymbolMeta,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> usize {
    let mut rank = resolved_symbol_rank(symbol);

    if let Some(context_file) = context_file {
        if symbol.file_path == context_file {
            rank += 1000;
        } else if symbol.semantic_path.contains("::") {
            rank = rank.saturating_sub(100);
        }
    }

    if let Some(include_context) = include_context {
        if include_context.include_paths.contains(&symbol.file_path) {
            rank += 200;
        }
        if include_context
            .companion_source_paths
            .contains(&symbol.file_path)
        {
            rank += 300;
        }
    }

    rank
}

fn indexed_symbol_candidate_rank(
    symbol: &IndexedSymbol,
    context_file: Option<&str>,
    include_context: Option<&CIncludeContext>,
) -> usize {
    let mut rank = indexed_symbol_rank(symbol);

    if let Some(context_file) = context_file {
        if symbol.file_path == context_file {
            rank += 1000;
        } else if symbol.semantic_path.contains("::") {
            rank = rank.saturating_sub(100);
        }
    }

    if let Some(include_context) = include_context {
        if include_context.include_paths.contains(&symbol.file_path) {
            rank += 200;
        }
        if include_context
            .companion_source_paths
            .contains(&symbol.file_path)
        {
            rank += 300;
        }
    }

    rank
}

fn resolve_workspace_symbols(workspace_root: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    let indexed_paths = collect_source_files(workspace_root)?;
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, None)?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols);
    Ok((resolved_symbols, indexed_files))
}

fn resolve_workspace_symbols_with_overrides(
    workspace_root: &Path,
    file_overrides: &BTreeMap<String, String>,
) -> Result<(Vec<SymbolMeta>, usize)> {
    let workspace_root = normalize_absolute_path(workspace_root)?;
    let mut indexed_paths = collect_source_files(&workspace_root)?;
    let mut known_paths: BTreeSet<String> = indexed_paths
        .iter()
        .map(|path| normalize_path(path))
        .collect();

    for override_path in file_overrides.keys() {
        let override_path = normalize_absolute_path(Path::new(override_path))?;
        if !override_path.starts_with(&workspace_root) || detect_language(&override_path).is_err() {
            continue;
        }

        let normalized_path = normalize_path(&override_path);
        if known_paths.insert(normalized_path) {
            indexed_paths.push(override_path);
        }
    }

    indexed_paths.sort();
    let indexed_files = indexed_paths.len();
    let raw_symbols = build_workspace_index(&indexed_paths, Some(file_overrides))?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols);
    Ok((resolved_symbols, indexed_files))
}

fn resolve_workspace_symbols_incremental(
    workspace_root: &Path,
    db_path: &Path,
) -> Result<(
    Vec<IndexedSymbol>,
    Vec<SymbolMeta>,
    Vec<PersistedFileState>,
    usize,
    usize,
    usize,
)> {
    let indexed_paths = collect_source_files(workspace_root)?;
    let indexed_files = indexed_paths.len();
    let connection = Connection::open(db_path)?;
    ensure_symbol_tables(&connection)?;

    let persisted_states = load_file_states(&connection)?;
    let persisted_symbols = load_indexed_symbols_grouped_by_file(&connection)?;

    let mut raw_symbols = Vec::new();
    let mut file_states = Vec::new();
    let mut rebuilt_files = 0;
    let mut reused_files = 0;

    for path in indexed_paths {
        let source = read_source(&path)?;
        let normalized_path = normalize_path(&path);
        let fingerprint = source_fingerprint(&source);

        file_states.push(PersistedFileState {
            file_path: normalized_path.clone(),
            fingerprint,
        });

        if persisted_states
            .get(&normalized_path)
            .is_some_and(|stored| *stored == fingerprint)
        {
            if let Some(stored_symbols) = persisted_symbols.get(&normalized_path) {
                raw_symbols.extend(stored_symbols.iter().cloned());
                reused_files += 1;
                continue;
            }
        }

        let document = parse_document(&path, &source)?;
        raw_symbols.extend(index_symbols_from_document(&path, &source, &document)?);
        rebuilt_files += 1;
    }

    assign_symbol_ids(&mut raw_symbols)?;
    let resolved_symbols = resolve_symbol_dependencies(&raw_symbols);
    Ok((
        raw_symbols,
        resolved_symbols,
        file_states,
        indexed_files,
        rebuilt_files,
        reused_files,
    ))
}

fn build_name_index(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut name_index = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        name_index
            .entry(symbol.base_name.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    name_index
}

fn assign_symbol_ids(raw_symbols: &mut [IndexedSymbol]) -> Result<()> {
    let symbol_ids = (0..raw_symbols.len())
        .map(|index| symbol_id_for_index(index, raw_symbols))
        .collect::<Result<Vec<_>>>()?;

    for (symbol, symbol_id) in raw_symbols.iter_mut().zip(symbol_ids) {
        symbol.symbol_id = symbol_id;
    }

    Ok(())
}

fn symbol_id_for_index(index: usize, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let symbol = &raw_symbols[index];
    let path = Path::new(&symbol.file_path);
    if detect_language(path).ok() != Some(LanguageId::C) || symbol.semantic_path.contains("::") {
        return Ok(symbol.semantic_path.clone());
    }

    let anchor = if is_c_header_path(path) {
        symbol.file_path.clone()
    } else {
        c_symbol_family_anchor(symbol, raw_symbols)?
    };

    Ok(format!("{anchor}::{}", symbol.base_name))
}

fn c_symbol_family_anchor(symbol: &IndexedSymbol, raw_symbols: &[IndexedSymbol]) -> Result<String> {
    let include_context = c_include_context_for_file(&symbol.file_path)?;
    let source_path = Path::new(&symbol.file_path);

    let best_header = raw_symbols
        .iter()
        .filter_map(|candidate| {
            (candidate.semantic_path == symbol.semantic_path
                && !candidate.semantic_path.contains("::")
                && is_c_header_path(Path::new(&candidate.file_path)))
            .then(|| {
                let rank =
                    c_family_header_rank(source_path, &candidate.file_path, &include_context);
                (candidate, rank)
            })
        })
        .filter(|(_, rank)| *rank > 0)
        .max_by_key(|(_, rank)| *rank)
        .map(|(candidate, _)| candidate);

    Ok(best_header
        .map(|candidate| candidate.file_path.clone())
        .unwrap_or_else(|| symbol.file_path.clone()))
}

fn c_family_header_rank(
    source_path: &Path,
    header_file_path: &str,
    include_context: &CIncludeContext,
) -> usize {
    let mut rank = 0;
    let header_path = Path::new(header_file_path);
    if same_stem(source_path, header_path) {
        rank += 1000;
    }
    if include_context.include_paths.contains(header_file_path) {
        rank += 500;
    }
    rank
}

fn same_stem(left: &Path, right: &Path) -> bool {
    left.file_stem()
        .and_then(|stem| stem.to_str())
        .zip(right.file_stem().and_then(|stem| stem.to_str()))
        .is_some_and(|(left_stem, right_stem)| left_stem == right_stem)
}

fn symbol_base_name(semantic_path: &str) -> String {
    semantic_path
        .rsplit("::")
        .next()
        .unwrap_or(semantic_path)
        .rsplit('.')
        .next()
        .unwrap_or(semantic_path)
        .to_string()
}

fn is_c_header_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "h" | "hpp" | "hh"))
}

fn symbol_meta_from_indexed(symbol: &IndexedSymbol) -> SymbolMeta {
    SymbolMeta::new(
        symbol.symbol_id.clone(),
        symbol.semantic_path.clone(),
        symbol.scope_path.clone(),
        symbol.file_path.clone(),
        symbol.node_kind.clone(),
        "workspace_symbol".to_string(),
        symbol.byte_range,
        symbol.signature.clone(),
        symbol.parameters.clone(),
        symbol.return_type.clone(),
        symbol.docstring.clone(),
        Vec::new(),
        Vec::new(),
    )
}

fn raw_symbol_indexes_by_id(raw_symbols: &[IndexedSymbol]) -> BTreeMap<String, Vec<usize>> {
    let mut indexes = BTreeMap::new();
    for (index, symbol) in raw_symbols.iter().enumerate() {
        indexes
            .entry(symbol.symbol_id.clone())
            .or_insert_with(Vec::new)
            .push(index);
    }
    indexes
}

fn resolve_dependencies_for_symbol(
    symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    name_index: &BTreeMap<String, Vec<usize>>,
) -> Vec<String> {
    let mut dependencies = BTreeSet::new();
    for reference_name in &symbol.references_by_name {
        if let Some(target_symbol_id) =
            resolve_reference_path(reference_name, symbol, raw_symbols, name_index)
        {
            if target_symbol_id != symbol.symbol_id {
                dependencies.insert(target_symbol_id);
            }
        }
    }
    dependencies.into_iter().collect()
}

fn resolve_symbol_dependencies(raw_symbols: &[IndexedSymbol]) -> Vec<SymbolMeta> {
    let name_index = build_name_index(raw_symbols);
    let symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let mut dependency_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for (symbol_id, indexes) in &symbol_indexes {
        let dependencies = dependency_map.entry(symbol_id.clone()).or_default();
        for index in indexes {
            dependencies.extend(
                resolve_dependencies_for_symbol(&raw_symbols[*index], raw_symbols, &name_index)
                    .into_iter(),
            );
        }
    }

    let mut reference_map: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (caller, callees) in &dependency_map {
        for callee in callees {
            reference_map
                .entry(callee.clone())
                .or_default()
                .insert(caller.clone());
        }
    }

    raw_symbols
        .iter()
        .map(|symbol| {
            SymbolMeta::new(
                symbol.symbol_id.clone(),
                symbol.semantic_path.clone(),
                symbol.scope_path.clone(),
                symbol.file_path.clone(),
                symbol.node_kind.clone(),
                "workspace_symbol".to_string(),
                symbol.byte_range,
                symbol.signature.clone(),
                symbol.parameters.clone(),
                symbol.return_type.clone(),
                symbol.docstring.clone(),
                dependency_map
                    .get(&symbol.symbol_id)
                    .map(|dependencies| dependencies.iter().cloned().collect())
                    .unwrap_or_default(),
                reference_map
                    .get(&symbol.symbol_id)
                    .map(|references| references.iter().cloned().collect())
                    .unwrap_or_default(),
            )
        })
        .collect()
}

fn impacted_symbol_ids(
    raw_symbols: &[IndexedSymbol],
    old_changed_symbols: &[IndexedSymbol],
    new_changed_symbols: &[IndexedSymbol],
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    changed_file_paths: &BTreeSet<String>,
) -> BTreeSet<String> {
    let impacted_names: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .map(|symbol| symbol.base_name.clone())
        .collect();
    let changed_reference_names: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .flat_map(|symbol| {
            symbol
                .references_by_name
                .iter()
                .map(|reference| reference_base_name(reference))
                .collect::<Vec<_>>()
        })
        .collect();

    let mut impacted_ids: BTreeSet<_> = old_changed_symbols
        .iter()
        .chain(new_changed_symbols.iter())
        .map(|symbol| symbol.symbol_id.clone())
        .collect();

    for symbol in raw_symbols {
        if changed_file_paths.contains(&symbol.file_path) {
            continue;
        }
        if symbol.base_name.as_str().is_empty() {
            continue;
        }
        if symbol
            .references_by_name
            .iter()
            .any(|reference_name| impacted_names.contains(&reference_base_name(reference_name)))
            || changed_reference_names.contains(&symbol.base_name)
        {
            impacted_ids.insert(symbol.symbol_id.clone());
        }
    }

    let seed_ids: Vec<_> = impacted_ids.iter().cloned().collect();
    for symbol_id in seed_ids {
        if let Some(symbol) = old_resolved_map.get(&symbol_id) {
            impacted_ids.extend(symbol.dependencies.iter().cloned());
            impacted_ids.extend(symbol.references.iter().cloned());
        }
    }

    impacted_ids
}

fn refresh_resolved_symbol_subgraph(
    raw_symbols: &[IndexedSymbol],
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    old_changed_symbols: &[IndexedSymbol],
    new_changed_symbols: &[IndexedSymbol],
    changed_file_paths: &BTreeSet<String>,
) -> (BTreeMap<String, SymbolMeta>, BTreeSet<String>) {
    let name_index = build_name_index(raw_symbols);
    let raw_symbol_indexes = raw_symbol_indexes_by_id(raw_symbols);
    let representative_raw_symbols = raw_symbol_map(raw_symbols);
    let impacted_ids = impacted_symbol_ids(
        raw_symbols,
        old_changed_symbols,
        new_changed_symbols,
        old_resolved_map,
        changed_file_paths,
    );

    let mut resolved_map = old_resolved_map.clone();
    for symbol in old_changed_symbols {
        resolved_map.remove(&symbol.symbol_id);
    }

    for impacted_id in &impacted_ids {
        let Some(raw_symbol) = representative_raw_symbols.get(impacted_id) else {
            resolved_map.remove(impacted_id);
            continue;
        };

        let Some(indexes) = raw_symbol_indexes.get(impacted_id) else {
            continue;
        };

        let mut symbol = symbol_meta_from_indexed(raw_symbol);
        let mut dependencies = BTreeSet::new();
        for index in indexes {
            dependencies.extend(
                resolve_dependencies_for_symbol(&raw_symbols[*index], raw_symbols, &name_index)
                    .into_iter(),
            );
        }
        symbol.dependencies = dependencies.into_iter().collect();
        resolved_map.insert(impacted_id.clone(), symbol);
    }

    let reference_impacted_paths =
        reference_impacted_paths(old_resolved_map, &resolved_map, &impacted_ids);

    for impacted_path in reference_impacted_paths {
        let callers = resolved_map
            .iter()
            .filter_map(|(caller_path, symbol)| {
                symbol
                    .dependencies
                    .iter()
                    .any(|dependency| dependency == &impacted_path)
                    .then_some(caller_path.clone())
            })
            .collect::<Vec<_>>();

        if let Some(symbol) = resolved_map.get_mut(&impacted_path) {
            symbol.references = callers;
        }
    }

    (resolved_map, impacted_ids)
}

fn reference_impacted_paths(
    old_resolved_map: &BTreeMap<String, SymbolMeta>,
    new_resolved_map: &BTreeMap<String, SymbolMeta>,
    impacted_paths: &BTreeSet<String>,
) -> BTreeSet<String> {
    let mut reference_paths = impacted_paths.clone();

    for impacted_path in impacted_paths {
        if let Some(symbol) = old_resolved_map.get(impacted_path) {
            reference_paths.extend(symbol.dependencies.iter().cloned());
            reference_paths.extend(symbol.references.iter().cloned());
        }
        if let Some(symbol) = new_resolved_map.get(impacted_path) {
            reference_paths.extend(symbol.dependencies.iter().cloned());
            reference_paths.extend(symbol.references.iter().cloned());
        }
    }

    reference_paths
}

fn materialize_resolved_symbol_rows(
    raw_symbols: &[IndexedSymbol],
    resolved_map: &BTreeMap<String, SymbolMeta>,
) -> Vec<SymbolMeta> {
    raw_symbols
        .iter()
        .filter_map(|raw_symbol| {
            resolved_map
                .get(&raw_symbol.symbol_id)
                .map(|resolved_symbol| {
                    SymbolMeta::new(
                        raw_symbol.symbol_id.clone(),
                        raw_symbol.semantic_path.clone(),
                        raw_symbol.scope_path.clone(),
                        raw_symbol.file_path.clone(),
                        raw_symbol.node_kind.clone(),
                        "workspace_symbol".to_string(),
                        raw_symbol.byte_range,
                        raw_symbol.signature.clone(),
                        raw_symbol.parameters.clone(),
                        raw_symbol.return_type.clone(),
                        raw_symbol.docstring.clone(),
                        resolved_symbol.dependencies.clone(),
                        resolved_symbol.references.clone(),
                    )
                })
        })
        .collect()
}

fn trace_from_symbols(
    resolved_symbols: &[SymbolMeta],
    indexed_files: usize,
    symbol_path: &str,
    direction: TraceDirection,
) -> Result<TraceSymbolGraphResult> {
    let symbol = choose_trace_symbol(resolved_symbols, symbol_path)
        .cloned()
        .ok_or_else(|| anyhow!("symbol not found in workspace index: {symbol_path}"))?
        .with_origin_type("trace_root");

    let callers = if matches!(direction, TraceDirection::Callers | TraceDirection::Both) {
        summarize_symbols(resolved_symbols, &symbol.references, None)
    } else {
        Vec::new()
    };

    let callees = if matches!(direction, TraceDirection::Callees | TraceDirection::Both) {
        summarize_symbols(
            resolved_symbols,
            &symbol.dependencies,
            Some(&symbol.file_path),
        )
    } else {
        Vec::new()
    };

    Ok(TraceSymbolGraphResult {
        evidence_keys: trace_evidence_keys(&symbol, &callers, &callees),
        symbol,
        callers,
        callees,
        indexed_files,
    })
}

fn trace_evidence_keys(
    symbol: &SymbolMeta,
    callers: &[SymbolSummary],
    callees: &[SymbolSummary],
) -> TraceEvidenceKeys {
    TraceEvidenceKeys {
        symbol: symbol.evidence_key.clone(),
        callers: callers
            .iter()
            .map(|summary| summary.evidence_key.clone())
            .collect(),
        callees: callees
            .iter()
            .map(|summary| summary.evidence_key.clone())
            .collect(),
    }
}

fn persist_symbol_index(
    db_path: &Path,
    workspace_root: &Path,
    raw_symbols: &[IndexedSymbol],
    symbols: &[SymbolMeta],
    file_states: &[PersistedFileState],
    indexed_files: usize,
) -> Result<()> {
    let connection = Connection::open(db_path)?;
    ensure_symbol_tables(&connection)?;

    connection.execute(
        "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [normalize_path(workspace_root)],
    )?;
    connection.execute(
        "INSERT INTO metadata(key, value) VALUES('indexed_files', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [indexed_files.to_string()],
    )?;

    let tx = connection.unchecked_transaction()?;
    tx.execute("DELETE FROM symbols", [])?;
    tx.execute("DELETE FROM file_state", [])?;
    let raw_symbol_rows = raw_symbol_row_map(raw_symbols);
    {
        let mut statement = tx.prepare(
            "INSERT INTO symbols (
                symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json, reference_names_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;

        for symbol in symbols {
            let raw_symbol = raw_symbol_rows
                .get(&symbol_row_key(symbol))
                .ok_or_else(|| anyhow!("missing raw symbol for {}", symbol.semantic_path))?;
            statement.execute(params![
                symbol.symbol_id,
                symbol.semantic_path,
                symbol.scope_path,
                symbol.file_path,
                symbol.node_kind,
                symbol.byte_range.0 as i64,
                symbol.byte_range.1 as i64,
                symbol.signature,
                serde_json::to_string(&symbol.parameters)?,
                symbol.return_type,
                symbol.docstring,
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                serde_json::to_string(&reference_names(raw_symbol))?,
            ])?;
        }
    }
    {
        let mut statement =
            tx.prepare("INSERT INTO file_state (file_path, fingerprint) VALUES (?1, ?2)")?;

        for file_state in file_states {
            statement.execute(params![file_state.file_path, file_state.fingerprint as i64])?;
        }
    }
    tx.commit()?;
    Ok(())
}

fn persist_symbol_refresh(
    db_path: &Path,
    workspace_root: &Path,
    raw_symbols: &[IndexedSymbol],
    symbols: &[SymbolMeta],
    file_states: &BTreeMap<String, u64>,
    changed_file_paths: &BTreeSet<String>,
    impacted_paths: &BTreeSet<String>,
    indexed_files: usize,
) -> Result<()> {
    let connection = Connection::open(db_path)?;
    ensure_symbol_tables(&connection)?;

    connection.execute(
        "INSERT INTO metadata(key, value) VALUES('workspace_root', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [normalize_path(workspace_root)],
    )?;
    connection.execute(
        "INSERT INTO metadata(key, value) VALUES('indexed_files', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [indexed_files.to_string()],
    )?;

    let raw_symbol_rows = raw_symbol_row_map(raw_symbols);
    let resolved_symbol_map = resolved_symbol_map(symbols);
    let changed_symbols: Vec<_> = symbols
        .iter()
        .filter(|symbol| changed_file_paths.contains(&symbol.file_path))
        .cloned()
        .collect();

    let tx = connection.unchecked_transaction()?;
    {
        let mut delete_statement = tx.prepare("DELETE FROM symbols WHERE file_path = ?1")?;
        for changed_file_path in changed_file_paths {
            delete_statement.execute([changed_file_path])?;
        }
    }

    {
        let mut insert_statement = tx.prepare(
            "INSERT INTO symbols (
                symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json, reference_names_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        )?;

        for symbol in &changed_symbols {
            let raw_symbol = raw_symbol_rows
                .get(&symbol_row_key(symbol))
                .ok_or_else(|| anyhow!("missing raw symbol for {}", symbol.semantic_path))?;
            insert_statement.execute(params![
                symbol.symbol_id,
                symbol.semantic_path,
                symbol.scope_path,
                symbol.file_path,
                symbol.node_kind,
                symbol.byte_range.0 as i64,
                symbol.byte_range.1 as i64,
                symbol.signature,
                serde_json::to_string(&symbol.parameters)?,
                symbol.return_type,
                symbol.docstring,
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                serde_json::to_string(&reference_names(raw_symbol))?,
            ])?;
        }
    }

    {
        let mut update_statement = tx.prepare(
            "UPDATE symbols
             SET dependencies_json = ?1, references_json = ?2
             WHERE symbol_id = ?3",
        )?;

        for impacted_path in impacted_paths {
            let Some(symbol) = resolved_symbol_map.get(impacted_path) else {
                continue;
            };
            if changed_file_paths.contains(&symbol.file_path) {
                continue;
            }
            update_statement.execute(params![
                serde_json::to_string(&symbol.dependencies)?,
                serde_json::to_string(&symbol.references)?,
                symbol.symbol_id,
            ])?;
        }
    }

    for changed_file_path in changed_file_paths {
        tx.execute(
            "DELETE FROM file_state WHERE file_path = ?1",
            [changed_file_path],
        )?;
        if let Some(fingerprint) = file_states.get(changed_file_path) {
            tx.execute(
                "INSERT INTO file_state (file_path, fingerprint) VALUES (?1, ?2)",
                params![changed_file_path, *fingerprint as i64],
            )?;
        }
    }

    tx.commit()?;
    Ok(())
}

fn load_symbol_index(db_path: &Path) -> Result<(Vec<SymbolMeta>, usize)> {
    if !db_path.exists() {
        return Err(anyhow!("symbol index {} does not exist", db_path.display()));
    }

    let connection = Connection::open(db_path)?;
    ensure_symbol_tables(&connection)?;
    load_symbols_from_connection(&connection)
}

fn ensure_symbol_tables(connection: &Connection) -> Result<()> {
    connection.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        CREATE TABLE IF NOT EXISTS metadata (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS symbols (
            symbol_id TEXT NOT NULL,
            semantic_path TEXT NOT NULL,
            scope_path TEXT,
            file_path TEXT NOT NULL,
            node_kind TEXT NOT NULL,
            start_byte INTEGER NOT NULL,
            end_byte INTEGER NOT NULL,
            signature TEXT,
            parameters_json TEXT NOT NULL DEFAULT '[]',
            return_type TEXT,
            docstring TEXT,
            dependencies_json TEXT NOT NULL,
            references_json TEXT NOT NULL,
            reference_names_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (semantic_path, file_path)
        );
        CREATE TABLE IF NOT EXISTS file_state (
            file_path TEXT PRIMARY KEY,
            fingerprint INTEGER NOT NULL
        );
        ",
    )?;
    ensure_reference_names_column(connection)?;
    ensure_symbol_id_column(connection)?;
    ensure_scope_path_column(connection)?;
    ensure_parameters_json_column(connection)?;
    ensure_return_type_column(connection)?;
    ensure_docstring_column(connection)?;
    ensure_symbols_primary_key_layout(connection)?;
    Ok(())
}

fn load_file_states(connection: &Connection) -> Result<BTreeMap<String, u64>> {
    let mut statement =
        connection.prepare("SELECT file_path, fingerprint FROM file_state ORDER BY file_path")?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as u64))
    })?;

    let mut states = BTreeMap::new();
    for row in rows {
        let (file_path, fingerprint) = row?;
        states.insert(file_path, fingerprint);
    }
    Ok(states)
}

fn load_indexed_symbols_grouped_by_file(
    connection: &Connection,
) -> Result<BTreeMap<String, Vec<IndexedSymbol>>> {
    let mut statement = connection.prepare(
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, reference_names_json
         FROM symbols
         ORDER BY file_path, semantic_path",
    )?;
    let rows = statement.query_map([], |row| {
        let parameters_json: String = row.get(8)?;
        let reference_names_json: String = row.get(11)?;
        let parameters: Vec<String> = serde_json::from_str(&parameters_json).unwrap_or_default();
        let reference_names: Vec<String> =
            serde_json::from_str(&reference_names_json).unwrap_or_default();
        let semantic_path: String = row.get(1)?;
        Ok(IndexedSymbol {
            symbol_id: row.get(0)?,
            base_name: symbol_base_name(&semantic_path),
            semantic_path,
            scope_path: row.get(2)?,
            file_path: row.get(3)?,
            node_kind: row.get(4)?,
            byte_range: (
                row.get::<_, i64>(5)? as usize,
                row.get::<_, i64>(6)? as usize,
            ),
            signature: row.get(7)?,
            parameters,
            return_type: row.get(9)?,
            docstring: row.get(10)?,
            references_by_name: reference_names.into_iter().collect(),
        })
    })?;

    let mut grouped = BTreeMap::new();
    for row in rows {
        let symbol = row?;
        grouped
            .entry(symbol.file_path.clone())
            .or_insert_with(Vec::new)
            .push(symbol);
    }
    Ok(grouped)
}

fn load_symbols_from_connection(connection: &Connection) -> Result<(Vec<SymbolMeta>, usize)> {
    let indexed_files = connection
        .query_row(
            "SELECT value FROM metadata WHERE key = 'indexed_files'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);

    let mut statement = connection.prepare(
        "SELECT symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
                signature, parameters_json, return_type, docstring, dependencies_json,
                references_json
         FROM symbols",
    )?;
    let rows = statement.query_map([], |row| {
        let parameters_json: String = row.get(8)?;
        let dependencies_json: String = row.get(11)?;
        let references_json: String = row.get(12)?;
        Ok(SymbolMeta::new(
            row.get(0)?,
            row.get(1)?,
            row.get(2)?,
            row.get(3)?,
            row.get(4)?,
            "workspace_symbol".to_string(),
            (
                row.get::<_, i64>(5)? as usize,
                row.get::<_, i64>(6)? as usize,
            ),
            row.get(7)?,
            serde_json::from_str(&parameters_json).unwrap_or_default(),
            row.get(9)?,
            row.get(10)?,
            serde_json::from_str(&dependencies_json).unwrap_or_default(),
            serde_json::from_str(&references_json).unwrap_or_default(),
        ))
    })?;

    let mut symbols = Vec::new();
    for row in rows {
        symbols.push(row?);
    }

    Ok((symbols, indexed_files))
}

fn ensure_reference_names_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "reference_names_json" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN reference_names_json TEXT NOT NULL DEFAULT '[]'",
        [],
    )?;
    Ok(())
}

fn ensure_symbol_id_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "symbol_id" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN symbol_id TEXT NOT NULL DEFAULT ''",
        [],
    )?;
    connection.execute(
        "UPDATE symbols SET symbol_id = semantic_path WHERE symbol_id = ''",
        [],
    )?;
    Ok(())
}

fn ensure_scope_path_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "scope_path" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN scope_path TEXT", [])?;
    Ok(())
}

fn ensure_parameters_json_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "parameters_json" {
            return Ok(());
        }
    }

    connection.execute(
        "ALTER TABLE symbols ADD COLUMN parameters_json TEXT NOT NULL DEFAULT '[]'",
        [],
    )?;
    Ok(())
}

fn ensure_return_type_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "return_type" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN return_type TEXT", [])?;
    Ok(())
}

fn ensure_docstring_column(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for column in columns {
        if column? == "docstring" {
            return Ok(());
        }
    }

    connection.execute("ALTER TABLE symbols ADD COLUMN docstring TEXT", [])?;
    Ok(())
}

fn ensure_symbols_primary_key_layout(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(symbols)")?;
    let columns = statement.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;

    let mut semantic_path_pk = 0;
    let mut file_path_pk = 0;
    for column in columns {
        let (name, pk_order) = column?;
        match name.as_str() {
            "semantic_path" => semantic_path_pk = pk_order,
            "file_path" => file_path_pk = pk_order,
            _ => {}
        }
    }

    if semantic_path_pk == 1 && file_path_pk == 2 {
        return Ok(());
    }

    if semantic_path_pk == 0 && file_path_pk == 0 {
        return Ok(());
    }

    connection.execute_batch(
        "
        ALTER TABLE symbols RENAME TO symbols_legacy;
        CREATE TABLE symbols (
            symbol_id TEXT NOT NULL,
            semantic_path TEXT NOT NULL,
            scope_path TEXT,
            file_path TEXT NOT NULL,
            node_kind TEXT NOT NULL,
            start_byte INTEGER NOT NULL,
            end_byte INTEGER NOT NULL,
            signature TEXT,
            parameters_json TEXT NOT NULL DEFAULT '[]',
            return_type TEXT,
            docstring TEXT,
            dependencies_json TEXT NOT NULL,
            references_json TEXT NOT NULL,
            reference_names_json TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (semantic_path, file_path)
        );
        INSERT INTO symbols (
            symbol_id, semantic_path, scope_path, file_path, node_kind, start_byte, end_byte,
            signature, parameters_json, return_type, docstring, dependencies_json,
            references_json, reference_names_json
        )
        SELECT
            COALESCE(NULLIF(symbol_id, ''), semantic_path),
            semantic_path, scope_path, file_path, node_kind, start_byte, end_byte, signature,
            COALESCE(parameters_json, '[]'), return_type, docstring,
            dependencies_json, references_json,
            COALESCE(reference_names_json, '[]')
        FROM symbols_legacy;
        DROP TABLE symbols_legacy;
        ",
    )?;
    Ok(())
}

fn raw_symbol_map(symbols: &[IndexedSymbol]) -> BTreeMap<String, IndexedSymbol> {
    let mut map = BTreeMap::new();
    for symbol in symbols {
        map.entry(symbol.symbol_id.clone())
            .and_modify(|existing| {
                if indexed_symbol_rank(symbol) > indexed_symbol_rank(existing) {
                    *existing = symbol.clone();
                }
            })
            .or_insert_with(|| symbol.clone());
    }
    map
}

fn raw_symbol_row_map(
    symbols: &[IndexedSymbol],
) -> BTreeMap<(String, String, usize, usize), IndexedSymbol> {
    symbols
        .iter()
        .cloned()
        .map(|symbol| {
            (
                (
                    symbol.semantic_path.clone(),
                    symbol.file_path.clone(),
                    symbol.byte_range.0,
                    symbol.byte_range.1,
                ),
                symbol,
            )
        })
        .collect()
}

fn resolved_symbol_map(symbols: &[SymbolMeta]) -> BTreeMap<String, SymbolMeta> {
    let mut map = BTreeMap::new();
    for symbol in symbols {
        map.entry(symbol.symbol_id.clone())
            .and_modify(|existing| {
                if resolved_symbol_rank(symbol) > resolved_symbol_rank(existing) {
                    *existing = symbol.clone();
                }
            })
            .or_insert_with(|| symbol.clone());
    }
    map
}

fn choose_trace_symbol<'a>(symbols: &'a [SymbolMeta], symbol_path: &str) -> Option<&'a SymbolMeta> {
    symbols
        .iter()
        .filter(|symbol| symbol.symbol_id == symbol_path || symbol.semantic_path == symbol_path)
        .max_by_key(|symbol| resolved_symbol_rank(symbol))
}

fn reference_names(symbol: &IndexedSymbol) -> Vec<String> {
    symbol.references_by_name.iter().cloned().collect()
}

fn reference_base_name(reference_name: &str) -> String {
    reference_name
        .rsplit('.')
        .next()
        .unwrap_or(reference_name)
        .to_string()
}

fn symbol_row_key(symbol: &SymbolMeta) -> (String, String, usize, usize) {
    (
        symbol.semantic_path.clone(),
        symbol.file_path.clone(),
        symbol.byte_range.0,
        symbol.byte_range.1,
    )
}

fn indexed_symbol_rank(symbol: &IndexedSymbol) -> usize {
    symbol_kind_rank(&symbol.node_kind)
}

fn resolved_symbol_rank(symbol: &SymbolMeta) -> usize {
    symbol_kind_rank(&symbol.node_kind)
}

fn symbol_kind_rank(node_kind: &str) -> usize {
    match node_kind {
        "function_definition" => 3,
        "class_definition" => 3,
        "type_definition" => 2,
        "declaration" => 1,
        _ => 0,
    }
}

fn c_include_context_for_file(file_path: &str) -> Result<CIncludeContext> {
    let path = Path::new(file_path);
    if detect_language(path).ok() != Some(LanguageId::C) {
        return Ok(CIncludeContext::default());
    }

    let mut include_paths = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_c_include_closure(path, &mut include_paths, &mut visited)?;

    let companion_source_paths = include_paths
        .iter()
        .filter_map(|include_path| companion_c_source_path(include_path))
        .collect();

    Ok(CIncludeContext {
        include_paths,
        companion_source_paths,
    })
}

fn collect_c_include_closure(
    path: &Path,
    include_paths: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
) -> Result<()> {
    let normalized_path = normalize_path(path);
    if !visited.insert(normalized_path) {
        return Ok(());
    }

    let source = read_source(path)?;
    let document = parse_document(path, &source)?;
    for include_target in c_include_targets(document.tree.root_node(), &source)? {
        let Some(include_path) = resolve_local_c_include(path, &include_target) else {
            continue;
        };
        let normalized_include = normalize_path(&include_path);
        if include_paths.insert(normalized_include) {
            collect_c_include_closure(&include_path, include_paths, visited)?;
        }
    }

    Ok(())
}

fn companion_c_source_path(include_path: &str) -> Option<String> {
    let path = Path::new(include_path);
    let extension = path.extension()?.to_str()?;
    if !matches!(extension, "h" | "hpp" | "hh") {
        return None;
    }

    let candidate = path.with_extension("c");
    candidate.exists().then(|| normalize_path(&candidate))
}

fn source_fingerprint(source: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    hasher.finish()
}
