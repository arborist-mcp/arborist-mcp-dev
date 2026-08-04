# Multi-Language Support Design

**Status:** Phases 1-4 implemented; Phase 5 Rust and Go skeleton/index/dependency/trace slices, Java query/semantic-skeleton/declaration-indexing/import-dependency/direct-trace slices, and C# parsing/raw-query/semantic-skeleton/declaration-indexing/local/root-and-exact-namespace-alias-static-and-namespace-import/global-alias-static-namespace-import-and-base-constructor/base-method/local-and-global-base-alias-import/cross-file-same-namespace-and-nested-source trace slices implemented
**Audience:** Arborist core, PyO3, gateway, and release maintainers
**Scope:** Rust core architecture for adding source languages without weakening existing Python, C, and C++ behavior.

## 1. Summary

Arborist currently parses and analyzes Python, C, and C++ through a Rust core built around Tree-sitter. It exposes semantic skeletons, AST patching and validation, symbol indexing, dependency tracing, source overlays, and a persisted SQLite index through a thin PyO3 and MCP gateway stack.

Adding languages by extending every `match LanguageId` branch would work for one or two additions, but it would make parsing, symbol extraction, reference resolution, patching, index persistence, and incremental refresh increasingly coupled. This design introduces a controlled internal language-adapter layer. It preserves a single Rust-owned analysis pipeline while allowing each language to define its grammar, semantic extraction, dependency facts, reference resolution, and safe patching behavior.

The design has five central rules:

1. **Parse support is not semantic support.** Every advertised operation is explicitly capability-gated per language.
2. **Adapters extract facts; shared services orchestrate them.** Adapters do not write SQLite, construct MCP responses, or bypass workspace/VFS policy.
3. **Reference information is structured.** Language-specific behavior must not be encoded in strings or control-character prefixes.
4. **Resolution is conservative.** An unresolved or ambiguous reference must not become a guessed graph edge.
5. **Persisted indexes record analysis provenance.** A change to language detection, grammar, or adapter semantics makes affected stored data stale.

The recommended delivery order is: establish the adapter substrate with Python, C, and C++ as compatibility implementations; replace encoded reference strings with structured facts; then use JavaScript/TypeScript as the first new-language validation; and finally add Rust, Go, Java, C#, Kotlin, and other languages incrementally.

## 2. Current State And Constraints

### 2.1 Current architecture

The workspace has a deliberate layering that this design preserves:

- `crates/arborist-core` owns parsing, semantic skeletons, AST patching, VFS, symbol extraction, dependency tracing, workspace scanning, and SQLite index persistence.
- `crates/arborist-py` is a thin PyO3 facade over the Rust core.
- `python/arborist_mcp` provides stdio JSON-RPC/MCP transport, request routing, and tool metadata.

Today, source-language routing is extension based. `LanguageId` contains `Python`, `C`, and `Cpp`; `language/parser.rs` maps extensions to a Tree-sitter grammar. Semantic extraction and patch targeting dispatch between a Python path and a shared C-family path. The symbol extractor follows the same pattern. C/C++ include dependencies are handled in a dedicated include graph. The reference resolver contains language-specific Python and C++ branches.

This organization is appropriate for three languages, particularly because C and C++ share substantial syntax and behavior. It becomes harder to maintain once languages with distinct module systems, symbol scopes, overload rules, and patching constraints are added.

### 2.2 Compatibility constraints

The following are non-negotiable during the migration:

- Existing Python, C, and C++ public request and response shapes remain stable unless a separately approved protocol change is made.
- Existing `symbol_id` and `semantic_path` behavior remains stable for existing languages. New internal identity fields must not silently rewrite public IDs.
- Live workspace, VFS-overlay, and persisted-index paths continue to have equivalent observable behavior.
- The stdio transport remains one JSON document per line.
- Workspace path, source-size, timeout, SQLite, and VFS safety checks remain centralized and fail closed.
- The PyO3 and gateway layers remain adapters, not duplicate implementations of language logic.
- Existing extension behavior is preserved by default. In particular, `.h` remains routed as C unless a future explicit workspace policy says otherwise.

### 2.3 Non-goals

This proposal does not:

- promise complete semantic understanding of every target language;
- provide runtime loading of untrusted language plugins or native libraries;
- require all languages to support tracing or automatic patching at launch;
- implement cross-language symbol resolution by name matching;
- replace Tree-sitter with a compiler frontend or language server;
- make dynamic-language dispatch precise when the source does not provide sufficient static evidence.

## 3. Goals And Delivery Priorities

### 3.1 Product goals

The architecture must support staged language delivery. A language may safely ship with syntax queries, semantic skeletons, and symbol discovery before it has reliable cross-file tracing or structural patching.

| Priority | Languages | Rationale |
| --- | --- | --- |
| P0 | JavaScript, TypeScript, Rust, Go, Java | High ecosystem coverage and a strong fit for Arborist's symbol/index/patch model. |
| P1 | C#, Kotlin, PHP, Ruby, Swift, Objective-C/Objective-C++ | Valuable ecosystems with additional dispatch or project-model complexity. |
| P2 | Dart, Scala, Elixir/Erlang, Clojure, Lua, Zig, CUDA/OpenCL, Bash, PowerShell | Add according to demonstrated user demand and analysis maturity. |
| Auxiliary | SQL, HCL/Terraform, Dockerfile, YAML, JSON, TOML, XML, HTML/CSS, Markdown, notebooks | Support first as structured configuration, templates, or dependency/reference inputs rather than full code graph languages. |

