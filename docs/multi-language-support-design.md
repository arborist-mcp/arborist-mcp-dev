# Multi-Language Support Design

**Status:** Phases 1-4 implemented; Phase 5 Rust and Go skeleton/index/dependency/trace slices, Java query/semantic-skeleton/declaration-indexing/import-dependency/direct-trace/instance-receiver-trace/var-constructor-receiver-trace/constructor-receiver-trace/interface-receiver-trace/member-chain-receiver-trace/factory-inferred-receiver-trace/constructor-chain-receiver-trace/interface-chain-receiver-trace/class-receiver-interface-trace/branching-interface-chain-trace/generic-receiver-trace/method-hop-receiver-trace/this-chain-receiver-trace/super-chain-receiver-trace/qualified-initializer-receiver-trace/nested-import-receiver-trace/direct-static-imported-field-chain-trace/direct-type-qualified-static-root-chain-trace/generic-static-root-member-chain-trace/direct-factory-root-member-chain-trace/parenthesized-member-chain-receiver-trace/parenthesized-var-initializer-receiver-trace/array-access-receiver-trace/factory-returned-array-receiver-trace/var-element-access-receiver-trace/var-qualified-element-access-receiver-trace/var-factory-call-element-access-receiver-trace/var-super-static-field-element-access-receiver-trace/var-factory-returned-array-receiver-trace/var-qualified-factory-array-receiver-trace/local-collection-range-element-receiver-trace/collection-parameter-range-element-receiver-trace/map-range-key-receiver-trace/nested-parenthesized-range-expression-trace slices, C# parsing/raw-query/semantic-skeleton/declaration-indexing/local/root-and-exact-namespace-alias-static-and-namespace-import/global-alias-static-namespace-import-and-base-constructor/base-method/local-and-global-base-alias-import/unshadowed-qualified-base/cross-file-same-namespace-and-nested-source/instance-receiver-trace/var-constructor-receiver-trace/factory-inferred-receiver-trace/bound-receiver-factory-receiver-trace/chained-factory-receiver-trace/base-factory-receiver-trace/base-factory-chain-receiver-trace/field-access-initializer-receiver-trace/base-initializer-receiver-trace/static-field-initializer-receiver-trace/static-imported-field-initializer-receiver-trace/inherited-field-initializer-receiver-trace/generic-inherited-field-initializer-receiver-trace/outer-generic-parameter-inherited-field-initializer-var-receiver-trace/static-factory-field-receiver-trace/factory-method-call-field-receiver-trace/alias-static-root-receiver-trace/cross-namespace-static-imported-and-inherited-receiver-trace/cross-namespace-base-rooted-receiver-trace/cross-namespace-constructed-and-factory-receiver-trace/cross-namespace-bound-receiver-trace/direct-static-field-member-chain-multidimensional-element-access-call-trace/direct-static-imported-member-chain-multidimensional-element-access-call-trace/direct-static-factory-member-chain-multidimensional-element-access-call-trace/direct-bare-factory-member-chain-multidimensional-element-access-call-trace/direct-static-imported-factory-member-chain-multidimensional-element-access-call-trace/direct-static-field-multidimensional-var-initializer-trace/direct-bare-static-field-multidimensional-var-initializer-trace/direct-static-imported-field-multidimensional-var-initializer-trace/direct-static-factory-multidimensional-var-initializer-trace/direct-static-imported-factory-multidimensional-var-initializer-trace/direct-static-field-multidimensional-foreach-element-trace/direct-bare-static-field-multidimensional-foreach-element-trace/direct-static-imported-field-multidimensional-foreach-element-trace/direct-static-imported-factory-multidimensional-foreach-element-trace/bare-inherited-field-multidimensional-foreach-element-trace/bare-inherited-field-multidimensional-var-initializer-trace/generic-inherited-field-multidimensional-foreach-element-trace/generic-inherited-field-multidimensional-var-initializer-trace/constructed-base-inherited-field-multidimensional-foreach-element-trace/constructed-base-inherited-field-multidimensional-var-initializer-trace/inherited-field-shadows-static-import-multidimensional-foreach-element-trace/inherited-field-shadows-static-import-multidimensional-var-initializer-trace/outer-generic-parameter-inherited-field-multidimensional-foreach-element-trace/outer-generic-parameter-inherited-field-multidimensional-var-initializer-trace/cross-file-namespace-inherited-field-multidimensional-foreach-element-trace/cross-file-namespace-inherited-field-multidimensional-var-initializer-trace/outer-generic-parameter-constructed-base-inherited-field-multidimensional-foreach-element-trace/outer-generic-parameter-constructed-base-inherited-field-multidimensional-var-initializer-trace/cross-namespace-static-field-member-chain-call-trace/cross-namespace-static-imported-member-chain-call-trace/cross-namespace-static-factory-member-chain-call-trace/cross-namespace-nested-type-static-root-trace/direct-bare-factory-member-chain-call-trace/bare-factory-root-chain-var-initializer-receiver-trace/this-rooted-factory-chain-var-initializer-receiver-trace/parenthesized-factory-chain-var-initializer-receiver-trace/bound-receiver-factory-chain-var-initializer-receiver-trace/bound-receiver-conditional-member-element-access-receiver-trace/var-element-access-marker-bound-collection-local-receiver-trace/bare-factory-root-conditional-member-element-access-receiver-trace/static-qualified-factory-conditional-member-element-access-receiver-trace/constructed-receiver-conditional-member-element-access-receiver-trace/file-scoped-using-cross-namespace-factory-receiver-trace/file-scoped-using-conditional-member-element-access-receiver-trace/file-scoped-using-static-factory-receiver-trace/parenthesized-conditional-member-element-access-receiver-trace/static-field-rooted-conditional-member-element-access-receiver-trace/alias-static-factory-conditional-member-element-access-receiver-trace/global-qualified-conditional-member-element-access-receiver-trace/nested-namespace-dotted-type-conditional-member-element-access-receiver-trace/namespace-imported-dotted-type-conditional-member-element-access-receiver-trace/alias-dotted-type-conditional-member-element-access-receiver-trace/static-imported-field-conditional-member-element-access-receiver-trace/bare-base-field-conditional-member-element-access-receiver-trace/inherited-shadows-static-import-conditional-member-element-access-receiver-trace/inherited-root-factory-element-access-receiver-trace/inherited-shadow-static-import-initializer-chain-receiver-trace/record-positional-property-conditional-member-element-access-receiver-trace/instance-receiver-static-member-element-access-receiver-trace/interface-inherited-conditional-member-element-access-receiver-trace/generic-member-type-conditional-member-element-access-receiver-trace/generic-inheritance-substitution-conditional-member-element-access-receiver-trace/generic-interface-extends-conditional-member-element-access-receiver-trace/generic-interface-extends-method-call-hop-receiver-trace/generic-receiver-nullable-multi-parameter-var-initializer-receiver-trace/cross-namespace-generic-interface-extends-method-call-hop-receiver-trace/cross-namespace-generic-inheritance-receiver-trace/cross-namespace-generic-imported-caller-receiver-trace/generic-nested-type-receiver-trace/cross-namespace-imported-nested-type-receiver-trace/nested-generic-base-type-receiver-trace/nested-generic-outer-parameter-receiver-trace/nested-generic-base-type-outer-parameter-receiver-trace/marker-bound-receiver-outer-parameter-trace/multi-level-nested-generic-receiver-outer-parameter-trace/factory-chain-and-array-element-nested-receiver-outer-parameter-trace/factory-array-and-element-access-factory-receiver-outer-parameter-trace/jagged-factory-array-receiver-outer-parameter-trace/var-held-array-element-receiver-outer-parameter-trace/chained-element-access-var-receiver-outer-parameter-trace/foreach-factory-array-element-receiver-outer-parameter-trace/chain-initializer-var-array-element-receiver-outer-parameter-trace/direct-constructed-generic-receiver-outer-parameter-trace/var-constructed-receiver-factory-initializer-outer-parameter-trace/var-constructed-receiver-factory-array-element-outer-parameter-trace/parenthesized-constructed-receiver-factory-outer-parameter-trace/multi-type-argument-constructed-receiver-factory-outer-parameter-trace/object-initializer-constructed-receiver-factory-outer-parameter-trace/direct-constructed-receiver-factory-array-element-outer-parameter-trace/generic-method-call-type-argument-return-substitution-trace/generic-method-call-type-argument-return-substitution-bound-receiver-trace/constructed-generic-receiver-property-chain-var-initializer-array-element-access-receiver-trace/nested-constructed-generic-receiver-property-chain-var-array-element-access-receiver-trace/nested-constructed-generic-receiver-member-chain-var-array-element-access-receiver-trace/nested-constructed-generic-receiver-method-call-chain-var-array-element-access-receiver-trace/nested-constructed-generic-receiver-method-call-chain-var-factory-array-element-access-receiver-trace/nested-constructed-generic-receiver-method-call-chain-element-access-initializer-receiver-trace/qualified-global-constructed-generic-receiver-outer-parameter-element-access-trace/derived-constructed-generic-base-outer-parameter-element-access-trace/constructed-static-receiver-generic-substitution-element-access-receiver-trace/constructed-static-receiver-factory-array-element-access-receiver-trace/constructed-static-receiver-static-member-element-access-receiver-trace/constructed-static-receiver-static-member-multidimensional-element-access-receiver-trace/constructed-static-receiver-static-member-multidimensional-var-initializer-receiver-trace/constructed-static-receiver-static-member-var-initializer-receiver-trace/constructed-static-receiver-static-member-multidimensional-foreach-element-receiver-trace/constructed-static-receiver-static-member-foreach-element-receiver-trace/constructed-static-receiver-static-member-multidimensional-chain-foreach-element-receiver-trace/constructed-static-receiver-static-member-chain-foreach-element-receiver-trace/constructed-static-receiver-static-member-multidimensional-array-var-initializer-receiver-trace/constructed-static-receiver-static-member-array-var-initializer-receiver-trace/static-member-bare-unbound-array-var-initializer-receiver-trace/static-member-bare-unbound-array-multidimensional-var-initializer-receiver-trace/direct-static-member-element-access-receiver-trace/direct-static-member-multidimensional-element-access-receiver-trace/generic-static-member-element-access-receiver-trace/generic-static-member-multidimensional-element-access-receiver-trace/constructed-generic-static-member-element-access-receiver-trace/constructed-generic-static-member-multidimensional-element-access-receiver-trace/inherited-static-member-element-access-receiver-trace/inherited-static-member-multidimensional-element-access-receiver-trace/constructed-base-inherited-static-member-element-access-receiver-trace/constructed-base-inherited-static-member-multidimensional-element-access-receiver-trace/static-imported-inherited-static-member-element-access-receiver-trace/static-imported-inherited-static-member-multidimensional-element-access-receiver-trace/static-imported-constructed-base-static-member-element-access-receiver-trace/static-imported-constructed-base-static-member-multidimensional-element-access-receiver-trace/parenthesized-member-chain-receiver-trace/parenthesized-var-initializer-receiver-trace/cross-namespace-factory-inferred-receiver-trace/nullable-reference-receiver-trace/base-ancestor-field-hop-receiver-trace/this-ancestor-field-hop-receiver-trace/bound-receiver-ancestor-field-hop-trace/constructed-receiver-ancestor-field-hop-trace/factory-receiver-ancestor-field-hop-trace/static-root-ancestor-field-hop-trace/inherited-root-trailing-hop-trace/cross-namespace-ancestor-field-hop-trace/generic-receiver-member-chain-trace/generic-static-root-member-chain-trace/interface-chain-member-hop-trace/cross-namespace-generic-interface-member-hop-trace/struct-receiver-member-chain-trace/constructor-receiver-trace/dotted-declared-type-receiver-trace/interface-receiver-trace/interface-chain-receiver-trace/struct-receiver-trace/member-chain-receiver-trace/this-chain-receiver-trace/method-call-hop-receiver-trace/constructor-chain-receiver-trace/base-chain-receiver-trace/array-access-receiver-trace/factory-returned-array-receiver-trace/var-element-access-receiver-trace/var-qualified-element-access-receiver-trace/var-factory-call-element-access-receiver-trace/var-super-static-field-element-access-receiver-trace/var-factory-returned-array-receiver-trace/var-qualified-factory-array-receiver-trace/multi-dimensional-array-element-access-receiver-trace/parenthesized-element-access-receiver-trace/jagged-array-element-access-receiver-trace/jagged-array-direct-element-access-receiver-trace/jagged-array-factory-returned-element-access-receiver-trace/foreach-var-element-type-receiver-trace/foreach-factory-returned-array-element-receiver-trace/foreach-chain-element-receiver-trace/foreach-var-local-collection-element-receiver-trace/foreach-parenthesized-collection-element-receiver-trace/foreach-base-rooted-collection-element-receiver-trace/await-foreach-fail-closed-trace/foreach-factory-chain-and-multi-dimension-collection-element-receiver-trace/base-rooted-element-access-receiver-trace/member-chain-method-call-hop-element-access-receiver-trace/constructed-receiver-factory-element-access-var-local-receiver-trace/constructed-receiver-factory-argument-element-access-var-local-receiver-trace/parenthesized-constructed-receiver-factory-element-access-var-local-receiver-trace/conditional-access-method-call-receiver-trace/conditional-access-var-initializer-receiver-trace/conditional-access-element-access-var-initializer-receiver-trace/conditional-access-element-access-method-call-receiver-trace trace slices, and Kotlin parsing/raw-query/semantic-skeleton/declaration-indexing/import-dependency/direct-trace/extension-function-trace/property-chain-trace/object-receiver-trace/constructor-trace/object-chain-trace/constructor-inferred-property-chain-trace/interface-receiver-trace/typealias-receiver-trace/companion-object-trace/companion-chain-trace/named-companion-trace/factory-inferred-property-chain-trace/factory-inferred-binding-trace/factory-inferred-nested-receiver-trace/nested-receiver-trace/nested-object-trace/nested-companion-trace/dotted-alias-trace/nested-parameter-receiver-trace/constructor-chain-trace/array-access-receiver-trace/factory-returned-array-receiver-trace/var-element-access-receiver-trace/var-qualified-element-access-receiver-trace/var-factory-call-element-access-receiver-trace/var-factory-returned-array-receiver-trace/var-qualified-factory-array-receiver-trace/parenthesized-var-initializer-receiver-trace/parenthesized-member-chain-receiver-trace/this-chain-receiver-trace/super-chain-receiver-trace/direct-factory-root-member-chain-trace/method-call-hop-receiver-trace/generic-receiver-trace/nullable-reference-receiver-trace/interface-method-hop-receiver-trace/interface-chain-receiver-trace/class-receiver-interface-trace/generic-interface-chain-receiver-trace/superclass-chain-receiver-trace/branching-interface-chain-trace/class-receiver-diamond-interface-chain-trace/generic-class-receiver-hierarchy-trace/nullable-class-receiver-hierarchy-trace/cross-file-imported-generic-class-receiver-trace/class-receiver-diamond-interface-chain-hop-trace/interface-receiver-diamond-chain-hop-trace/this-and-super-rooted-hierarchy-trace/cross-file-var-super-static-field-element-access-receiver-trace/cross-file-nullable-this-and-super-rooted-member-trace/cross-file-branching-interface-chain-receiver-trace/cross-file-interface-property-chain-receiver-trace/cross-file-named-companion-receiver-trace/cross-file-explicit-companion-chain-receiver-trace/cross-file-factory-root-member-chain-receiver-trace/cross-file-typealias-property-chain-receiver-trace/cross-file-var-parenthesized-initializer-receiver-trace/cross-file-deep-constructor-chain-receiver-trace/cross-file-enum-companion-receiver-trace/cross-file-generic-superclass-rooted-receiver-trace/cross-file-dotted-alias-receiver-trace/cross-file-constructor-inferred-property-chain-receiver-trace/cross-file-nullable-reference-receiver-trace/cross-file-deep-object-chain-receiver-trace/cross-file-var-companion-object-element-access-receiver-trace/cross-file-var-super-static-field-element-access-receiver-trace/cross-file-var-factory-returned-array-receiver-trace/cross-file-var-factory-call-element-access-receiver-trace/cross-file-var-qualified-element-access-receiver-trace/cross-file-var-element-access-receiver-trace/cross-file-parenthesized-receiver-member-chain-trace/cross-file-qualified-factory-returned-array-receiver-trace/cross-file-factory-returned-array-receiver-trace/cross-file-companion-property-chain-trace/cross-file-array-access-receiver-trace/cross-file-constructor-rooted-superclass-chain-trace/cross-file-nullable-class-receiver-hierarchy-trace/cross-file-aliased-type-import-class-receiver-hierarchy-trace/cross-file-typealias-receiver-trace/cross-file-generic-interface-chain-hop-trace/cross-file-diamond-interface-chain-hop-trace/cross-file-this-and-super-rooted-class-receiver-hierarchy-trace/cross-file-class-receiver-hierarchy-hop-trace/class-hierarchy-hop-receiver-trace/qualified-factory-array-element-access-receiver-trace/member-chain-after-factory-element-access-receiver-trace/cross-file-qualified-factory-element-access-member-chain-receiver-trace/var-element-access-inferred-member-chain-receiver-trace/array-property-element-access-hop-receiver-trace/companion-chain-element-access-method-hop-receiver-trace/var-companion-chain-element-access-inferred-member-chain-receiver-trace/this-super-rooted-factory-element-access-member-chain-receiver-trace/var-this-rooted-element-access-inferred-member-chain-receiver-trace/this-super-rooted-factory-call-inferred-member-chain-receiver-trace/var-this-super-rooted-factory-element-access-base-inferred-member-chain-receiver-trace/nullable-factory-force-unwrap-receiver-trace/nullable-declared-array-receiver-trace/anonymous-companion-inline-element-access-receiver-trace/bare-companion-root-receiver-trace/implicit-companion-property-receiver-trace/implicit-companion-function-receiver-trace/implicit-member-function-receiver-trace/implicit-inherited-property-receiver-trace/implicit-inherited-member-function-receiver-trace/property-chain-initializer-receiver-trace/property-chain-initializer-array-element-access-receiver-trace/bare-property-initializer-receiver-trace/property-chain-initializer-method-call-hop-receiver-trace/cross-file-property-chain-initializer-method-call-hop-receiver-trace/property-chain-initializer-constructor-call-receiver-trace/property-chain-initializer-nullable-generic-root-receiver-trace/property-chain-initializer-object-companion-root-receiver-trace/property-chain-initializer-local-binding-root-receiver-trace/property-chain-initializer-top-level-property-root-receiver-trace/property-chain-initializer-top-level-array-property-root-receiver-trace/property-chain-initializer-bound-array-root-receiver-trace/property-chain-initializer-factory-array-root-receiver-trace/property-chain-initializer-element-access-hop-receiver-trace/property-chain-initializer-top-level-array-element-access-hop-root-receiver-trace/property-chain-initializer-factory-element-access-hop-receiver-trace/property-chain-initializer-method-call-element-access-base-receiver-trace/property-chain-initializer-dotted-factory-call-binding-receiver-trace/property-chain-initializer-bound-array-element-access-root-receiver-trace/property-chain-initializer-chained-factory-call-binding-receiver-trace/property-chain-initializer-nested-object-factory-call-binding-receiver-trace/property-chain-initializer-constructor-call-factory-binding-receiver-trace/property-chain-initializer-if-when-expression-binding-receiver-trace/property-chain-initializer-scope-function-binding-receiver-trace/property-chain-initializer-scope-function-this-rooted-binding-receiver-trace/property-chain-initializer-scope-function-explicit-parameter-binding-receiver-trace/property-chain-initializer-scope-function-chain-receiver-binding-receiver-trace/property-chain-initializer-scope-function-branch-binding-receiver-trace/property-chain-initializer-scope-function-nullable-receiver-binding-receiver-trace/property-chain-initializer-scope-function-nested-chain-receiver-binding-receiver-trace/property-chain-initializer-elvis-expression-binding-receiver-trace/property-chain-initializer-multi-statement-lambda-body-binding-receiver-trace/property-chain-initializer-nullable-nested-scope-chain-binding-receiver-trace/property-chain-initializer-multi-statement-nullable-nested-scope-chain-binding-receiver-trace/property-chain-initializer-multi-statement-branch-elvis-binding-receiver-trace/property-chain-initializer-scope-branch-result-binding-receiver-trace/property-chain-initializer-scope-branch-nested-scope-call-binding-receiver-trace/property-chain-initializer-scope-branch-enclosing-member-nested-scope-call-binding-receiver-trace/property-chain-initializer-nested-scope-call-single-result-binding-receiver-trace/property-chain-initializer-scope-function-enclosing-member-direct-call-binding-receiver-trace/property-chain-initializer-scope-function-scope-call-local-chain-binding-receiver-trace/cross-file-scope-function-scope-call-local-chain-binding-receiver-trace/scope-function-lambda-branch-local-binding-receiver-trace/scope-function-lambda-branch-local-chain-binding-receiver-trace/scope-function-lambda-branch-local-branch-result-binding-receiver-trace/scope-function-lambda-branch-local-scope-call-binding-receiver-trace/scope-function-lambda-branch-local-scope-call-arm-binding-receiver-trace/scope-function-lambda-branch-local-scope-call-local-binding-receiver-trace/scope-function-lambda-branch-local-scope-call-chain-binding-receiver-trace/scope-function-lambda-branch-local-scope-call-branch-body-binding-receiver-trace/scope-function-lambda-branch-local-apply-navigation-binding-receiver-trace/scope-function-lambda-branch-local-scope-call-navigation-binding-receiver-trace/cross-file-scope-function-lambda-branch-local-scope-call-navigation-binding-receiver-trace/scope-function-lambda-branch-local-nested-scope-call-navigation-binding-receiver-trace/cross-file-scope-function-lambda-branch-local-nested-scope-call-navigation-binding-receiver-trace/scope-function-lambda-branch-local-when-elvis-scope-call-navigation-binding-receiver-trace/scope-function-lambda-branch-local-when-elvis-nested-scope-call-navigation-binding-receiver-trace/scope-function-lambda-branch-local-parenthesized-receiver-scope-call-navigation-binding-receiver-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-field-multidimensional-foreach-element-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-static-field-multidimensional-var-initializer-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-static-field-multidimensional-foreach-element-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-property-multidimensional-var-initializer-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-property-multidimensional-foreach-element-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-qualified-static-field-multidimensional-var-initializer-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-field-multidimensional-var-initializer-tracebare-inherited-field-member-chain-call-trace/bare-inherited-field-member-chain-multidimensional-element-access-call-trace/outer-generic-parameter-constructed-base-inherited-field-multidimensional-element-access-call-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-field-multidimensional-element-access-call-trace/outer-generic-parameter-constructed-nested-generic-base-inherited-static-factory-var-initializer-and-direct-call-trace/type-qualified-outer-generic-parameter-constructed-nested-generic-base-inherited-static-factory-var-initializer-and-direct-call-trace/bare-generic-static-factory-direct-member-chain-trace/type-alias-constructed-generic-base-inherited-static-factory-and-direct-call-trace/type-alias-constructed-generic-base-inherited-static-field-member-chain-call-trace/global-type-alias-constructed-generic-base-inherited-static-field-property-and-multidimensional-member-chain-trace/direct-constructed-generic-receiver-inherited-static-factory-and-field-chain-trace/cross-namespace-direct-constructed-generic-inherited-static-factory-declaring-base-caller-edge-trace/cross-namespace-multi-hop-constructed-generic-inherited-static-factory-declaring-base-caller-edge-trace/cross-namespace-alias-outer-constructed-generic-nested-inherited-static-factory-declaring-base-caller-edge-trace/ slices implemented. Rust also contributes structural patch targeting and syntax-level patch validation for functions, methods, and declaration items by semantic path or source position, with language-specific patch binding validation deferred. Kotlin also contributes structural patch targeting and syntax-level patch validation for classes, interfaces, enums, named objects, functions, simple properties, companion objects, and type aliases by semantic path or source position, with language-specific patch binding validation deferred. C# also contributes structural patch targeting and syntax-level patch validation for classes, structs, interfaces, enums, records, methods, and constructors by semantic path or source position, with language-specific patch binding validation deferred. Go also contributes structural patch targeting and syntax-level patch validation for functions, methods, and type specifications and aliases by semantic path or source position, with language-specific patch binding validation deferred. Java also contributes structural patch targeting and syntax-level patch validation for classes, interfaces, enums, records, annotation types, methods, constructors, and nested types by semantic path or source position, with language-specific patch binding validation deferred. JavaScript/TypeScript also contribute conservative default-import call resolution to named module default exports and namespace-import member-call resolution to named exports within the bound module, plus namespace re-export member-call resolution and star re-export chain resolution for named imports, with namespace member calls following the bound module's named and star re-export chains and default exports, for live and persisted symbol indexes, with direct namespace-object calls resolving CommonJS callable exports and language-specific patch binding validation: identifier references inside a patched symbol resolve to visible function, arrow, and method parameters (including destructured, default, rest, and TypeScript parameter-property bindings), local `const`/`let`/`var` declarators including destructured declarations, `for`-of/`for`-in and C-style `for` loop variables, catch parameters, nested callable parameters, same-file top-level declarations, or explicit named, default, and namespace imports, while member names, object keys, labels, JSX tag and attribute names, type spellings, and standard host/global names are ignored, and unknown bare identifiers fail closed.
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
| JavaScript/TypeScript | Query, skeleton, symbols, conservative direct-call trace including static local named imports, named re-export chains, default imports that name a module's default export, namespace member calls that follow the bound module's named and star re-export chains and default exports, star re-export chains, and static local module dependency refresh; language-specific patch binding validation follows in a later adapter slice. | Dynamic imports, bundler aliases, framework injection, rich type-driven dispatch. |
| Rust | Query, skeleton, symbols, module dependencies, patch targeting; selected direct-call trace | Macro expansion, trait-method dispatch, complete Cargo feature resolution. |
| Go | Query, skeleton, symbols, package imports, direct-call trace including bounded same-package interface method specifications, direct interface embedding, and unique same-file and same-package factory-result method calls, patch targeting | General interface dispatch, interface embedding/implementation dispatch, and build-tag-aware workspace modes. |
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
- Kotlin and Java cross-language edges only after an explicit JVM interop model exists.

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
| JavaScript/TypeScript | static `import`, re-export-from, and a conservative `require` subset (`const ns = require(...)`, `const { helper } = require(...)`). |
| Rust | `mod` declarations and optionally manifest-informed workspace roots. |
| Go | package import paths and workspace module boundaries. |
| Java/Kotlin | package/import facts and configured source roots. |

