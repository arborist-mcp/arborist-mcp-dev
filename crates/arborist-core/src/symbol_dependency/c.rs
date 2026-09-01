use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::Result;

use crate::deadline::DeadlineCheck;
use crate::language::{
    c_companion_source_path_with_deadline, c_include_targets, c_include_targets_with_offsets,
    detect_language, is_c_header_path, normalize_path, parse_document, parse_document_with_timeout,
    read_source, resolve_local_c_include,
};
use crate::model::LanguageId;
use crate::symbol_index_model::IndexedSymbol;
use crate::workspace_scan::WorkspaceScanDeadline;

#[derive(Debug, Default)]
pub(crate) struct CIncludeContext {
    pub(crate) include_paths: BTreeSet<String>,
    pub(crate) companion_source_paths: BTreeSet<String>,
}

pub(crate) fn c_include_context_for_file_with_overrides_and_deadline(
    file_path: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<CIncludeContext> {
    let path = Path::new(file_path);
    if !matches!(
        detect_language(path).ok(),
        Some(LanguageId::C | LanguageId::Cpp)
    ) {
        return Ok(CIncludeContext::default());
    }

    let mut include_paths = BTreeSet::new();
    let mut visited = BTreeSet::new();
    collect_c_include_closure_with_overrides(
        path,
        &mut include_paths,
        &mut visited,
        file_overrides,
        deadline,
    )?;

    if let Some(deadline) = deadline {
        deadline.check("building C include context")?;
    }
    let mut companion_source_paths = BTreeSet::new();
    for include_path in &include_paths {
        if let Some(candidate) =
            c_companion_source_path_with_deadline(Path::new(include_path), deadline)?
        {
            companion_source_paths.insert(normalize_path(&candidate));
        }
    }

    Ok(CIncludeContext {
        include_paths,
        companion_source_paths,
    })
}

/// A per-resolution-pass cache of each parsed C/C++ source file's include
/// targets (with their byte offsets). Resolution walks the same overlay or
/// indexed file once per reference; memoizing the parse here avoids re-parsing
/// the same source hundreds of times per pass (the parse dominated C++ overlay
/// trace cost).
#[derive(Default)]
pub(crate) struct CIncludeTargetsCache {
    by_file: HashMap<String, Vec<(usize, String)>>,
}

impl CIncludeTargetsCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

pub(crate) fn c_include_context_for_file_before_with_overrides(
    file_path: &str,
    byte_offset: usize,
    file_overrides: Option<&BTreeMap<String, String>>,
    include_targets_cache: &mut CIncludeTargetsCache,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<CIncludeContext> {
    if let Some(deadline) = deadline {
        deadline.check("building C include context")?;
    }
    let path = Path::new(file_path);
    if !matches!(
        detect_language(path).ok(),
        Some(LanguageId::C | LanguageId::Cpp)
    ) {
        return Ok(CIncludeContext::default());
    }

    let mut include_paths = BTreeSet::new();
    let mut visited = BTreeSet::from([normalize_path(path)]);
    for include_target in c_include_targets_before_cached(
        path,
        file_overrides,
        byte_offset,
        include_targets_cache,
        deadline,
    )? {
        let Some(include_path) =
            resolve_local_c_include_with_overrides(path, &include_target, file_overrides)
        else {
            continue;
        };
        let normalized_include = normalize_path(&include_path);
        if include_paths.insert(normalized_include) {
            collect_c_include_closure_with_overrides(
                &include_path,
                &mut include_paths,
                &mut visited,
                file_overrides,
                deadline,
            )?;
        }
    }

    if let Some(deadline) = deadline {
        deadline.check("building C include context")?;
    }
    let mut companion_source_paths = BTreeSet::new();
    for include_path in &include_paths {
        if let Some(candidate) =
            c_companion_source_path_with_deadline(Path::new(include_path), deadline)?
        {
            companion_source_paths.insert(normalize_path(&candidate));
        }
    }

    Ok(CIncludeContext {
        include_paths,
        companion_source_paths,
    })
}

fn c_include_targets_before_cached(
    path: &Path,
    file_overrides: Option<&BTreeMap<String, String>>,
    byte_offset: usize,
    include_targets_cache: &mut CIncludeTargetsCache,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<Vec<String>> {
    let key = normalize_path(path);
    if !include_targets_cache.by_file.contains_key(&key) {
        if let Some(deadline) = deadline {
            deadline.check("parsing C include context")?;
        }
        let source = source_for_path(path, file_overrides)?;
        if let Some(deadline) = deadline {
            deadline.check("parsing C include context")?;
        }
        let document = match deadline {
            Some(deadline) => parse_document_with_timeout(
                path,
                &source,
                deadline
                    .remaining_timeout_micros("parsing C include context")?
                    .unwrap_or(0),
            )?,
            None => parse_document(path, &source)?,
        };
        let targets = c_include_targets_with_offsets(document.tree.root_node(), &source)?;
        include_targets_cache.by_file.insert(key.clone(), targets);
    }
    Ok(include_targets_cache.by_file[&key]
        .iter()
        .filter(|(offset, _)| *offset < byte_offset)
        .map(|(_, target)| target.clone())
        .collect())
}

pub(super) fn c_symbol_family_anchor_with_deadline(
    symbol: &IndexedSymbol,
    raw_symbols: &[IndexedSymbol],
    deadline: Option<&WorkspaceScanDeadline>,
) -> Result<String> {
    if let Some(deadline) = deadline {
        deadline.check("resolving C/C++ symbol identities")?;
    }
    let include_context = c_include_context_for_file_with_overrides_and_deadline(
        &symbol.file_path,
        None,
        deadline.map(|deadline| deadline as &dyn DeadlineCheck),
    )?;
    let source_path = Path::new(&symbol.file_path);

    let best_header = raw_symbols
        .iter()
        .filter(|candidate| {
            candidate.semantic_path == symbol.semantic_path
                && !candidate.semantic_path.contains("::")
                && is_c_header_path(Path::new(&candidate.file_path))
        })
        .map(|candidate| {
            let rank = c_family_header_rank(source_path, &candidate.file_path, &include_context);
            (candidate, rank)
        })
        .filter(|(_, rank)| *rank > 0)
        .max_by_key(|(_, rank)| *rank)
        .map(|(candidate, _)| candidate);

    Ok(best_header
        .map(|candidate| candidate.file_path.clone())
        .unwrap_or_else(|| symbol.file_path.clone()))
}

pub(super) fn same_stem(left: &Path, right: &Path) -> bool {
    left.file_stem()
        .and_then(|stem| stem.to_str())
        .zip(right.file_stem().and_then(|stem| stem.to_str()))
        .is_some_and(|(left_stem, right_stem)| left_stem == right_stem)
}

fn collect_c_include_closure_with_overrides(
    path: &Path,
    include_paths: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    file_overrides: Option<&BTreeMap<String, String>>,
    deadline: Option<&dyn DeadlineCheck>,
) -> Result<()> {
    if let Some(deadline) = deadline {
        deadline.check("building C include context")?;
    }
    let normalized_path = normalize_path(path);
    if !visited.insert(normalized_path) {
        return Ok(());
    }

    let source = source_for_path(path, file_overrides)?;
    let document = match deadline {
        Some(deadline) => parse_document_with_timeout(
            path,
            &source,
            deadline
                .remaining_timeout_micros("parsing C include context")?
                .unwrap_or(0),
        )?,
        None => parse_document(path, &source)?,
    };
    if let Some(deadline) = deadline {
        deadline.check("extracting C include context")?;
    }
    for include_target in c_include_targets(document.tree.root_node(), &source)? {
        let Some(include_path) =
            resolve_local_c_include_with_overrides(path, &include_target, file_overrides)
        else {
            continue;
        };
        let normalized_include = normalize_path(&include_path);
        if include_paths.insert(normalized_include) {
            collect_c_include_closure_with_overrides(
                &include_path,
                include_paths,
                visited,
                file_overrides,
                deadline,
            )?;
        }
    }

    Ok(())
}

fn source_for_path(
    path: &Path,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Result<String> {
    file_overrides
        .and_then(|overrides| overrides.get(&normalize_path(path)))
        .cloned()
        .map(Ok)
        .unwrap_or_else(|| read_source(path))
}

fn resolve_local_c_include_with_overrides(
    current_path: &Path,
    include_target: &str,
    file_overrides: Option<&BTreeMap<String, String>>,
) -> Option<std::path::PathBuf> {
    resolve_local_c_include(current_path, include_target).or_else(|| {
        let parent = current_path.parent()?;
        let candidate =
            crate::language::normalize_absolute_path(&parent.join(include_target)).ok()?;
        file_overrides
            .is_some_and(|overrides| overrides.contains_key(&normalize_path(&candidate)))
            .then_some(candidate)
    })
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use anyhow::Result;

    use super::{
        CIncludeTargetsCache, c_include_context_for_file_before_with_overrides,
        c_include_context_for_file_with_overrides_and_deadline,
    };
    use crate::deadline::DeadlineCheck;
    use crate::language::{normalize_path, write_source_atomic};

    struct RejectCompanionSourceScan;

    impl DeadlineCheck for RejectCompanionSourceScan {
        fn check(&self, phase: &str) -> Result<()> {
            if phase == "scanning C/C++ companion source paths" {
                anyhow::bail!("test deadline expired during {phase}");
            }
            Ok(())
        }
    }

    #[test]
    fn include_context_reads_source_overrides() {
        let root = std::env::temp_dir().join(format!(
            "arborist-c-include-overrides-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temporary include workspace should be created");
        let source_path = root.join("main.c");
        let header_path = root.join("extra.h");
        write_source_atomic(&source_path, "int main(void) { return 0; }\n")
            .expect("source should be written");
        write_source_atomic(&header_path, "#define EXTRA 1\n").expect("header should be written");

        let mut overrides = BTreeMap::new();
        overrides.insert(
            normalize_path(&source_path),
            "#include \"extra.h\"\nint main(void) { return EXTRA; }\n".to_owned(),
        );

        let context = c_include_context_for_file_with_overrides_and_deadline(
            &normalize_path(&source_path),
            Some(&overrides),
            None,
        )
        .expect("override include context should load");

        assert!(
            context
                .include_paths
                .contains(&normalize_path(&header_path))
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn include_context_companion_source_scan_honors_deadline() {
        let root = std::env::temp_dir().join(format!(
            "arborist-c-include-companion-deadline-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temporary include workspace should be created");
        let header = root.join("helper.h");
        let source_path = root.join("main.c");
        write_source_atomic(&header, "int helper(int value);\n").expect("header should be written");
        write_source_atomic(
            &source_path,
            "#include \"helper.h\"\nint main(void) { return helper(1); }\n",
        )
        .expect("source should be written");

        let error = c_include_context_for_file_with_overrides_and_deadline(
            &normalize_path(&source_path),
            None,
            Some(&RejectCompanionSourceScan),
        )
        .expect_err("companion source scanning must honor the deadline");

        assert!(
            error
                .to_string()
                .contains("test deadline expired during scanning C/C++ companion source paths")
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn include_context_before_offset_memoizes_parse_across_calls() {
        let root = std::env::temp_dir().join(format!(
            "arborist-c-include-before-cache-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temporary include workspace should be created");
        let source_path = root.join("main.c");
        let first_header = root.join("first.h");
        let second_header = root.join("second.h");
        let source = "#include \"first.h\"\nint before(void) { return 0; }\n#include \"second.h\"\nint after(void) { return 0; }\n";
        write_source_atomic(&source_path, source).expect("source should be written");
        write_source_atomic(&first_header, "#define FIRST 1\n").expect("first header");
        write_source_atomic(&second_header, "#define SECOND 1\n").expect("second header");

        let normalized_source = normalize_path(&source_path);
        let before_offset = source.find("int before").expect("before symbol");
        let after_offset = source.find("int after").expect("after symbol");
        let mut cache = CIncludeTargetsCache::new();

        let before = c_include_context_for_file_before_with_overrides(
            &normalized_source,
            before_offset,
            None,
            &mut cache,
            None,
        )
        .expect("before include context should load");
        assert!(
            before
                .include_paths
                .contains(&normalize_path(&first_header))
        );
        assert!(
            !before
                .include_paths
                .contains(&normalize_path(&second_header))
        );

        let after = c_include_context_for_file_before_with_overrides(
            &normalized_source,
            after_offset,
            None,
            &mut cache,
            None,
        )
        .expect("after include context should load");
        assert!(after.include_paths.contains(&normalize_path(&first_header)));
        assert!(
            after
                .include_paths
                .contains(&normalize_path(&second_header))
        );

        // A fresh cache still returns the same per-offset visibility.
        let mut fresh_cache = CIncludeTargetsCache::new();
        let fresh_before = c_include_context_for_file_before_with_overrides(
            &normalized_source,
            before_offset,
            None,
            &mut fresh_cache,
            None,
        )
        .expect("fresh before include context should load");
        assert!(
            fresh_before
                .include_paths
                .contains(&normalize_path(&first_header))
        );
        assert!(
            !fresh_before
                .include_paths
                .contains(&normalize_path(&second_header))
        );

        let _ = fs::remove_dir_all(&root);
    }
}