### 3.2 Engineering goals

The implementation must:

1. centralize language recognition and capability reporting;
2. prevent a new language from requiring broad edits across unrelated modules;
3. make adapter contracts testable with common fixtures and invariants;
4. retain language-specific precision without leaking implementation encodings into shared data structures;
5. make index invalidation deterministic when an adapter or grammar changes;
6. keep all unsupported or ambiguous analysis results conservative and visible.

## 4. Terminology

| Term | Meaning |
| --- | --- |
| **Language** | A stable built-in source language such as Python, C++, TypeScript, or Rust. |
| **Adapter** | The Rust implementation that provides language-specific parsing and analysis behavior. |
| **Registry** | The built-in mapping from language identifiers and detection rules to adapters. |
| **Capability** | A named operation that a language is explicitly allowed to serve. |
| **Document** | A path, source string, language selection, and parsed Tree-sitter tree. |
| **Symbol draft** | Language-extracted, non-persisted information about one symbol. |
| **Reference fact** | Structured evidence that a symbol imports, calls, reads, uses, or otherwise refers to another name/path. |
| **Resolver** | The logic that maps a reference fact to one, many, or no indexed symbols. |
| **File dependency** | A source-level relationship such as C `#include`, Python import, or TypeScript import. |
| **Analysis revision** | A stable version string covering grammar and semantic extraction behavior for an adapter. |

## 5. Capability Model

A language must advertise each independently supported operation. Parsing a file is insufficient evidence that tracing or patching it is correct.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageCapabilities(u32);

impl LanguageCapabilities {
    pub const TREE_QUERY: Self = Self(1 << 0);
    pub const SEMANTIC_SKELETON: Self = Self(1 << 1);
    pub const SYMBOL_INDEX: Self = Self(1 << 2);
    pub const FILE_DEPENDENCIES: Self = Self(1 << 3);
    pub const REFERENCE_TRACE: Self = Self(1 << 4);
    pub const PATCH_TARGETING: Self = Self(1 << 5);
    pub const PATCH_VALIDATION: Self = Self(1 << 6);
}
```

The exact Rust representation may use an existing local bitflag pattern, but its semantic contract must remain the same.

### 5.1 Capability meanings

| Capability | Contract |
| --- | --- |
| `TREE_QUERY` | The file can be parsed and queried through the existing Tree-sitter query API. |
| `SEMANTIC_SKELETON` | The adapter can produce stable, validated skeleton output with symbols and byte ranges. |
| `SYMBOL_INDEX` | The adapter can extract stable indexed symbols appropriate for live and persisted lookups. |
| `FILE_DEPENDENCIES` | The adapter can extract source-level file/module dependencies suitable for incremental refresh. |
| `REFERENCE_TRACE` | The adapter can provide sufficiently conservative reference facts and resolution logic for symbol tracing. |
| `PATCH_TARGETING` | The adapter can map position/path selections to structural replacement targets. |
| `PATCH_VALIDATION` | The adapter can normalize a proposed replacement, apply it in virtual source, and verify safe syntax-level constraints. |

Every public operation that needs a capability checks it before invoking the adapter. For example, requesting `trace_symbol_graph` for an adapter without `REFERENCE_TRACE` returns a clear unsupported-operation error. It must not return an empty graph, because an empty graph communicates a materially different result.

```rust
fn require_capability(
    descriptor: &LanguageDescriptor,
    required: LanguageCapabilities,
    operation: &'static str,
) -> Result<()> {
    if !descriptor.capabilities.contains(required) {
        bail!("{operation} is not supported for {} files", descriptor.display_name);
    }
    Ok(())
}
```

### 5.2 Initial capability targets

| Language family | Initial capability target | Deferred work |
| --- | --- | --- |
| Python, C, C++ | Preserve current behavior | Migration only; no intentional reduction. |
| JavaScript/TypeScript | Query, skeleton, symbols, conservative direct-call trace including static local named imports and named re-export chains, and static local module dependency refresh; default/namespace resolution and patch targeting/validation follow in later adapter slices. | Dynamic imports, bundler aliases, framework injection, rich type-driven dispatch. |
| Rust | Query, skeleton, symbols, module dependencies, patch targeting; selected direct-call trace | Macro expansion, trait-method dispatch, complete Cargo feature resolution. |
| Go | Query, skeleton, symbols, package imports, direct-call trace, patch targeting | Interface dispatch and build-tag-aware workspace modes. |
| Java | Query, skeleton, symbols, package/import dependencies, direct-call trace, patch targeting | Full Maven/Gradle classpath and type hierarchy resolution. |
| Configuration/template languages | Query and selected structured references | General symbol graph and automatic code patching. |

## 6. Architecture Overview

```text
                         +--------------------+
                         | LanguageRegistry   |
                         | detection + policy |
                         +----------+---------+
                                    |
                                    v