The existing C include-dependent refresh behavior now runs through a generic local-file-dependency reverse index while retaining its tested implementation internally. JavaScript/TypeScript contributes static relative imports, re-exports, and direct literal `require` calls that resolve to local JS/TS-family files. Direct calls through local named imports (including aliases), default imports that name a module's default export, namespace-import member calls, and namespace re-export member calls are resolved only to matching exported symbols in the imported module or the modules its named and star re-export chains reach; namespace default-member calls (`ns.default(...)`) resolve to the bound module's named default export. Static named `export { name as alias } from "./module"`, namespace re-export (`export * as name from "./module"`) chains, and star re-export chains are followed recursively with cycle detection; missing, non-local, non-exported, cyclic, and ambiguous bindings fail closed instead of falling back to same-named workspace symbols. Direct namespace-object calls (`ns(...)`) resolve only CommonJS callable exports (`module.exports = <function>`); dynamic imports, package specifiers, escaped literals, and bundler aliases remain unresolved. Languages are added one at a time; an adapter without `FILE_DEPENDENCIES` does not contribute reverse-refresh paths. Go's bounded module-import dependency adapter now filters imported package directories to parseable production `.go` sources with a package declaration, rejects syntax-invalid candidates, and fails closed when a directory exposes multiple production package names. Java's bounded source-level dependency adapter now also resolves unique local package wildcard imports (`import pkg.*`) to parseable `.java` files declaring that package, and static wildcard imports (`import static pkg.Type.*`) to the declaring type's source file; ambiguous package roots and unresolved candidates remain fail-closed.

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

