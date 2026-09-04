# Language Onboarding Template

This checklist is the standard path to add a new source language to Arborist. It keeps the capability-flag policy honest and avoids making the gateway/PyO3 layers carry language logic. See [gap audit](language-gaps.md) and [design note](multi-language-support-design.md) for background.

## 1. Choose a tree-sitter grammar

- Add the grammar crate to the workspace `Cargo.toml` (`[workspace.dependencies]`) and to `crates/arborist-core/Cargo.toml`.
- Prefer cratype that is compatible with the pinned `tree-sitter` version and still exposes a unit `Language` value usable through the repository's existing `fn <lang>_grammar() -> Language` pattern.
- If a grammar exposes multiple dialects (PHP `php`/`php_only`, TypeScript `typescript`/`tsx`), each dialect may be registered as a separate `LanguageId` sharing adapters; each `LanguageDescriptor` gets its own extension list.
- Update `Cargo.lock` through `cargo test --locked` or `cargo build`; do not hand-edit the lockfile.

## 2. Add stable language identity

- Extend `LanguageId` in `crates/arborist-core/src/model/primitives.rs`) with a serde-stable snake_case name.
- Add the persisted id mapping in `persisted_language_id()` of `language/registry.rs`.
- If the language does not belong to an existing group, add a new arm in `language_family_id()`.
- Add language-family tests where the registry exposes the new id in `supported_languages()` and extension routing.

## 3. Register extensions

- Define an extension constant near the other constants in `language/registry.rs` (for example `PHP_EXTENSIONS`).
- Populate it with the file extensions routed to this language; extension matching is case-insensitive and rejects duplicates.
- Watch parenthesized collisions: e.g. C/C++ headers, TS/TSX, `.h` and `.m` must stay unambiguous.
- If an extension should map to a grammar family but needs a distinct language id, register aseparate `LanguageDescriptor`.

## 4. Implement the grammar function

- Add `fn <lang>_grammar() -> Language` in `registry.rs` next to the other grammar functions.
- For a family with the same grammar (JS/TS/TSX), keep one `LanguageDescriptor` per id but share one `LanguageAdapter` struct.
- Add a smoke unit test: `parse_document` on a fixture succeeds and returns the language id.

## 5. Implement semantic skeleton slice

- Create `crates/arborist-core/src/semantic/<lang>.rs` following the Java/C#/Kotlin style: collect symbol nodes, compute `semantic_path`, `scope_path`, `signature`, `parameters`, `return_type`, `docstring`(nullable), and always populate `symbol_id`.
- Export helpers used by symbol extraction. The helpers should be `pub(crate)` and reused by the extractor, not reimplemented.
- `docstring` can be `None` initially, but the other fields should be stable once a node kind is emitted.
- dd unit tests asserting available semantic paths and `node_kind` values on at least one fixture.

## 6. Implement symbol indexing slice

- Create `crates/arborist-core/src/symbol_extractor/<lang>.rs` mirroring the Java/Go extractor shape: one `index_<lang>_symbols_with_deadline` entry, recursive/named-field handling as needed.
- Populate `IndexedSymbol` fields: `semantic_path`, `scope_path`, `file_path`, `node_kind`, `byte_range`, `signature`, `parameters`, `return_type`, `docstring`, `references_by_name`, `call_arities_by_name`, `is_overload`.
- Wire the extractor in the adapter's `extract_symbols` method.
- Add fixtures and unit tests for declarations, overload flags, referenced names, and arity facts.

## 7. Add local dependency extraction

- Create `crates/arborist-core/src/language/<lang>.rs` for language-specific import/module/include resolution.
- Prefer a `*_with_deadline` wrapper that checks the deadline before/while walking; reuse existing `*_with_deadline` patterns.
- For the first slice it is acceptable to return an empty dependency list until file-level resolution is implemented, but the capability flag must not claim `FILE_DEPENDENCIES` then.
- If file dependencies exist, add fixtures for resolved paths, unresolved paths, and no-dependency files.

## 8. Patch targeting/validation

- Implement at least patch targeting: `find_semantic_node`, `ascend_to_symbol`, `position_symbol_identity`, `semantic_path_for_node`, `symbol_id_for_node`, and patch replacement normalization.
- Do not set `PATCH_VALIDATION` until binding validation is real. A language can ship with `PATCH_TARGETING` only and docs annotated `syntax-targeting` / `no validation yet`.
- If binding validation is implemented, add tests for valid renamed/inserted bindings and invalid dangling references.

## 9. Register adapter and capability flags

- Define `<LANG>_DESCRIPTOR` with `analysis_revision` (e.g. `<lang>-v1`) and the exact combined capabilities; do notdefault to `FULL_CURRENT_SUPPORT`.
- Define `<LANG>_ADAPTER`; if using a syntax-only base, wrap it in a new adapter struct or reuse the family adapter when behavior is shared.
- Append the adapter to the `builtin()` array; keep array order deterministic so `supported_languages()`/provenance output remains stable.
- Assert that no extension or language id collides (existing `LanguageRegistry::new` panics on duplicates)。

## 10. Docs, catalog, fixtures, and commits

- Update `docs/tools.md` language support section with the exact extension list, capability depth,conservative slices,and any deferred behavior.
- Update `docs/language-gaps.md` matrix for the new language.
- Regenerate/verify catalog: `python scripts/tool_catalog.py --check` and (when output changes) `python -m arborist_mcp.gateway --dump-tool-catalog`.
- Add fixtures under `crates/arborist-core/tests/fixtures/languages/<lang>/` (malformed、direct_calls、ambiguity_*、resolver_*以及overloads if applicable) and index them in that directory's README.
- Keep related commits granular: dependency/manifest、core modules、fixtures/tests、docs/catalog as needed. Push after roughly every 10-15 commits and wait for CI before continuing (see the rollout plan for the exact Git/CI rhythm).