+-------------------+    +---------------------------+
| workspace / VFS   | -> | common parse orchestration| -> ParsedDocument
| paths + limits    |    | size + deadline controls  |
+-------------------+    +-------------+-------------+
                                        |
           +----------------------------+----------------------------+
           |                             |                            |
           v                             v                            v
+----------------------+      +----------------------+    +-----------------------+
| semantic skeleton    |      | symbol extraction    |    | file dependency facts |
| capability gate      |      | capability gate      |    | capability gate       |
+----------+-----------+      +----------+-----------+    +----------+------------+
           |                             |                            |
           +--------------------+--------+----------------------------+
                                |
                                v
                 +-------------------------------+
                 | shared index and graph layer  |
                 | identities, resolver dispatch,|
                 | live/VFS/SQLite parity        |
                 +---------------+---------------+
                                 |
                                 v
               +------------------------------------+
               | patch preview / source query / API  |
               | Rust public API -> PyO3 -> gateway  |
               +------------------------------------+
```

The registry is the only place that chooses a language. The common pipeline is the only place that enforces workspace, VFS, source-size, deadline, and storage policy. Adapters only operate on already validated documents.

## 7. Language Registry And Detection

### 7.1 Stable language identifiers

`LanguageId` remains an explicit internal enum. It should gain stable serde names for persistence rather than relying on declaration order.

```rust
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum LanguageId {
    Python,
    C,
    Cpp,
    JavaScript,
    TypeScript,
    Rust,
    Go,
    Java,
    CSharp,
    Kotlin,
}
```

New variants may be added; existing values must not be renamed or reused. The project should not add `Other(String)`: accepting arbitrary names would make grammar loading, index compatibility, test coverage, and security guarantees indeterminate.

### 7.2 Descriptors and registry

A descriptor declares stable metadata and the public capability contract of one adapter.

```rust
pub struct LanguageDescriptor {
    pub id: LanguageId,
    pub display_name: &'static str,
    pub extensions: &'static [&'static str],
    pub capabilities: LanguageCapabilities,
    pub analysis_revision: &'static str,
    pub detection_priority: u8,
}

pub struct LanguageRegistry {
    adapters: BTreeMap<LanguageId, &'static dyn LanguageAdapter>,
    extensions: BTreeMap<String, Vec<LanguageId>>,
}
```

`analysis_revision` changes when the locked Tree-sitter grammar behavior, skeleton extraction, symbol identity generation, reference-fact extraction, dependency extraction, or patch normalization changes in a way that may alter analysis output.

The initial registry is static and compiled into the native module:

```rust
pub fn builtin_language_registry() -> &'static LanguageRegistry;
```

Runtime loading of arbitrary adapter DLLs/shared libraries is explicitly out of scope. It would introduce ABI compatibility, code-execution, reproducibility, and wheel-packaging risks. Future optional language support should use Cargo features and a controlled release artifact, not untrusted runtime plugins.

### 7.3 Detection policy

Language detection returns its evidence rather than only an enum value:

```rust
pub enum DetectionEvidence {
    WorkspaceOverride,
    Extension,
    ContentProbe,
}

pub struct DetectedLanguage {
    pub id: LanguageId,
    pub evidence: DetectionEvidence,
}
```

Detection order is:

1. explicit workspace override;
2. unambiguous file extension;
3. narrowly scoped content probe, only for explicitly supported ambiguities;
4. error when no reliable result exists.

A future workspace configuration may supply overrides such as:

```json
{
  "language_overrides": {
    "**/*.h": "cpp",
    "ios/**/*.m": "objective_c"
  }
}
```

This configuration must be opt-in. Absent an override, existing behavior is preserved, including routing `.h` through the C grammar.

## 8. Adapter Interface And Shared Document

The registry and adapters operate on a common borrowed document representation:

```rust
pub struct AnalysisDocument<'a> {
    pub path: &'a Path,
    pub source: &'a str,
    pub language_id: LanguageId,
    pub tree: &'a tree_sitter::Tree,
}
```

The common parser orchestration remains responsible for path normalization and containment, VFS source overlays, source-size limits, parser construction, parse deadlines, and consistent error mapping. An adapter receives no writable workspace or database handle.

```rust
pub trait LanguageAdapter: Send + Sync {
    fn descriptor(&self) -> &'static LanguageDescriptor;
    fn tree_sitter_language(&self) -> tree_sitter::Language;

    fn build_semantic_skeleton(
        &self,
        document: AnalysisDocument<'_>,
        request: SemanticSkeletonRequest<'_>,
    ) -> Result<SemanticSkeleton>;