Python file dependencies now use a bounded source-level adapter for static `import` and `from ... import ...` statements. It resolves only local `.py`/`.pyi` modules and package `__init__` files; dynamic imports, external packages, import hooks, and unresolved names fail closed. `from ... import ...` statements retain dependencies on both the imported package/module and any resolved submodule, so changes to `__init__.py` invalidate importers; wildcard imports use the same package-level dependency.

### Phase 4: JavaScript and TypeScript

Add `tree-sitter-javascript` and `tree-sitter-typescript` using the repository's existing dependency and lockfile conventions. Implement the first external adapter with a conservative initial capability set.

The first delivered slices register JavaScript (`.js`, `.jsx`, `.mjs`, `.cjs`), TypeScript (`.ts`, `.mts`, `.cts`), and TSX (`.tsx`) grammars for parsing, Tree-sitter queries, semantic skeletons, conservative direct-call symbol indexing/tracing, static local module dependency extraction, direct calls through static local named imports, direct named re-export chains, and structural patching. The first patch slice supports semantic and position targets with syntax-level validation; a further slice adds JavaScript/TypeScript patch binding validation: identifier references inside a patched symbol resolve to visible function, arrow, and method parameters (including destructured, default, rest, and TypeScript parameter-property bindings), local `const`/`let`/`var` declarators including destructured declarations, `for`-of/`for`-in and C-style `for` loop variables, catch parameters, nested callable parameters, same-file top-level function, class, interface, enum, type-alias, and variable declarations, or explicit named, default, and namespace imports, while member names, object keys, labels, JSX tag and attribute names, type spellings, and standard host/global names are ignored, and unknown bare identifiers fail closed. A later slice adds conservative default-import call resolution for named module default exports across live and persisted indexes, and another adds namespace-import member-call resolution to named exports within the bound module, and a further slice resolves star re-export chains for named imports with cycle detection and fail-closed ambiguity handling, and another resolves namespace re-export member calls through `export * as name from "./module"` bindings, and a later slice makes namespace member calls follow the bound module's named and star re-export chains with cycle detection and fail-closed ambiguity handling, and another resolves namespace default-member calls to the bound module's named default export. Direct namespace-object call resolution is limited to CommonJS callable exports and remains fail-closed for ESM-only and non-callable modules. CommonJS `require` bindings feed the same resolution machinery: `const ns = require("./module")` binds the module namespace and `const { helper } = require("./module")` binds named members, including destructured members with default values such as `const { helper = fallback } = require("./module")` or `const { helper: bound = fallback } = require("./module")` that still bind the module member (the default only applies at runtime), so namespace member calls, namespace-object calls, and destructured member calls resolve with the same fail-closed behavior, while dynamic require arguments, array patterns, rest elements, nested patterns, and missing local modules fail closed. CommonJS `module.exports = { ... }` object-literal exports expose their shorthand, same-named, aliased identifier, and named function/generator/class expression entries as namespace members, where aliased pairs and differently-named expression values resolve through the exported-name to local-name mapping (so `module.exports = { helper: function helper() {} }` names the declared `helper` symbol and a final `module.exports = { default: function app() {} }` entry names the interop default member), while method definitions, computed and string keys, anonymous function/class values, non-symbol values, and non-object exports fail closed. CommonJS `exports.name = ...` and `module.exports.name = ...` member assignments expose the assigned local symbol as a namespace member through the same exported-name to local-name mapping, where identifier and named function/generator/class values resolve and anonymous, computed, string-key, and non-symbol values fail closed. TypeScript `import name = require("./module")` bindings bind the module namespace through the same machinery, so namespace member calls and namespace-object calls resolve with the same fail-closed behavior, and the module specifier feeds static local dependency refresh. Wholesale CommonJS `module.exports = require("./module")` re-exports alias the module namespace to the target module's export object, so namespace member calls, destructured member calls, and namespace-object calls resolve within the terminal module of the re-export chain, while ambiguous, cyclic, dynamic, or unresolvable chains fail closed. Default imports and namespace `default` members also resolve through CommonJS interop exports: `exports.default = ...` / `module.exports.default = ...` member assignments name the default member, a `module.exports = <callable>` export is the default import target when no ESM or member default exists, and a `module.exports = <value>` replacement shadows member assignments, while ambiguous, anonymous, or conflicting defaults fail closed. A `module.exports = <value>` replacement also shadows the `exports` alias and any export object it replaced, so `exports.*` member assignments (before or after the replacement) and object-literal or `module.exports.*` member exports that precede the final replacement fail closed, while `module.exports.*` member assignments after the final replacement still expose their assigned local symbol as a namespace member. Inline CommonJS `require("./module").member(...)` member calls and inline bare `require("./module")(...)` namespace-object calls resolve through the same namespace-member and CommonJS-callable machinery, with the module specifier resolved against the referencing file so overlay/override paths apply, while missing modules, non-exported members, and non-callable or ESM-only modules fail closed instead of falling back to same-named workspace symbols. CommonJS export members whose assigned value aliases another module also resolve through the same machinery: `exports.name = require("./module")`, `module.exports.name = require("./module")`, and object-literal entries `module.exports = { name: require("./module") }` expose a namespace member that resolves within the aliased module, where whole-module aliases resolve to the target's single CommonJS callable export and member aliases such as `require("./module").member` resolve like any namespace member of the target, while ambiguous, missing, dynamic, cyclic, or non-callable aliases fail closed. Destructured `const { helper } = require("./bridge")` bindings and named `import { helper } from "./bridge"` bindings resolve through such module-valued export members as well, following transitive whole-module and member alias chains to the terminal module, with the same fail-closed behavior for ambiguous, missing, non-callable, and cyclic aliases. Constructor calls through `new` also feed the same resolution machinery: `new Foo()`, `new Imported()`, and `new ns.Foo()` resolve to the constructed class or function declaration for local symbols, named imports, default imports, and namespace members, and a constructor on a module namespace bound through `const Counter = require("./module")` or TypeScript `import Counter = require("./module")` resolves to the bound module's single CommonJS constructible export, where classes count as constructible but not directly callable so plain namespace-object calls stay limited to callable exports, while unknown, missing, ambiguous, dynamic, cyclic, or non-constructible exports fail closed. Object-literal spread re-exports (`module.exports = { ...require("./module") }`) spread the target module's named exports into the export object, so namespace member calls, destructured member calls, and inline require member calls resolve within the spread target like star re-exports, following the target's wholesale re-export chains, member aliases, direct exports, and further spreads, while explicit object entries shadow spread-provided members and missing targets, multiple spread targets providing one member, and cyclic spreads fail closed. Default imports and namespace `default` members also resolve through a CommonJS export object's default member: a final `module.exports = { default: local }` entry names a local symbol, a final `module.exports = { default: require("./module") }` entry aliases the target module's default through the module-valued member machinery (the target's single CommonJS callable export, member default, or further export-object default), and a final `module.exports = { ...require("./module") }` spread forwards the target's default when it is resolvable, while the module's own ESM default export or `exports.default` / `module.exports.default` member still shadows these, and conflicting, ambiguous, anonymous, missing, or cyclic export-object defaults fail closed. TypeScript `export = <value>` export assignments mirror their CommonJS `module.exports = <value>` counterparts through the same machinery: `export = <callable>` names the module's callable/constructible export for `import name = require("./module")` namespace-object consumers, and `export = require("./module")` wholesale re-exports alias the module namespace to the target module's export object so namespace member calls, destructured member calls, and namespace-object calls resolve within the terminal module of the chain, while non-require values, dynamic require arguments, ambiguous, missing, cyclic, or unresolvable chains fail closed. TypeScript `export = { ... }` object-literal exports mirror the CommonJS `module.exports = { ... }` machinery: shorthand, aliased, and named function/generator/class expression entries expose namespace members, `export = { name: require("./module") }` entries alias module-valued members, `export = { ...require("./module") }` spreads re-export the target's named members and default, and a final `export = { default: local }` entry names the interop default member, while method definitions, computed and string keys, non-symbol values, non-require spread arguments, and non-object final replacements fail closed. Named default re-exports (`export { default } from "./module"`) follow the terminal module's full CommonJS interop default like a default import: the re-export names the terminal module's ESM default export or `exports.default` / `module.exports.default` member, a CommonJS callable `module.exports = ...` export is the default under interop semantics, and an export-object `default` entry (including `export = { default: local }` and final spread forwarding) resolves through the namespace-member fallback, while anonymous or absent defaults fail closed. Constructor references through default imports also accept a single CommonJS constructible (class) export (`module.exports = <class>` or TypeScript `export = <class>`) as the default target, directly and through wholesale or named default re-export chains, while plain calls on such defaults stay limited to callable exports and fail closed for classes, and anonymous or absent defaults fail closed.

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

