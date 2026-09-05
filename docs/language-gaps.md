# Language Capability Gap Audit

**Status:** Living baseline for the multi-language support rollout. Edit this file as capability work lands; keep it pinned to the current registry resources (`crates/arborist-core/src/language/registry.rs`) rather than duplicating policy. See also `multi-language-support-design.md` and `language-onboarding.md`.

## Capability dimensions

Arborist models language support through a `LanguageCapabilities` bitmask; every language should converge toward `FULL_CURRENT_SUPPORT` unless a gap below explicitly blocks a capability dimension. Dimensions:

- Tree-sitter queries (`TREE_QUERY`)
- Semantic skeletons (`SEMANTIC_SKELETON`)
- Symbol indexing (`SYMBOL_INDEX`)
- Local file dependencies (`FILE_DEPENDENCIES`)
- Reference tracing (`REFERENCE_TRACE`)
- Patch targeting (`PATCH_TARGETING`)
- Patch binding validation (`PATCH_VALIDATION`).

## Current capability summary

The registry's `FULL_CURRENT_SUPPORT` currently names all builtin languages: Python, C, C++, JavaScript/TypeScript/TSX, Rust, Go, Java, Kotlin, C#,and Lua/PHP/Swift. However, a green capability flag is not the same as deep behavioral coverage; many languages implement a conservative slice for a flag. This audit records the behavioral gaps that remain after the flag is set.

| Language | Known behavioral gaps | Suggested first work |
| --- | --- | --- |
| Python | Local dependency resolution is static; virtualenv marker/site-packages and import-hook indirection not modeled | Keep current scope; preserve regressions |
| C | C++ overload/ambiguous include resolution remains bounded; header/source companion discovery is conservative | Keep current scope; preserve regressions |
| C++ | Broader overload resolution and template/specialization dispatch remain open | Expand C++ overload slices; add fixtures before behavior changes |
| JavaScript | Dynamic imports, bundler aliases, framework injection, and rich type-driven dispatch not modeled | Keep conservative direct-call/export-chain behavior; add TODO fixtures if needed |
| TypeScript | Same dynamic/type-driven gaps as JavaScript; type-only imports are handled in patch binding already | Keep parity with JavaScript family |
| TSX | Same gaps as TypeScript; JSX member dispatch stays conservative | Keep parity with TypeScript |
| Rust | Cross-file/import trace resolutionand patch binding validation are conservative; macro expansionand trait-method dispatch remain open | Extend local-module dependency refresh and direct-call/inline-module-qualified trace; then broader patch binding |
| Go | Same-package/factory/interface slices are described in `multi-language-support-design.md`; general interface dispatch, embedding/implementation dispatch, build-tag modes remain open | Extend imported local-package factory-result calls; then patch binding validation breadth |
| Java | Full Maven/Gradle classpathand type hierarchy resolution not modeled | Keep current scope; preserve regressions |
| Kotlin | Similar JVM classpath gap as Java; standard-library auto-import facts are handled in patch binding | Keep parity with Java; expand patch binding regressions |
| C# | Project/csproj and NuGet graph not modeled; same-namespace cross-file slices are delivered | Keep parity with Java/Go style cross-file method slices |

| Lua | No require/module multi-file function graph; only same-file bare direct function calls resolve; no overloads/scope qualification; patch validation remains conservative same-file direct-call only | Keep conservative slice; add fixtures for require/module/resolver extension flows when real |
| PHP | No namespace/class/static method graph; only same-file bare top-level function calls resolve; no overloads/scope qualification; patch validation remains conservative same-file direct-call only | Keep conservative slice; add fixtures for namespace/class and resolver extension flows when real |
| Swift | No class/type graph; only top-level `func` declarations produce semantic skeletons; no namespace/class/static/instance method graph, indexing, dependencies, tracing, or patch validation | Add fixtures for Swift top-level function skeletons;then extend with class/struct/extension scopes when real |
## What “Full” means here

`FULL_CURRENT_SUPPORT` means the registry will dispatch all seven capability dimensions to the adapter. It does not claim:

- dependency graphs resolve every ecosystem manifest (cargo, Maven/Gradle, csproj/NuGet, package.json, Gemfile, etc.)
- tracing resolves build systems, dynamic imports, type hierarchies, or virtual environments
- patch validation accepts every valid edit or rejects every invalid edit

All such boundaries Should be described in `docs/tools.md` per language rather than implied by a bitmask.

## Phase targets

Stage A closes the conservative slices that are already advertised but under-tested in Rust/Go/JavaScript/TypeScript/TSX:

1. Audit this file against each adapter's `analysis_revision` after any registry change.
2. Add language fixtures for any gapthat changes public output(skeleton/index/trace/patch preview).
3. Bump/adjust `analysis_revision` strings when a language's behavior changes substantively.