    fn extract_symbols(
        &self,
        document: AnalysisDocument<'_>,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<Vec<IndexedSymbolDraft>>;

    fn collect_file_dependencies(
        &self,
        document: AnalysisDocument<'_>,
        deadline: Option<&WorkspaceScanDeadline>,
    ) -> Result<FileDependencies>;

    fn find_symbol_node(
        &self,
        document: AnalysisDocument<'_>,
        selector: SymbolSelector<'_>,
        deadline: Option<&dyn DeadlineCheck>,
    ) -> Result<Option<tree_sitter::Node<'_>>>;

    fn normalize_patch(
        &self,
        request: PatchNormalizationRequest<'_>,
    ) -> Result<NormalizedPatch>;

    fn resolve_reference(
        &self,
        request: ResolveReferenceRequest<'_>,
        candidates: &SymbolCandidateIndex,
    ) -> Result<ResolutionOutcome>;
}
```

The final API may split this trait into focused subtraits if that makes Rust lifetimes or module visibility clearer. The contract is that entry points share a validated document context, capabilities are checked before invocation, and each operation has one language-owned implementation.

## 9. Symbols And Structured Reference Facts

### 9.1 Problem statement

The current raw index model stores reference names and call arities separately. That represents only a subset of the information needed by modern languages and forces C++ receiver/value-category details into internal string encodings. Such encodings are difficult to validate, migrate, test, or extend safely.

### 9.2 Reference fact model

Replace encoded references with a structured, serializable representation.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReferenceFact {
    pub kind: ReferenceKind,
    pub spelling: ReferenceSpelling,
    pub byte_range: ByteRange,
    pub lexical_scope: Option<String>,
    pub call: Option<CallContext>,
    pub import: Option<ImportContext>,
    pub receiver: Option<ReceiverContext>,
    pub language_details: ReferenceLanguageDetails,
}

pub enum ReferenceKind {
    Call,
    ValueRead,
    TypeUse,
    Inheritance,
    Import,
    Include,
    MacroUse,
}

pub struct ReferenceSpelling {
    pub raw: String,
    pub segments: Vec<String>,
    pub qualification: Qualification,
}

pub enum Qualification {
    Unqualified,
    Relative,
    ModuleQualified,
    NamespaceQualified,
    MemberQualified,
}

pub struct CallContext {
    pub argument_count: usize,
    pub has_spread_arguments: bool,
    pub is_constructor: bool,
}
```

The actual model should reuse existing range and validation primitives where available. Fields are omitted only when they are not reliably known; an adapter must not fabricate type or dispatch information.

### 9.3 Controlled language details

Most information should be expressed through common fields. Cases that cannot be generalized remain explicit and versioned:

```rust
pub enum ReferenceLanguageDetails {
    None,
    Python(PythonReferenceDetails),
    Cpp(CppReferenceDetails),
    TypeScript(TypeScriptReferenceDetails),
}
```

This is intentionally more restrictive than arbitrary JSON metadata. A typed variant makes its persistence schema, migration needs, semantics, and test coverage visible in review. It also prevents new adapters from creating hidden string protocols.

### 9.4 Symbol drafts and internal identity

Adapters return symbol drafts rather than final persisted rows:

```rust
pub struct IndexedSymbolDraft {
    pub public_symbol_id: String,
    pub semantic_path: String,
    pub base_name: String,
    pub scope_path: Option<String>,
    pub language_id: LanguageId,
    pub file_path: String,
    pub node_kind: String,
    pub byte_range: ByteRange,
    pub signature: Option<String>,
    pub is_overload: bool,
    pub parameters: Vec<String>,
    pub return_type: Option<String>,
    pub docstring: Option<String>,
    pub references: Vec<ReferenceFact>,
}

pub struct SymbolKey {
    pub language_id: LanguageId,
    pub public_symbol_id: String,
    pub file_path: String,
    pub start_byte: usize,
    pub end_byte: usize,
}
```

Existing public `symbol_id` values stay unchanged. `SymbolKey` is used for internal graph and storage uniqueness so same-named symbols in different languages cannot collide accidentally.

## 10. Conservative Reference Resolution

Resolution is dispatched according to the **caller language**, because the caller determines how a spelling is interpreted.

```rust
pub struct ResolveReferenceRequest<'a> {
    pub caller: &'a IndexedSymbol,
    pub reference: &'a ReferenceFact,
    pub workspace: &'a WorkspaceResolutionContext,
}

pub enum ResolutionOutcome {
    Resolved(SymbolKey),
    Ambiguous(Vec<SymbolKey>),
    Unresolved,
    Unsupported,
}
```

| Outcome | Graph behavior |
| --- | --- |
| `Resolved` | Add a dependency edge. |
| `Ambiguous` | Add no edge; preserve an optional diagnostic for future inspection. |
| `Unresolved` | Add no edge. |
| `Unsupported` | Add no edge and preserve the language capability boundary. |

### 10.1 Cross-language policy

The default resolver considers only candidates in the same language. Explicit, tested bridges may relax that rule for tightly related language pairs:

- C and C++ through include and `extern "C"`-aware behavior;
- JavaScript and TypeScript as one JS/TS module family;
- Objective-C++ with C++ and Objective-C after dedicated adapter support;
- Kotlin and Java only after an explicit JVM interop model exists.

Python-to-native extensions, Java JNI, Node native bindings, FFI, reflection, `eval`, and framework injection remain unresolved unless a future dedicated bridge can provide sound enough rules. Name equality alone is never sufficient to create a cross-language edge.

## 11. File Dependencies And Incremental Refresh

File dependencies are not limited to C/C++ includes. Introduce a language-neutral result from which the existing C include graph can evolve.

```rust
pub struct FileDependencies {
    pub entries: Vec<FileDependency>,
}

pub struct FileDependency {
    pub kind: FileDependencyKind,
    pub raw_specifier: String,
    pub resolved_path: Option<PathBuf>,
    pub resolution: DependencyResolution,
}

pub enum FileDependencyKind {
    Include,
    Import,
    Module,
    Package,
}
```

| Family | Dependency source |
| --- | --- |
| C/C++ | `#include`, keeping current literal-preprocessor branch behavior. |
| Python | `import` and `from ... import`, resolved only within known source/package roots. |
| JavaScript/TypeScript | static `import`, re-export-from, and a conservative `require` subset. |
| Rust | `mod` declarations and optionally manifest-informed workspace roots. |
| Go | package import paths and workspace module boundaries. |
| Java/Kotlin | package/import facts and configured source roots. |

The existing C include-dependent refresh behavior now runs through a generic local-file-dependency reverse index while retaining its tested implementation internally. JavaScript/TypeScript contributes static relative imports, re-exports, and direct literal `require` calls that resolve to local JS/TS-family files. Direct calls through local named imports (including aliases) are resolved only to matching symbols in the imported module. Static named `export { name as alias } from "./module"` chains are followed recursively with cycle detection; missing, non-local, and cyclic bindings fail closed instead of falling back to same-named workspace symbols. Dynamic imports, package specifiers, escaped literals, star re-exports, default/namespace symbol resolution, and bundler aliases remain unresolved. Languages are added one at a time; an adapter without `FILE_DEPENDENCIES` does not contribute reverse-refresh paths.

## 12. Patching Model

The existing patch workflow remains shared:

1. validate source and resolve the file through live/VFS state;
2. parse with the selected adapter grammar;
3. resolve a structural symbol or position target;
4. normalize the replacement only when language rules require it;
5. build the virtual result;
6. parse and validate the virtual result;
7. return a preview or commit through the existing safe write path.

Adapters own only language-specific behavior, such as Python indentation normalization or a TypeScript declaration target definition. The common layer owns edit-batch validation, byte-range checks, virtual-file behavior, syntax reparse, error mapping, and atomic filesystem writes.

A language must not advertise `PATCH_TARGETING` or `PATCH_VALIDATION` until it has tests for both successful replacements and rejection/rollback paths.

## 13. Persisted Index Compatibility

### 13.1 Required provenance

A persisted index needs enough metadata to determine whether a stored file was analyzed using the same language behavior as the running build. Store at least:

- `language_id`;
- `analysis_revision`;
- language-detection policy fingerprint;
- raw reference-fact schema revision, if facts are persisted.

The current implementation stores a deterministic index-level `analysis_provenance` metadata record. It contains a provenance-schema revision, every selectable canonical language ID and its adapter analysis revision, the extension-routing fingerprint, and the persisted reference-fact schema revision. Index-level validation is sufficient while routing is built in and workspace language overrides do not exist; a future override surface must extend provenance to record the selected language per affected file.

Every current-schema persisted read, rebuild, and refresh validates this record before loading persisted symbols. Missing, malformed, or mismatched provenance fails closed and requires a rebuild; valid historical v1-v5 indexes may be migrated explicitly to the current schema.

### 13.2 Invalidation rules

Affected files must be refreshed or an index marked stale when any of the following change:

- the selected language for a path;
- a workspace language override;
- adapter semantic extraction revision;
- Tree-sitter grammar behavior relevant to analysis;
- reference-fact persistence schema;
- supported file-dependency behavior.

No implicit rewrite is permitted for an unrecognized, incomplete, foreign, or invalid SQLite database. The existing fail-closed inspection and explicit migration model remains mandatory.

### 13.3 Migration strategy

The move from encoded reference names to `ReferenceFact` requires an index schema migration. It should be shipped separately from adding the first new language:

1. introduce the new in-memory fact model while retaining current external results for Python/C/C++;
2. add a schema version and explicit migration implementation;
3. migrate valid known Arborist indexes only;
4. rebuild affected files or the full index when old data cannot be translated exactly;
5. add tests for valid migration, foreign databases, incomplete schema, unknown versions, corrupted JSON, and stale analysis revisions.

## 14. Public API And Tooling Impact

The adapter layer is internal. Existing MCP tool names, request shapes, and response shapes need not change merely because the registry exists.

Potential future public additions should be deliberate and versioned:

- reporting supported languages and per-language capabilities;
- exposing a detected language in inspection output;
- exposing `language` on symbol results as an optional additive field;
- allowing workspace language overrides through a separately designed config surface.

If any of these become public, update the tool manifest/specifications, result schemas, `docs/tools.md`, `docs/protocol.md`, `README.md`, the generated tool catalog, and protocol regression tests together.

## 15. Proposed Module Layout

The final layout should preserve existing focused modules rather than create one large abstraction file.

```text
crates/arborist-core/src/
  language/
    registry.rs
    detection.rs
    capabilities.rs
    document.rs
    adapter.rs
    adapters/
      python.rs
      c.rs
      cpp.rs
      typescript.rs
      javascript.rs
      rust.rs
      go.rs
      java.rs

  semantic/
    python.rs
    c/
    typescript.rs
    rust.rs

  symbol_extractor/
    python.rs
    c.rs
    typescript.rs
    rust.rs

  symbol_dependency/
    facts.rs
    resolution/
      python.rs
      c_family.rs
      typescript.rs
      rust.rs

  file_dependency/
    c_family.rs
    python.rs
    typescript.rs
    rust.rs
```

Existing Python, C, and C++ implementations should be moved or wrapped only when doing so preserves their tests. The registry should compose focused language implementations rather than duplicate semantic, extraction, and resolution logic inside an adapter monolith.

## 16. Implementation Plan

### Phase 0: Contract baseline

Before architectural changes, capture and strengthen current Python/C/C++ contracts:

- extension routing;
- parser timeout and oversize-source behavior;
- skeleton content and symbol/path alignment;
- stable symbol IDs and overload behavior;
- live, VFS, and persisted-index parity;
- index refresh behavior for C/C++ includes;
- symbol trace behavior;
- patch preview, syntax validation, and rollback behavior;
- SQLite health and migration failures.

**Exit criterion:** baseline tests pass with no intentional behavior changes.

### Phase 1: Registry-only migration

Add `LanguageRegistry`, descriptors, and capability declarations for Python, C, and C++. Route detection, grammar selection, and `supported_languages()` through the registry. Keep existing semantic/index/patch code paths otherwise intact.

**Exit criterion:** all existing supported paths produce the same results and `.h` keeps its current C default.

### Phase 2: Adapter composition

Wrap or move existing language-specific implementation behind the adapter interface. Replace scattered language dispatch in semantic skeleton extraction, symbol extraction, symbol-node targeting, and parser selection. Keep C/C++ shared helpers intact beneath their separate adapters where that preserves current behavior.

**Exit criterion:** no user-visible behavior changes; adding a placeholder adapter requires a localized registration and implementation change rather than broad edits across core services.

### Phase 3: Structured reference facts

Introduce `ReferenceFact`, adapt Python/C/C++ extraction, and update the resolver to consume facts rather than encoded names and arities. Implement the index schema migration and stale-data checks.

**Exit criterion:** Python/C/C++ dependency graph fixtures remain compatible; control-character reference encodings are removed from shared models.

### Phase 4: JavaScript and TypeScript

Add `tree-sitter-javascript` and `tree-sitter-typescript` using the repository's existing dependency and lockfile conventions. Implement the first external adapter with a conservative initial capability set.

The first delivered slices register JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`), TypeScript (`.ts`, `.mts`, `.cts`), and TSX (`.tsx`) grammars for parsing, Tree-sitter queries, semantic skeletons, conservative direct-call symbol indexing/tracing, static local module dependency extraction, direct calls through static local named imports, direct named re-export chains, and structural patching. The first patch slice supports semantic and position targets with syntax-level validation; language-specific JavaScript/TypeScript reference-binding validation remains deferred. Star re-export/default/namespace import resolution remains explicitly withheld until its adapter support is implemented.

Initial scope:

- JavaScript: `.js`, `.jsx`, `.mjs`, `.cjs`;
- TypeScript: `.ts`, `.tsx`, `.mts`, `.cts`;
- functions, classes, methods, interfaces, enums, and exports as applicable;
- static imports, re-export-from, and conservative direct `require` handling;
- direct calls where names and module ownership can be resolved confidently;
- symbol/position patch targeting and syntax validation.

Explicitly defer arbitrary dynamic import/require behavior, bundler aliases, `eval`, decorator/framework injection, and type-driven dispatch.

**Exit criterion:** JS/TS contract fixtures cover parsing, skeletons, indexes, imports, conservative trace results, overlays, persisted indexes, and patches.

### Phase 5: Rust, Go, and Java

Add one language per reviewable sequence. Each language starts with discovery and indexing, then gains dependency and trace capability only after dedicated fixtures establish safe behavior. The current Rust slice supports `.rs` parsing, raw Tree-sitter queries, semantic skeletons, declaration indexing for named modules, functions, structs, enums, traits, type aliases, constants, statics, trait signatures, and inherent-`impl` methods, plus local module dependency refresh for unambiguous out-of-line `mod` declarations. It traces unshadowed bare direct calls to functions in the same source-file module and qualified direct calls to functions in inline modules in the same source file. Trait-implementation members are not indexed; path-semantic modules and `use` paths do not create dependency edges. Out-of-line module, Cargo, and import resolution, plus patching, remain capability-gated until dedicated adapters and fixtures establish safe behavior. The current Go slice supports `.go` parsing, raw Tree-sitter queries, semantic skeletons, conservative declaration indexing for named type specifications and aliases, functions, and methods with named local receiver types, and source-position identity. It refreshes importers only for static imports strictly below the nearest valid simple `go.mod` module path, mapping the import to direct `.go` files in that package directory. It also traces unshadowed bare direct calls to top-level functions declared in the same source file and unambiguous direct calls to functions through local package imports, using an explicit alias or the imported package's declared name. Module-root imports, external modules, `replace`, `go.work`, vendoring, build tags, general cross-file/package/import resolution, method dispatch, and patching remain capability-gated until their own fixtures establish safe behavior. Java now contributes `.java` routing, raw Tree-sitter query execution, package-qualified semantic skeletons, and conservative declaration indexing for top-level and nested classes, interfaces, enums, annotation types, methods, and constructors. Java overload IDs are assigned only for duplicate methods and constructors within one file. It refreshes importers for explicit local type imports and explicit single-member static imports whose owning type resolves to a local `.java` file under an ancestor source root; wildcard imports, static wildcard imports, missing, and ambiguous source paths fail closed. It traces unqualified and `this.method()` calls only when one same-type, same-file, non-varargs method has the call arity; `Type.method()` calls through a unique explicit non-static local type import when the type name is unshadowed; and bare calls through a unique explicit local static-method import only when no same-type method has that name. Imported targets must be static with exactly one non-varargs arity match. Static field/type imports, instance/member dispatch, overload type selection, and patching remain capability-gated until dedicated resolution and workspace fixtures establish safe behavior. C# now contributes `.cs` routing, raw Tree-sitter query execution, namespace-qualified semantic skeletons, and conservative declaration indexing for block and file-scoped namespaces, classes, structs, interfaces, enums, records, methods, and constructors. Its skeleton paths support bounded expansion selectors, while C# methods and constructors with duplicate semantic paths receive stable overload identities. It traces an unshadowed unqualified or explicit `this.` method call, a `: this(...)` constructor initializer, a conservative `: base(...)` constructor initializer and direct `base.Method()` call with a simple, `global::`, local, or root-level global type-alias/namespace-import base type, a globally namespace-qualified `global::...` static call, a unique simple same-namespace `Type.Method()` static call across the workspace, including from a nested source type, an explicit type-alias call of the form `Alias.Method()`, a `using static Fully.Qualified.Type;` bare call, a root-level `global using static Fully.Qualified.Type;` bare call contributed by any scanned C# source file (including directive-only files), and a `using Fully.Qualified.Namespace;` call of the form `Type.Method()`, root-level `global using Fully.Qualified.Namespace;` calls, and root-level `global using Alias = Fully.Qualified.Type;` calls contributed by any scanned C# source file, including directive-only files. Type aliases, static imports, and namespace imports may be declared at file root or directly in a block/file-scoped namespace. An alias declared in the caller's exact namespace shadows a root alias with the same local name; root and exact namespace static/ordinary imports are both considered. Duplicate aliases in the same scope, duplicate imports, and ambiguous targets fail closed; outer-namespace alias/import inheritance is not inferred. Same-type unqualified/`this.` forms require one same-file non-`params` target with the call arity; globally qualified, simple same-namespace, and imported static calls may resolve exactly one matching workspace target. Imported targets must have exactly one indexed type declaration, while same-file type-name collisions, competing imported targets, and ambiguous type declarations fail closed. A static import is considered only for a bare call with no same-type method of that name; a namespace import is considered only when no matching receiver type exists in the caller's namespace anywhere in the workspace. Local declarations, lambda parameters, type parameters, and enclosing-type fields/properties/events suppress unqualified and simple type-qualified call facts; qualified targets must be static. `base(...)` and `base.Method()` require one unique class/record base declaration. Simple and `global::` base names, plus unique unshadowed aliases or namespace imports declared at file root, in the caller's exact namespace, or as root-level global usings contributed by scanned C# sources (including directive-only files), are supported. Constructor targets must have an exact-arity, non-`params` match; `base.Method()` targets must be directly declared non-static methods with one exact-arity, non-`params` match. Generic, non-`global::` qualified, ambiguous or colliding alias/import, and non-class/record namespace-import base types fail closed. When any C# source changes, refresh conservatively re-resolves all indexed C# symbols against tracked source paths, including directive-only global-using files, without reindexing unchanged C# sources. Other member dispatch, overload type selection, and patching remain capability-gated until dedicated C# fixtures establish conservative contracts.

Suggested order:

1. Rust: `mod`, `use`, functions, `impl`, traits, associated items;
2. Go: packages, imports, functions, methods, interfaces;
3. Java: establish parsing and raw-query compatibility, then package-qualified semantic skeletons, declaration indexing, conservative explicit type/static-import refresh, same-type/explicit-`this`, explicit-local-type static-call, and explicit-static-import tracing, overload identity, and conservative resolution;
4. C#: parsing, raw-query compatibility, semantic skeletons, declaration indexing, same-type
   unqualified/explicit-`this` method, `this(...)`, conservative `base(...)` constructor-initializer, direct `base.Method()`, local/global alias-base and namespace-import-base tracing, globally qualified static-call tracing,
   cross-file simple same-namespace static-call tracing, including from nested source types, root and exact namespace-scoped
   type-alias/static-import bare-call tracing, root-level global-alias/static/namespace-import tracing from all scanned C# files,
   and namespace-import static-call tracing and conservative dependency refresh are
   established; add broader
   tracing only through dedicated adapter slices; Kotlin after explicit JVM interop assumptions are documented.

## 17. Test And Validation Strategy

### 17.1 Common adapter contract suite

Each adapter must satisfy a reusable test suite covering:

- extension and override detection;
- grammar setup and malformed-source behavior;
- source-size and deadline enforcement;
- valid byte ranges and UTF-8 position behavior;
- skeleton `available_paths` / `available_symbols` consistency;
- stable IDs across unchanged source;
- symbol range and signature invariants;
- capability denial behavior;
- VFS overlay parity;
- persisted-index reload and stale-revision behavior;
- patch preview success and failure paths for adapters advertising patching.

### 17.2 Language fixtures

Create small, targeted fixtures under a dedicated language-oriented test tree:

```text
tests/fixtures/languages/
  python/
  c/
  cpp/
  javascript/
  typescript/
  rust/
  go/
  java/
```

Fixtures should include direct calls, imports, ambiguity, malformed syntax, shadowing, nested scopes, overloads where meaningful, and changes that require incremental refresh.

### 17.3 Resolver safety tests

For every adapter with `REFERENCE_TRACE`, test that:

- a clear same-language target resolves;
- ambiguity produces no accidental edge;
- unresolved names produce no accidental edge;
- disabled cross-language matching produces no accidental edge;
- every approved bridge has positive and negative fixtures;
- live and persisted graph construction agree.

### 17.4 Fuzzing and resource limits

New grammars and language-specific Tree-sitter queries should be added to the existing fuzz and deadline strategy. At minimum, malformed input and queries must not panic, bypass capture limits, or ignore parse deadlines. New source walkers must check existing cooperative deadlines at bounded intervals.

## 18. Observability And Diagnostics

Diagnostics are valuable internally, but public responses should not become noisy by default. The core should retain structured internal reasons for:

- unsupported capability;
- detection ambiguity;
- unresolved reference;
- ambiguous reference;
- stale analysis revision;
- unsupported cross-language bridge.

Where an existing inspection or debug result can expose this safely, prefer structured category/value fields over prose that callers would need to parse. No diagnostic may include secrets, source contents beyond existing permitted output, or paths outside the allowed workspace.

## 19. Alternatives Considered

### 19.1 Continue extending global `match LanguageId` statements

**Rejected.** This is simple in the short term but makes every new language modify parsing, semantic extraction, indexing, resolution, patching, and refresh paths. It also hides capability boundaries and encourages superficial support declarations.

### 19.2 One generic Tree-sitter query implementation for all languages

**Rejected.** Tree-sitter gives common parse infrastructure, but imports, scopes, overloads, modules, declarations, and patch boundaries vary enough that a generic query-only semantic layer would either be inaccurate or unmaintainable.

### 19.3 Runtime native adapter plugins

**Rejected for the initial architecture.** Native runtime plugins complicate security, ABI stability, distribution, dependency locking, and test reproducibility. Static built-in adapters are sufficient for planned language expansion.

### 19.4 Store arbitrary adapter metadata as JSON

**Rejected.** It hides schema contracts, weakens validation, and recreates the string-encoding problem in another form. Use a shared fact model and explicit, versioned language-detail variants instead.

### 19.5 Resolve cross-language references by basename

**Rejected.** It produces plausible but incorrect graph edges, which is worse than an unresolved edge for patch validation and impact analysis.

## 20. Risks And Mitigations

| Risk | Mitigation |
| --- | --- |
| Adapter abstraction becomes a large generic framework before a real new language validates it. | Migrate in phases and use JS/TS as the first external proof point. |
| Existing C++ precision regresses during structured-reference migration. | Keep C++ fixtures as a hard compatibility baseline; migrate in an isolated change. |
| Persisted indexes mix results from incompatible language behavior. | Persist `language_id` and analysis revision; validate on load; fail closed. |
| Dynamic languages produce misleading call graphs. | Capability-gate trace and resolve only clear static evidence. |
| Project/module resolution becomes dependency-manager-specific. | Start with source-level dependencies; add Cargo, npm, Maven, Gradle, or MSBuild intelligence only as separately bounded work. |
| Added grammars increase build size or wheel complexity. | Add languages individually, use locked dependencies, and validate release artifacts per platform. |
| Public protocol surface drifts while adding languages. | Keep the adapter layer internal; update manifests, catalog, docs, and protocol tests together for deliberate public changes. |

## 21. Acceptance Criteria

The multi-language substrate is ready for the first new language when all of the following are true:

- Python, C, and C++ retain their documented behavior and regression coverage.
- Language recognition, descriptors, grammar selection, and capability checks use one registry.
- A language adapter cannot bypass source, deadline, VFS, workspace, or SQLite safety controls.
- Raw symbol references are structured and validated rather than encoded in implementation strings.
- Reference resolution can represent resolved, ambiguous, unresolved, and unsupported outcomes without guessing.
- Persisted indexes detect language/analysis-revision incompatibility.
- A minimal TypeScript or JavaScript adapter can be added without modifying unrelated language branches across parsing, skeletons, indexing, tracing, patching, and persistence.
- Common adapter contract tests and JS/TS-specific fixtures pass.

## 22. Follow-up Decisions

These questions should be decided when implementation begins, not implicitly inside the first adapter patch:

1. Should workspace language overrides be a core API only at first, or exposed through a checked-in configuration file?
2. When should `language` become an additive public field on symbol/read/search responses?
3. Does `analysis_revision` live only in the file-state table, or redundantly on symbols for faster validation and diagnostics?
4. Should JavaScript and TypeScript be separate `LanguageId` values with an explicit bridge, or one shared JS/TS family plus syntax-mode metadata?
5. Which package-manager/source-root behaviors are justified for each language before they are allowed to influence file dependency resolution?
6. Which language capabilities, if any, should be visible in `tools/list` or a dedicated inspection tool?

Until these decisions are made, the implementation should prefer internal, conservative defaults and preserve current public contracts.