Add one language per reviewable sequence. Each language starts with discovery and indexing, then gains dependency and trace capability only after dedicated fixtures establish safe behavior. The current Rust slice supports `.rs` parsing, raw Tree-sitter queries, semantic skeletons, declaration indexing for named modules, functions, structs, enums, traits, type aliases, constants, statics, trait signatures, and inherent-`impl` methods, plus local module dependency refresh for unambiguous out-of-line `mod` declarations. It traces unshadowed bare direct calls to functions in the same source-file module, qualified direct calls to functions in inline modules in the same source file, and `module::function()` or `crate::module::function()` calls through an explicit chain of source-file-root out-of-line `mod` declarations. Each module in the chain must use one unambiguous default-layout file (`module.rs` or `module/mod.rs`), and the terminal file must contain one matching top-level function. It also traces unshadowed bare calls through exact `use crate::module::function;`, `use self::module::function;`, or `use super::...` bindings, including grouped `use` paths and explicit `as` aliases when their targets are reachable through the unique out-of-line parent/module chain. Crate-root imports from out-of-line children and repeated `super::` ancestor navigation are supported. Equivalent qualified `crate::...` and `super::...` calls from out-of-line children use the same conservative parent/module chain. A further slice follows unambiguous `pub use` re-exports so imported and qualified calls through an out-of-line module file or the crate root forward to the defining top-level function: exact re-exports such as `pub use crate::impl_mod::function;`, relative crate-root re-exports such as `pub use api::helper;`, grouped `pub use crate::impl_mod::{function, other};` paths, and `as` aliases such as `pub use crate::impl_mod::function as renamed;` all resolve through the same conservative parent/module chain, including nested re-export chains, while private `use`, ambiguous duplicate re-exports of the same local name, and cyclic re-export chains fail closed. A further slice makes bare `crate::name()` calls (from the crate root or out-of-line children) resolve to a matching crate-root top-level function or through a crate-root `pub use` re-export to the defining module, matching the already-supported `super::name()` parent-qualified calls. A further slice adds exact `use crate::api::function;` imports and `crate::api::function()` qualified calls whose target is declared inside an inline `mod api` at the crate root, resolving through the crate-root file's indexed inline-module semantic path with the same fail-closed rules for missing, ambiguous, or non-function targets, while `self::`-relative imports from inside nested inline modules remain capability-gated. A further slice adds type-qualified static call tracing: `Type::method()` calls whose leading path component is a CamelCase identifier that is not a declared out-of-line module resolve to a unique inherent `impl` method with that exact semantic path, while missing, ambiguous (multiple types defining the same method name), or non-method targets fail closed. A further slice adds module-binding `use crate::api;` calls into inline modules: from the crate root or an out-of-line child, `use crate::api;` (including `use crate::api as alias;` and grouped `use crate::{api};` bindings) lets `api::function()` or `alias::function()` calls resolve to a function declared inside the inline `mod api` at the crate root through the crate-root file's indexed inline-module semantic path, while missing modules, missing functions, or non-function targets fail closed. A further slice adds inline modules nested in out-of-line module files: `crate::api::inner::helper()` calls, `use crate::api::inner::helper;` imports, and module-binding `use crate::api;` plus `api::inner::helper()` calls from out-of-line children resolve through the out-of-line module chain into the inline `mod inner` declared in the terminal module file's indexed inline-module semantic path, while missing modules, missing functions, or non-function targets fail closed. A further slice adds instance-method call tracing for struct-literal receivers: `let c = Counter {}; c.increment();` and `Counter {}.increment();` calls resolve to a unique inherent `impl` method with that exact semantic path when the local binding's struct-literal type is unambiguous, while unknown receiver types, shadowed or ambiguous local bindings, non-struct initializers, and missing or non-method targets fail closed. Tuple-struct and unit-struct receivers such as `Counter(1).increment()`, `let c = Counter(1); c.increment();`, `Unit.run()`, or `let u = Unit; u.run();` also trace to a unique inherent `impl` method with that exact semantic path when the receiver name is a declared tuple or unit struct in the same source module and is not shadowed by a local binding, parameter, local function, module, or import, while unknown, imported, module-named, non-struct, or shadowed names fail closed. Module-qualified receiver type paths such as `api::Counter` also resolve when the leading path component is an inline `mod` declared in the same source file: typed-parameter receivers, constructor-call bindings such as `let c = api::Counter::new(); c.increment();`, tuple-struct and unit-struct construction such as `api::Counter(1).increment()` or `let u = api::Unit; u.run();`, and struct-literal receivers such as `api::Plain {}.step()` all trace to the unique inherent `impl` method with that exact semantic path, while out-of-line-module-qualified, imported, unknown, or shadowed path names fail closed. Crate-rooted receiver type paths such as `crate::api::Counter` also resolve when the path components after `crate::` form an inline `mod` declared in the same source file or name a struct declared directly at the crate root: typed-parameter receivers, constructor-call bindings such as `let c = crate::api::Counter::new(); c.increment();`, tuple-struct and unit-struct construction such as `crate::api::Counter(1).increment()` or `let u = crate::api::Unit; u.run();`, and struct-literal receivers such as `crate::api::Plain {}.step()` all trace to the unique inherent `impl` method with that exact semantic path, while out-of-line-module-qualified, imported, unknown, or shadowed path names fail closed. `self::`- and `super::`-rooted receiver type paths such as `let c = self::Root::new(); c.go();`, `let c = super::api::Counter::new(); c.increment();`, `super::api::Counter(1).increment()`, `super::api::Unit.run()`, or `super::api::Plain {}.step()` inside an inline `mod` also trace to the unique inherent `impl` method with that exact semantic path when the path components resolve relative to the caller's module or its parent chain, while `super` used above the crate root, out-of-line-module-qualified, imported, unknown, or shadowed path names fail closed. Type-qualified static calls through inline modules such as `api::Counter::new()` or `outer::inner::Unit::run()` also resolve to the unique inherent `impl` method with that exact semantic path when the leading path components form an inline `mod` declared in the same source file and the type name is a CamelCase identifier, while unknown modules, non-CamelCase type tails, out-of-line-module-qualified or imported leading names, and shadowed leading paths fail closed. Turbofish generic segments in type-qualified static calls such as `RootCounter::<u8>::new()` or `api::Counter::<u8>::new()` are ignored for resolution, so they trace to the same unique inherent `impl` method as the corresponding plain call, while unknown types or modules still fail closed. Trailing turbofish generic segments on called functions such as `api::helper::<u8>()`, `RootCounter::new::<u8>()`, `api::Counter::new::<u8>()`, or `local_helper::<u8>()` are ignored for resolution, so they trace to the same module function, local function, or inherent `impl` method as the corresponding plain call, while unknown targets still fail closed. Trailing turbofish generic segments on struct, tuple-struct, and unit-struct construction receivers such as `Counter::<u8> {}.step()`, `Counter::<u8>(1).increment()`, or `let u = Unit::<u8>; u.run();` are ignored for resolution, so they trace to the same unique inherent `impl` method as the corresponding plain construction, while unknown or shadowed construction targets still fail closed. `Self`-rooted receiver type paths such as `let c = Self::new(); c.increment();`, `Self(0).increment()`, `let c = Self; c.run();`, or `Self {}.step()` inside inherent `impl` methods also trace to the unique inherent `impl` method on the impl's own type through constructor-call bindings, tuple-struct and unit-struct construction, and struct-literal receivers, while `Self` used outside an impl, trait-impl methods, and nested functions fail closed. Trailing or middle turbofish generic segments on `Self`-rooted static calls and construction receivers such as `Self::<u8>::new()`, `Self::new::<u8>()`, `let c = Self::<u8>::new(); c.increment();`, `Self::<u8>(0).increment()`, `let u = Self::<u8>; u.run();`, or `Self::<u8> {}.step()` inside inherent `impl` methods are ignored for resolution, so they trace to the same inherent `impl` methods, while `Self` used outside an impl, trait-impl methods, and nested functions fail closed.  Typed-parameter receivers such as `fn caller(c: &Counter, d: Counter, e: &mut Counter) { c.increment(); }` also trace to a unique inherent `impl` method with that exact semantic path when the parameter type is a plain or referenced struct name, while primitive, generic, path-typed, unknown, shadowed, or ambiguous parameter types fail closed. Constructor-call bindings such as `let c = Counter::new(); c.increment();` also trace to a unique inherent `impl` method with that exact semantic path when the constructor is a same-file two-segment `Type::constructor` call whose type name is a plain, unimported, non-module struct name, while module-qualified, path-typed, imported, unknown, shadowed, or ambiguous constructor bindings fail closed. Turbofish constructor-call bindings such as `let c = Counter::<u8>::new(); c.increment();` or `let c = api::Counter::<u8>::new(); c.increment();` also trace to a unique inherent `impl` method with that exact semantic path when the surrounding type path resolves to a plain same-file struct or an inline-module-qualified struct, while path-typed, imported, unknown, shadowed, or ambiguous constructor bindings fail closed. `self.`-rooted calls inside inherent `impl` methods such as `fn twice(&self) { self.increment(); }` also trace to a unique inherent `impl` method with that exact semantic path on the impl's own type, while trait-impl methods and nested functions fail closed. `Self::`-rooted static calls such as `Self::new()` inside inherent `impl` methods also trace to a unique inherent or associated function on the same impl type, while missing or deeper unresolvable static targets and `Self` used outside an impl fail closed. Member-chain hops are no longer capability-gated: `outer.inner.increment();` calls resolve each intermediate field's declared plain struct type in the same source file before dispatching the final method, and chains rooted at typed-parameter receivers, struct-literal bindings, or `self.` inside inherent `impl` methods such as `self.middle.leaf.run();` follow the same field resolution, while unknown fields, unknown base receivers, generic or non-plain field types, missing methods, and primitive receivers fail closed. Member-chain hops through zero-argument method calls such as `outer.get_inner().increment();` or `make_root().middle().leaf().run();` also resolve each intermediate call's declared return type when it is a plain, unimported, same-file struct name returned by an inherent `impl` method with only `self` parameters or by a zero-argument top-level function, resolved in the callable's own module scope, before dispatching the final method, while argument-taking calls, generic, primitive, referenced, path-typed, or missing return types, unknown methods or functions, and unknown base receivers fail closed. Trailing turbofish generic segments on member-chain call hops such as `outer.get_inner::<u8>().increment();` or `make_root::<u8>().go();`, including their `let` bindings such as `let inner = outer.get_inner::<u8>(); inner.increment();`, are ignored for resolution, so they trace to the same intermediate call and final method as the corresponding plain hops, while unknown methods, functions, or return types still fail closed.  A further slice infers `let` bindings from the same zero-argument call hops: `let inner = outer.get_inner(); inner.increment();` or `let root = make_root(); root.middle().leaf().run();` bind each local to the callable's declared plain struct return type in the callable's own module scope and then dispatch the trailing member chain on it, while argument-taking calls, generic, primitive, referenced, path-typed, or missing return types, unknown methods or functions, shadowed function bindings, and unknown base receivers fail closed. A further slice infers `let` bindings from field accesses: `let inner = outer.inner; inner.increment();`, `let leaf = root.middle.leaf; leaf.run();`, `let inner = Outer { inner: Inner {} }.inner; inner.increment();`, or `let inner = self.inner; inner.increment();` bind each local to the declared plain struct field type resolved through the same field-chain rules and dispatch the trailing method on it, while unknown fields, unknown base receivers, generic or non-plain field types, and missing methods fail closed. Malformed source, path-semantic modules, duplicate declarations/import aliases, ambiguous layouts, and ambiguous parent chains fail closed; wildcard imports are not considered. Trait-implementation members are not indexed. Inline-module, Cargo, and import resolution beyond those exact bindings remain capability-gated until dedicated adapters and fixtures establish safe behavior. A further slice adds Rust patch binding validation: identifier references inside a patched symbol resolve to visible local parameters and `let`, `for`, `match`, closure, or `if let` pattern bindings, same-file item declarations, or `use`-introduced names, while type annotations, field and method names, path-qualified names, macro invocations, and standard prelude names are ignored, and unknown bare identifiers fail closed. The current Go slice supports `.go` parsing, raw Tree-sitter queries, semantic skeletons, conservative declaration indexing for named type specifications and aliases, functions, and methods with named local receiver types, and source-position identity. It refreshes importers for static imports strictly below the nearest valid simple `go.mod` module path, mapping the import to direct `.go` files in that package directory, and conservatively refreshes matching production sources in the same local package together. It also traces unshadowed bare direct calls to top-level functions declared in the same source file or in one matching production source in the same directory and package, plus unambiguous direct calls through local package imports using an explicit alias or the imported package's declared name, and calls through a named composite literal such as `Counter{}.Value()`, `(&Counter{}).Value()`, or `Box[int]{}.Value()` to one matching production method in the same local package; a direct named type-conversion receiver such as `Scalar(value).Value()`, `(*Scalar)(value).Value()`, `(Scalar)(value).Value()`, or `Box[int](value).Value()` when its base type is one unique same-package production `type` specification; a direct named type-assertion receiver such as `value.(Scalar).Value()` when `Scalar` is one unique same-package production `type` specification; a simple local alias receiver such as `type Alias = Counter; Alias{}.Value()`, `Alias(value).Value()`, or `value.(Alias).Value()` when its alias chain reaches one unique same-package production named `type` declaration without a cycle; or an unshadowed named local receiver, local-type parameter, or a function-body local variable, including variables declared in nested lexical blocks or control-statement initializers, including expression-switch initializers whose bindings remain scoped to the switch statement, and range element bindings inferred from direct array, slice, or map composite literals, `make` slice/map/channel expressions, pointer-to-array collection parameters, local collection bindings, named collection parameters, or unique same-file named collection types and aliases with a proven array, slice, map, or channel element type; ambiguous, unresolved, and cyclic collection aliases fail closed; generic collection declarations and aliases are expanded only when every type argument and element type resolves uniquely to a same-file named type; type-switch aliases remain binding-only for this trace slice and fail closed when their dynamic receiver type cannot be proven, including variables initialized from a parenthesized or address-of composite literal, from a same-file named type conversion such as `Scalar(value)` or `Box[int](value)`, or from a same-file factory call whose declared return type is one unique named local type, including a single named result parameter, parenthesized result type, or same-file alias chain that resolves uniquely to that type; multi-name `var` initializers are matched positionally only when the number of values exactly matches the number of names, and ambiguous, unresolved, or multi-result factory shapes fail closed; ambiguous, unresolved, or cyclic alias chains fail closed; direct factory-call member receivers remain conservative and retain the factory dependency. Direct calls through a local interface that directly embeds one uniquely resolved same-package interface may resolve to the embedded interface's unique method specification when other direct parents do not declare that method; recursive or multi-level embedding, imported or qualified embedded interfaces, implementation dispatch, and ambiguous parent declarations remain fail-closed. If a conversion-shaped receiver name has no matching local type specification, it retains a direct factory-function dependency instead of guessing a method target. A further slice resolves unambiguous direct calls through explicitly imported local package named types and simple type aliases for composite literals, named conversions, and type assertions, including explicit import aliases and generic type spellings; aliases are followed only through one unique production declaration, including conservative pointer, parenthesized, and generic spellings that normalize to a simple named target, and cyclic, malformed, or ambiguous alias chains fail closed; only exported imported type and method names are eligible, the imported production package must have one valid package declaration, and unresolved, ambiguous, shadowed, or invalid package bindings fail closed. Same-package factory-result method calls are resolved through one unique, unnamed single-result interface return; named results, multi-result factories, ambiguous factories, shadowed factory names, and concrete returns fail closed while retaining only the factory dependency where it is statically known. Qualified imported factory-call receivers whose return type cannot be proven retain their direct imported factory-function dependency rather than guessing a method target. Module-root imports, external modules, `replace`, `go.work`, vendoring, build tags, general cross-file/package/import resolution, interface dispatch, and other method dispatch remain capability-gated until their own fixtures establish safe behavior. Structural patch targeting covers functions, methods, and type specifications and aliases by semantic path or source position with syntax-level validation; a further slice adds Go patch binding validation: identifier references inside a patched symbol resolve to visible receiver, parameter, named-result, `:=`, `var`/`const` spec, range-variable, `if`/`for`/`switch` initializer, type-switch alias, and closure-parameter bindings plus same-file function, type, `var`, and `const` declarations and explicit or default import names (excluding blank and dot imports, which do not bind a package identifier), while type annotations, field and method names, type-assertion and conversion types, package-qualified type spellings, labels, blank identifiers, and predeclared Go names are ignored, and unknown bare identifiers fail closed. Local module import binding validation is bounded by the caller's deadline, caps both package-directory entries and import-spec traversal, and fails closed when a bound is exceeded or the imported production package cannot be parsed uniquely or its package declarations disagree. Java now contributes `.java` routing, raw Tree-sitter query execution, package-qualified semantic skeletons, and conservative declaration indexing for top-level and nested classes, interfaces, enums, annotation types, fields, methods, and constructors. Java overload IDs are assigned only for duplicate methods and constructors within one file. It refreshes importers for explicit local type imports (including nested type imports such as `import com.example.Outer.Inner;`), explicit single-member static imports (including nested static-member imports such as `import static com.example.Outer.Inner.method;`), direct superclass links whose base resolves from the same package, a unique explicit local type import, or an exact qualified local source spelling, and direct interface links whose interface resolves by the same local-source rules. Those links require an owning type that resolves to a local `.java` file under an ancestor source root; wildcard imports, static wildcard imports, missing, and ambiguous source paths fail closed. It traces an explicit `this(...)` constructor initializer only when one same-type, same-file, non-varargs constructor has the call arity; a direct local-source `super(...)` constructor initializer only when one unique direct base-class non-varargs constructor has the call arity; plus unqualified and `this.method()` calls only when one same-type, same-file, non-varargs method has the call arity; `Type.method()` calls through a unique explicit non-static local type import when the type name is unshadowed; and bare calls through a unique explicit local static-method import only when no same-type method has that name. It also traces a `Type.method()` call from a top-level caller class to a unique same-package top-level class or interface static method with an exact,
non-varargs arity match, plus `Outer.Helper.method()` through a unique same-package or explicitly imported outer type and nested class. It also traces `receiver.method(...)` calls whose leading receiver is a locally bound value (formal parameter, declared local, or enclosing-class field) to a unique non-static, non-varargs instance method with a unique arity match on the receiver's declared class type (generic declared types such as `Box<String>` normalize to the raw base type), resolved from the same package, an explicit local type import, a nested scope, or an exact qualified spelling and walked up a unique local-source superclass chain, a `var` local receiver infers its class type from a constructor initializer such as `var helper = new Helper()`, including dotted nested types such as `new Outer.Inner()`, or from the declared return type of a unique same-file same-type factory method or unique explicit static-method import when the initializer is a bare method call such as `var value = makeFoo()`, or of a unique instance method call on a locally bound receiver such as `var value = group.makeFoo()`, `var value = new Group().makeFoo()`, `var value = this.makeFoo()`, `var value = super.makeFoo()`, or `var value = group.inner().makeFoo()` with each receiver hop resolved through the same member-chain rules (while factory-inferred `var` receiver hops, unbound or static type receivers, and unknown or ambiguous qualified callees fail closed), with the factory return type resolved in the factory's own file and package scope, and then dispatches like any other typed receiver, while constructor-call receivers such as `new Helper().helper(...)` or `new Outer.Inner().helper(...)` dispatch directly on the constructed type path, receivers declared with an interface type resolve to a method directly declared on that interface (abstract or default), or to a method declared on a uniquely resolved direct local super-interface chain when the interface itself does not declare the method, while branching super-interface chains resolve only when exactly one branch provides a uniquely arity-matched method and every other branch proves it has no declaration (a declaration reached identically through multiple branches still resolves once), and competing, ambiguous, cyclic, or unresolvable branches fail closed, member chains such as `group.member.helper(...)` resolve each intermediate field's declared class or interface type, resolved in the field's own file and enclosing scope, before dispatching the final method, and constructor-call chains such as `new Group().inner.helper(...)` dispatch the same way through the constructed type path, and member chains through zero-argument method-call hops such as `group.inner().helper(...)` or `new Group().inner().helper(...)` resolve each intermediate call's declared return type (generic return types normalize to the raw base type) in the called method's own file and enclosing scope before dispatching the final member, and `this.`-rooted chains such as `this.member.helper(...)` or `this.inner().helper(...)` dispatch on the enclosing type through the same member-chain rules while plain `this.method()` calls keep the same-type contract, and `super.`-rooted chains such as `super.inner().helper(...)` or `super.member.helper(...)` dispatch on the unique local-source direct superclass type path through the same member-chain rules while plain `super.method()` calls keep the direct-base-chain contract, and class-typed receivers whose class and direct superclass chain do not declare the method dispatch a uniquely arity-matched non-varargs `default` method through resolved direct local interfaces when exactly one interface chain or branch provides it and every other chain or branch proves it has no declaration, while competing, unresolved, or ambiguous chains and nearer same-name class declarations fail closed, and bound receivers with unknown, primitive, or varargs declared types, malformed generic or array spellings, non-constructor, non-factory `var` initializers and unresolvable factory initializers (unknown or ambiguous factories, factories without a usable declared return type, or arity mismatches), qualified initializer callees with factory-inferred `var`, unbound, or static type receiver hops, anonymous-class constructor receivers, interface receivers whose interface and direct super-interface chain do not declare the called method, unknown or ambiguous chain hops, unknown `this.`-rooted or `super.`-rooted chain hops, and method-call hops with non-empty argument lists, static hops, or primitive or void return types, and static methods reached through instance references fail closed instead of falling through to same-named static type calls. Matching callers are re-resolved during refresh without reindexing
  unchanged Java source files. As a deliberately limited interface-dispatch slice, a bare or explicit `this.` call in a class with no explicit `extends` clause, or with one uniquely resolved direct local superclass chain that declares no method of that name, and one or more uniquely resolved direct local interfaces, including lexical outer-scope interface references, trace only when exactly one interface chain or branch provides a directly declared, uniquely arity-matched non-varargs `default` method and every other chain or branch proves it has no declaration of that method. Explicit `super.method()` calls and bare calls without a same-type
  declaration also walk a unique local-source chain of direct base classes, resolved from the same package, a unique explicit local type import, or an exact qualified local source spelling; cycles, ambiguous
  classes, nested/outer generic bases, and nearer nonmatching declarations fail closed. A generic direct base such as `Base<String>` or `com.base.Base<String>` is normalized to its simple or exact qualified base name without type-argument selection. A same-package or explicitly imported outer-qualified direct base such as `Outer.Base` is supported only when one local outer source file and one indexed nested `class_declaration` remain. Wildcard outer imports and broader nested/outer scope semantics beyond this direct source or explicit-import form remain unsupported. Imported targets must be static with exactly one non-varargs arity match. Static field imports, general interface dispatch with competing defaults or abstract declarations, branching or ambiguous interface-inheritance chains, superclass chains that are unresolved, ambiguous, cyclic, or declare the called method name, and broader member dispatch, instance/member dispatch other than explicit simple `super.method()` calls, inherited bare calls across unique local-source base chains, and overload type selection remain capability-gated until dedicated resolution and workspace fixtures establish safe behavior. Structural patch targeting covers classes, interfaces, enums, records, annotation types, methods, constructors, and nested types by semantic path or source position with syntax-level validation; a further slice adds Java patch binding validation: identifier references inside a patched symbol resolve to visible formal parameters, local declarators, `for` and enhanced-`for` variables, catch parameters, try-with-resources variables, lambda parameters, pattern variables, record components, same-file type, method, field, constant, and enum-constant declarations, or explicit single-name imports, while type annotations, field and method names, package-qualified type spellings, cast/instanceof/object-creation types, labels, annotation names, wildcard imports, and predeclared Java names are ignored, and unknown bare identifiers fail closed. C# now contributes `.cs` routing, raw Tree-sitter query execution, namespace-qualified semantic skeletons, and conservative declaration indexing for block and file-scoped namespaces, classes, structs, interfaces, enums, records, methods, and constructors. Its skeleton paths support bounded expansion selectors, while C# methods and constructors with duplicate semantic paths receive stable overload identities. It traces an unshadowed same-type unqualified or explicit `this.` method call, a bare or explicit `this.` inherited instance call from a non-static caller when no same-type method name is indexed, a `: this(...)` constructor initializer, and a conservative `: base(...)` constructor initializer and `base.Method()` call through a unique class/record ancestor chain with a simple or generic, unshadowed qualified, `global::`, local, or root-level global type-alias/namespace-import base type, a globally namespace-qualified `global::...` static call, a unique simple same-namespace or enclosing-namespace `Type.Method()` static call across the workspace, a bounded `Outer.Helper.Method()` or `AliasOuter.Helper.Method()` static call through one unique local/imported outer type or unique local/global type alias and unique nested types in the caller's namespace, an enclosing namespace, imported namespaces, or root-level global usings, including from a nested source type, an explicit type-alias call of the form `Alias.Method()`, a `using static Fully.Qualified.Type;` bare call, a root-level `global using static Fully.Qualified.Type;` bare call contributed by any scanned C# source file (including directive-only files), and a `using Fully.Qualified.Namespace;` call of the form `Type.Method()`, root-level `global using Fully.Qualified.Namespace;` calls, and root-level `global using Alias = Fully.Qualified.Type;` calls contributed by any scanned C# source file, including directive-only files. Type aliases, static imports, and namespace imports may be declared at file root or directly in a block/file-scoped namespace. Aliases resolve from the caller's namespace through enclosing namespaces to file root, with the nearest binding taking precedence; static and ordinary imports from each of those scopes are considered. Duplicate aliases in the same scope, duplicate imports, and ambiguous targets fail closed. Same-type unqualified/`this.` forms require one same-file non-`params` target with the call arity; when no same-type method name is indexed, a non-static caller may resolve a bare or explicit `this.` call through the same unique class/record ancestor chain used by `base.Method()`; globally qualified, simple same-namespace, and imported static calls may resolve exactly one matching workspace target. Imported targets must have exactly one indexed type declaration, while same-file type-name collisions, competing imported targets, and ambiguous type declarations fail closed. A static import is considered only for a bare call with no same-type method of that name; a namespace import is considered only when no matching receiver type exists in the caller's namespace anywhere in the workspace. Local declarations, lambda parameters, type parameters, and enclosing-type fields/properties/events suppress unqualified and simple type-qualified call facts; a locally bound receiver with a usable declared type records an instance `receiver.method(...)` fact instead, while untyped bindings fail closed. Qualified targets must be static. `base(...)` and `base.Method()` require one unique class/record base declaration. Simple or generic, `global::`, and exact unshadowed qualified base names, plus unique unshadowed aliases or namespace imports declared at file root, in the caller's namespace or an enclosing namespace, or as root-level global usings contributed by scanned C# sources (including directive-only files), are supported. A qualified name fails closed when its first segment is an alias or when a source-namespace-relative type could shadow it. Constructor targets must have an exact-arity, non-`params` match. `base.Method()` walks only a unique class/record ancestor chain when no nearer indexed method has the same name; the first indexed method found must be one non-static, exact-arity, non-`params` method, and a `base.`-rooted member chain such as `base.member.helper(...)` or `base.inner().helper(...)` walks each intermediate hop (field, property, event, or arity-matched non-static method-call hop) on the unique class/record base type through the same member-chain and method-call-hop rules before dispatching the final member, with unknown, ambiguous, unresolvable, static, or arity-mismatched hops and missing or static final members failing closed. Generic base, static type-receiver, type-alias, and static-import spellings with balanced type-argument lists (for example `Outer<int>.Method()` and `Outer<int>.Helper<string>.Method()`) are normalized to their declaration paths and resolve only when one indexed target path remains. Generic arity and type-argument selection, ambiguous or colliding alias/import, non-class/record namespace-import base types, cycles, and nearer static, `params`, ambiguous, or arity-mismatched methods fail closed. When any C# source changes, refresh conservatively re-resolves all indexed C# symbols against tracked source paths, including directive-only global-using files, without reindexing unchanged C# sources; stale byte ranges for changed refreshed paths do not block a valid source shrink before rebuild replaces them. It also traces `receiver.method(...)` calls whose leading receiver is a locally bound value (formal parameter, typed local, enclosing-type field, enclosing-type property/event, or a `var` local whose initializer is a constructor call such as `var helper = new Helper()` or `var helper = new Outer.Inner()`) to a unique non-static, non-`params`, exact-arity instance method on the receiver's declared class or record type (generic declared types such as `Box<int>` normalize to the raw base type), resolved in the caller's namespace, an enclosing namespace, an imported namespace, or an explicit `global::` spelling, with dotted declared types such as `Outer.Inner` resolving through the caller's namespace ancestors and then the global scope, and walked up a unique class/record ancestor chain when the declared type itself does not index the method, while a struct-typed receiver dispatches directly to the struct's own non-static, non-`params`, exact-arity method (structs have no ancestor chain). Bound receivers shadow same-named types; receivers bound without a usable declared type (non-constructor `var` locals, lambda parameters, `foreach` variables, local functions, and type parameters), unknown or primitive declared types, non-class/record/struct declared types, and nearer same-name static, `params`, ambiguous, or arity-mismatched methods fail closed instead of falling through to same-named static type calls. A constructor-call receiver such as `new Helper().run(...)` or `new Outer.Inner().run(...)` resolves the constructed type through the same declared-type rules (simple names through the caller's namespace and import scopes, dotted names through namespace ancestors and then the global scope) and dispatches the member as a unique non-static, non-`params`, exact-arity instance call on the constructed class or record type or its unique ancestor chain, while static methods reached through fresh instances, unknown or ambiguous constructed types, missing members, and anonymous creations fail closed. A constructor-rooted member chain such as `new Group().Make().Run(1)`, `new Group().holder.Make().Run(1)`, `new Group().GetWorker().Run(1)`, or `new Outer.Inner().Make().Run(1)` walks each intermediate hop (field, property, event, or arity-matched non-static method-call hop) on the constructed type through the same member-chain and method-call-hop rules, with unknown, ambiguous, unresolvable, static, or arity-mismatched hops and missing or static final members failing closed. A member chain such as `group.member.helper(...)` whose leading receiver is a bound value resolves each intermediate hop to a uniquely declared field, property, or event on the current type (the hop's declared type resolves in the declaring type's own file and import scope, so class/record, struct, and interface-typed hops dispatch the final member with the same unique-dispatch rules), while unknown, ambiguous, or unresolvable intermediate hops, hops whose declared type is not indexed, and missing or static final members fail closed, and a member chain whose intermediate hops are method calls, such as `group.inner().helper(...)` or `group.inner(1).helper(...)`, dispatches each hop method as an arity-matched non-static instance call and continues on its declared return type (resolved in the called method's own file and enclosing scope, with generic return types normalized to their raw base type) before dispatching the final member with the same class/record, struct, and interface rules, while unknown, ambiguous, or arity-mismatched hops, static hops, and primitive or `void` return types fail closed. A `this.`-rooted member chain such as `this.member.helper(...)` walks the same hops on the enclosing type (which must be uniquely declared in the source file), with the same fail-closed rules for unknown or unresolvable hops and missing or static final members. An interface-typed receiver such as `IWorker worker` dispatches to a unique non-static, non-`params`, exact-arity method declared on that interface or on one branch of its unique interface-extends chain (the interface resolves through the caller's namespace, enclosing namespaces, namespace imports, dotted type paths, or an explicit `global::` spelling, and each parent interface resolves through the declaring interface's own namespace and import scope), a declaration on an interface shadows inherited declarations so a same-name static, `params`, or arity-mismatched method blocks parent lookup, a declaration reached identically through multiple parent branches still resolves once, and static interface members, methods missing from the interface and its extends chain, competing declarations across parent branches, cyclic or unresolvable parent interfaces, and unresolved or ambiguous interface types fail closed. Structural patch targeting covers classes, structs, interfaces, enums, records, methods, and constructors by semantic path or source position with syntax-level validation; a further slice adds C# patch binding validation: identifier references inside a patched symbol resolve to visible formal parameters, local declarators, `for` and `foreach` variables, catch parameters, using-resource variables, lambda and anonymous-method parameters, local functions and their parameters, pattern variables, out-variables, query variables, record components, same-file type, method, field, property, event, and enum-constant declarations, or aliased using directives, while type spellings, member names, labels, attribute names, cast/is/as/object-creation types, `nameof`/`typeof`/`sizeof` types, wildcard-like namespace and static imports, and standard C# names are ignored or fail closed, and unknown bare identifiers fail closed.

Suggested order:

1. Rust: `mod`, `use`, functions, `impl`, traits, associated items;
2. Go: packages, imports, functions, methods, interfaces;
3. Java: establish parsing and raw-query compatibility, then package-qualified semantic skeletons, declaration indexing, conservative explicit type/static-import refresh, same-type/explicit-`this`, explicit-local-type static-call, and explicit-static-import tracing, overload identity, and conservative resolution;
4. C#: parsing, raw-query compatibility, semantic skeletons, declaration indexing, same-type
   unqualified/explicit-`this` method, inherited bare/explicit-`this` instance-method tracing through unique ancestor chains, `this(...)`, conservative `base(...)` constructor-initializer, `base.Method()` tracing through unique ancestor chains, generic-base tracing, local/global alias-base and namespace-import-base tracing, unshadowed qualified-base tracing, globally qualified static-call tracing,
   cross-file simple same-namespace and enclosing-namespace static-call tracing and bounded outer/nested-type static-call tracing through direct, namespace-import, or local/global alias roots, including from nested source types, root, exact, and enclosing namespace-scoped
   type-alias/static-import bare-call tracing, root-level global-alias/static/namespace-import tracing from all scanned C# files,
   generic static type-receiver, type-alias, and static-import tracing and conservative dependency refresh are
   established. The C# adapter now advertises source-level file dependencies for explicit type aliases, static type imports, direct base/interface references, and same-directory namespace imports; dependency candidates must parse cleanly and declare the referenced type or namespace, while global using directives, recursive source-root discovery, and MSBuild/package resolution remain outside this bounded slice; add broader
   tracing only through dedicated adapter slices. Kotlin now provides `.kt`/`.kts` routing, raw Tree-sitter queries, package-qualified semantic skeletons, and conservative declaration indexing for top-level and nested classes, interfaces, enums, named objects, functions, simple properties, and type aliases. Local declarations inside function bodies are omitted because they lack stable file-level semantic paths. Kotlin now collects direct-call reference facts and refreshes unique explicit import dependents against `.kt` files that declare the imported package. It also refreshes unique local wildcard-package import dependents by indexing every parseable `.kt` source in the uniquely discovered package directory, while ambiguous package roots remain unresolved. Kotlin dependency resolution now covers both routed source extensions, `.kt` and `.kts`, for explicit imports and wildcard package scans. It traces unqualified direct calls to enclosing-type functions, same-package top-level functions, and unique explicitly imported top-level functions from other packages, and traces qualified receiver calls when the receiver type is pinned to a local class, interface, or type alias by a constructor initializer, explicit type annotation, parameter type, or enclosing-class property; named-object receivers such as `Config.helper(...)` dispatch to the object's members when the name resolves to a uniquely declared same-package or explicitly imported object, class-name receivers such as `Config.helper(...)` dispatch to companion-object members when the name resolves to a uniquely declared class or interface and the member is declared in its companion object while instance members and unknown companion members fail closed, explicit companion chains such as `Config.Companion.helper(...)` dispatch directly to companion members when the class name resolves to a uniquely declared type and no local binding shadows it while instance members, unknown companion members, and extension fallbacks fail closed, named companion objects such as `companion object Factory` are indexed under their declared name so calls such as `Config.Factory.helper(...)` or `Config.Factory.holder.run(...)` dispatch through the same canonical companion scope as the `Companion` spelling while unknown companion names and companion chains rooted at object declarations fail closed, companion property chains such as `Config.Companion.holder.run(...)` resolve each intermediate property's declared type within the companion scope before dispatching the final member or extension while chains through instance properties fail closed, nested companion receivers such as `Outer.Inner.helper(...)`, `Outer.Inner.Companion.helper(...)`, or `Outer.Inner.Factory.holder.run(...)` dispatch through the canonical companion scope of a nested class or interface when the first hop resolves to a uniquely declared type and the second hop names exactly one nested class or interface that hosts a companion while nested types without companions, unknown or ambiguous nested types, and locally shadowed outer names fail closed, and chained receiver calls such as `group.member.helper(...)` or `Config.holder.run(...)` additionally resolve each intermediate property's declared type, falling back to a bare-identifier constructor initializer such as `val member = Other()` when the property has no explicit type, or to the declared return type of a uniquely resolved same-file, same-package, or explicitly imported initializer function such as `val member = makeOther()`, a dotted factory return type such as `fun makeInner(): Outer.Inner` pins a nested receiver through the same dotted type-path rules while missing nested targets and factories without a declared return type fail closed, and a local receiver binding such as `val other = makeOther()` resolves the same way through the factory's declared return type, with the first hop either locally bound or a named object, before dispatching the final member or extension, and generic declared types such as `Box<String>` and nullable declared spellings such as `Box?`, `Outer.Inner?`, or `Box<String>?` normalize to their raw dotted base types for bound parameters, property hops, method-return hops, factory returns, and factory-inferred local bindings while generic arrays such as `Array<Helper>` and nullable generic arrays such as `Array<Helper>?` stay capability-gated for the array slices, and when the pinned type has no matching member, an unambiguous top-level extension function for that receiver type declared in the same file, the same package, or explicitly imported resolves the call, while member functions shadow extensions and qualified non-constructor initializers, function-call initializers whose function has no declared return type or is unknown or ambiguous, unknown chain hops, ambiguous targets, overload type selection, and patch capabilities remain capability-gated, and bare calls to constructible class names such as `Other(...)` resolve to the class declaration through the same scope/import rules, qualified nested constructors such as `Outer.Inner(...)` and nested receiver paths such as `Outer.Inner` in local bindings, property initializers, declared property types, or parameter types resolve through the same dotted type-path rules, dotted type-alias targets such as `typealias Helper = Outer.Inner` expand through the same rules while missing nested targets and cyclic alias chains fail closed, and type aliases declared in or imported from another package such as `import org.util.Helper` with `typealias Helper = Entry` expand their target in the alias's own file and package scope so the trailing member dispatches across packages while unresolvable or generic alias targets fail closed, nested object receivers such as `Outer.Inner.helper(...)` dispatch to the nested object's members when the first hop resolves to a uniquely declared class or object and the second hop names exactly one nested object declaration while unknown or ambiguous nested objects, nested classes or interfaces that share a nested object's name, and locally shadowed outer names fail closed, and constructor-call receivers such as `Outer.Inner().helper(...)` or `Group().member.helper(...)` resolve the constructed type path through the same constructible-class rules and then dispatch the member chain like any other instance receiver while function-call bases such as `makeOther().helper(...)`, unknown or missing types, and non-constructible bases fail closed, and interfaces, enums, and sealed/abstract/annotation/inner classes, unknown or missing nested types, and nested type aliases fail closed. A class-typed receiver also dispatches a member declared on a parent class in its direct superclass chain when neither the class nor any nearer superclass declares it, resolving each supertype in the class's own file and package scope through any number of intermediate classes while nearer declarations shadow inherited ones and unknown members, ambiguous classes, cyclic or unresolvable superclass chains, and competing overload sets fail closed, and member-chain property and method-call hops on a class-typed receiver resolve through the same direct superclass chain and implemented-interface fallbacks before the extension fallback, so inherited hops such as `derived.entry.helper(...)` or `derived.inner().helper(...)` continue the chain on the inherited declared type, and cross-file class-receiver hierarchy hops such as `derived.entry.helper(...)` or `impl.inner().helper(...)` resolve inherited property and method-call hops declared in an explicitly imported package through the same superclass-chain and implemented-interface fallbacks with each hop's declared type resolved in the declaring type's own file and package scope before the trailing member dispatches while unresolvable imported supertypes and implemented interfaces fail closed for the affected hop, and cross-file `this.`- and `super.`-rooted class-receiver hierarchy hops such as `this.entry.helper(...)`, `this.inner().helper(...)`, `super.entry.helper(...)`, or `super.inner().helper(...)` dispatch the same imported superclass-chain and implemented-interface members and hops from inside the enclosing class while unresolvable imported supertypes and implemented interfaces fail closed for the affected root, including hops reached through a diamond-shaped implemented-interface graph such as `class Impl : Diamond` with `interface Diamond : Left, Right`, `interface Left : Root`, and `interface Right : Root` resolving the shared-ancestor hop exactly once while competing or blocked diamond branches fail closed, and cross-file diamond-shaped implemented-interface chains such as `class Impl : Diamond` with `Diamond`, `Left`, `Right`, and `Root` declared in an explicitly imported package resolve the same shared-ancestor property and method-call hops exactly once through the imported interface chain while competing or blocked imported diamond branches fail closed, and a generic interface extends chain declared in an imported package dispatches property and method-call hops declared on the raw generic base with each hop type resolved in the declaring interface's own package while unresolvable hop types fail closed, and an aliased type import such as `import org.util.Base as B` with `class Derived : B()` or `class Impl : R` resolves the imported superclass and implemented interface through the alias binding for the same class-receiver hierarchy dispatch while unresolvable aliased supertypes and implemented interfaces fail closed, and nullable class-typed receiver spellings such as `Derived?` or `Impl?` normalize to their raw class before the same imported class-receiver hierarchy dispatch for property and method-call hops while unresolvable imported supertypes and implemented interfaces fail closed, and a constructed receiver from an imported package such as `Base().entry.helper(...)` or `Base().inner().helper(...)` dispatches property and method-call hops declared on an ancestor class in the imported package's own superclass chain with the hop type resolved in the declaring type's own package while unknown hops, unresolvable hop types, and blocked constructed superclass chains fail closed, and element-access receivers on array-typed parameters, locals, and enclosing-class properties such as `items[0].helper(...)` dispatch on an imported element component type while unresolvable element types and non-array subscript bases fail closed, and companion property chains such as `Config.Factory.holder.run(...)` or `Config.Companion.holder.run(...)` where the companion is declared in an imported package resolve each intermediate property's declared type within the companion scope so the trailing member dispatches across packages while unresolvable companion property types and unknown companion chain hops fail closed, and a `val` local initialized from a qualified factory call such as `val items = Util.makeItems()` where the companion, object, or bound-receiver factory is declared in an explicitly imported package dispatches an element access on the factory return array's element component type resolved in the factory's own package scope while unresolvable component types and primitive array components fail closed, and a bare factory-call element-access receiver such as `makeItems()[0].helper(...)` with `makeItems` declared in an explicitly imported package dispatches the final member on the factory return array's element component type resolved in the factory's own package scope while unresolvable component types and primitive array components fail closed, and a parenthesized member-chain receiver such as `(group).entry.helper(...)`, `(group).inner().entry.helper(...)`, `(makeGroup()).entry.helper(...)`, `(Group()).entry.helper(...)`, or `((group)).entry.helper(...)` where the bound type, hop types, factory, and trailing member are declared in an explicitly imported package unwraps to the same chain spelling and resolves each hop's declared type in the declaring type's own package so the trailing member dispatches across packages while unknown chain hops, unknown parenthesized roots, and nullable parenthesized receivers fail closed, and a `val` local bound from a single-level element access on an array-typed parameter, local property, or enclosing-class property whose element component type is declared in an explicitly imported package inherits that imported component type and dispatches the final member across packages while primitive-component bases, multi-dimensional element access, unknown bases, and unresolvable imported element types fail closed, and a `val` local bound from an element access with a qualified base such as `val first = group.fieldItems[0]` or `val second = group.holder.fieldItems[0]` whose bound and hop types are declared in an explicitly imported package walks each intermediate property's declared type in the declaring type's own package and dispatches the final member on the terminal array field's imported element component type while `this`-rooted bases, method-call bases, unknown receivers, unknown hops, and primitive- or multi-dimensional-component bases fail closed, and a `val` local bound from a factory-call element access such as `val first = makeItems()[0]` whose factory is declared in an explicitly imported package resolves the leading call through the same factory rules and dispatches the final member on the factory return array's element component type resolved in the factory's own package scope while overloaded factories, qualified callees, unknown factories, and primitive-, multi-dimensional-, or non-array-returning imported factories fail closed, and a `val` local initialized from a factory call whose declared return type is a single-level array such as `val items = makeItems()` where the bare factory is declared in an explicitly imported package dispatches an element access on the array's element component type resolved in the factory's own package scope while unknown factories, primitive- or multi-dimensional-returning factories, and non-array-returning imported factories fail closed, and a `val` local bound from a `super`-rooted element access such as `val first = super.inheritedItems[0]` where the direct superclass and the array property's element type are declared in an explicitly imported package dispatches the final member on the element component type resolved in the superclass's own package scope while a direct superclass lacking the array property and unknown superclass array properties fail closed, and a `val` local bound from a `super`-rooted element access such as `val first = super.inheritedItems[0]` where the direct superclass and the array property's element type are declared in an explicitly imported package dispatches the final member on the element component type resolved in the superclass's own package scope while a direct superclass lacking the array property and unknown superclass array properties fail closed, and a `val` local bound from an element access with a companion-object or object root such as `val first = Util.fieldItems[0]` or `val second = Factory.fieldItems[0]` where the static root is declared in an explicitly imported package dispatches the final member on the static root's array property element component type resolved in the static root's own package scope while unknown static roots, unknown static fields, and non-property static roots fail closed, and an object-rooted nested-object chain such as `Outer.Inner.holder.member.run(...)` whose root and nested objects are declared in an explicitly imported package resolves each intermediate property's declared type in the declaring type's own package scope before dispatching the terminal member while unknown nested objects and unknown property hops fail closed, and nullable declared spellings such as `Box?`, `Box<String>?`, and `Entry?` declared in an explicitly imported package normalize to the underlying raw base type for bound parameters, property hops, method-return hops, factory returns, and factory-inferred local bindings so the trailing member dispatches on the same imported raw class declaration while unknown hops, missing final members, unresolvable nullable factory return types, nullable generic array receivers, and nullable value-type receivers fail closed, and a property without an explicit declared type whose initializer is a bare-identifier constructor call such as `val inferred = Other()` pins the hop through the imported constructed type when the enclosing type is declared in an explicitly imported package so the trailing member dispatches across packages while missing properties and factory initializers without a declared return type fail closed, and an aliased import of an outer type such as `import org.util.Outer as O` resolves dotted nested constructors (`O.Inner()`), dotted nested companions (`O.Inner.Companion.helper(...)`), and dotted nested types in property chains (`O.Group().holder.member.helper(...)`) across packages while unknown nested types and unknown property hops fail closed, while unknown hops fail closed, and a class-typed receiver whose class and direct superclass chain do not declare the method dispatches a uniquely arity-matched member declared on one of its directly implemented interfaces when exactly one direct-interface chain provides it and every other chain proves it has no declaration, where a member reached through multiple branches of the same diamond-shaped implemented-interface graph such as `class DiamondImpl : Diamond` with `interface Diamond : Left, Right`, `interface Left : Root`, and `interface Right : Root`, or through two direct interfaces that share a common ancestor declaration, still resolves exactly once, and generic implemented-interface spellings such as `class Impl : IBox<String>` (including cross-file spellings resolved through the explicit import in the class file such as `import org.util.IBox`, dispatching interface members and method-call hops declared in the imported package), generic superclass specifiers such as `class Derived : Base<Entry>()`, and nullable class-typed receiver spellings such as `Derived?` or `Impl?` normalize to their raw base types or class declarations for the same class-hierarchy dispatch while unresolvable raw generic bases fail closed, and same-name declarations in the receiver class hierarchy, competing or unresolved interface chains, and classes without implemented interfaces fail closed. A generic interface-typed receiver such as `IBox<String>` or a nullable generic interface spelling such as `IBox<String>?` normalizes to its raw interface before walking the same interface extends chain for members, property hops, and method-call hops, while competing generic branches, unresolvable generic parent interfaces, and unknown generic hops fail closed. An interface-typed receiver whose member is reachable through multiple branches of the same diamond-shaped interface hierarchy such as `interface Diamond : Left, Right` with `Left : Root` and `Right : Root`, or through one branch that declares the member while every other branch proves it absent, still resolves the member exactly once, and property and method-call hops reached through the same diamond-shaped extends chain continue on the shared ancestor declared type exactly once, while competing declarations on different branches and any blocked (unresolvable or cyclic) branch fail closed. Kotlin makes no Java/JVM source-linkage assumptions and intentionally withholds those resolution capabilities until dedicated Kotlin adapter slices establish their contracts. Structural patch targeting covers classes, interfaces, enums, named objects, functions, simple properties, companion objects, and type aliases by semantic path or source position with syntax-level validation; a further slice adds Kotlin patch binding validation: identifier references inside a patched symbol resolve to visible function parameters, primary-constructor class parameters, local `val`/`var` properties including destructured declarations, `for`-loop variables, lambda and anonymous-function parameters including the implicit `it` lambda parameter, catch parameters, setter parameters, same-file class, object, function, property, type-alias, and class-parameter declarations, or explicit imports, while type spellings, member names, labels, annotation names, `this`/`super` receivers, cast/is/as test types, and auto-imported Kotlin standard-library names are ignored or fail closed, and unknown bare identifiers fail closed.

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

New grammars and language-specific Tree-sitter queries should be added to the existing fuzz and deadline strategy. At minimum, malformed input and queries must not panic, bypass capture limits, or ignore parse deadlines. New source walkers must check existing cooperative deadlines at bounded intervals. C++ qualified/unqualified reference path and type-alias resolution walkers now check these deadlines at bounded intervals.

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
