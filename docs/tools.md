# Tool Guide

This guide summarizes Arborist's tool families and semantic behavior. The exact
MCP schemas are generated from the gateway and checked in at
[`docs/tool-catalog.json`](tool-catalog.json).

As of this revision, `tools/list` returns 58 tools:

- Read tools: 29, including batch reads, semantic skeletons, patch previews, raw Tree-sitter
  queries, symbol reads, symbol list/search, and graph-backed read bundles.
- Write tools: 2, `arborist/patch_ast_node` and
  `arborist/patch_ast_node_at_position`.
- VFS tools: 10, including open/change/close, virtual patching, byte edits,
  commit/discard, and virtual reads.
- Index tools: 9, covering register, unregister, list, inspect, migrate,
  rebuild, workspace refresh, and file refresh for symbol indexes.
- Trace tools: 8, covering graph/neighborhood traces plus trace-backed replay
  and validation.

## Language Support

Arborist uses case-insensitive extension routing with explicit per-language capabilities:

- Python: `.py`, `.pyi` — semantic skeletons, indexing, tracing, patching, and queries.
- C grammar: `.c`, `.h` — semantic skeletons, indexing, tracing, patching, and queries.
- C++ grammar: `.cc`, `.cpp`, `.cxx`, `.c++`, `.tpp`, `.tcc`, `.ipp`, `.inl`,
  `.hpp`, `.hh`, `.hxx`, `.h++` — semantic skeletons, indexing, tracing, patching, and queries.
- JavaScript: `.js`, `.jsx`, `.mjs`, `.cjs` — semantic skeletons, indexing,
  conservative direct-call tracing, static local dependency refresh, structural patching,
  and queries.
- TypeScript: `.ts`, `.mts`, `.cts`; TSX: `.tsx` — the same initial capabilities as
  JavaScript.
- Rust: `.rs` — Tree-sitter parsing, raw queries, semantic skeletons, declaration indexing, and
  conservative local-module dependency refresh for unambiguous out-of-line `mod` declarations.
  It traces unshadowed bare direct calls to functions in the same source-file module, qualified direct
  calls to functions in inline modules in the same source file, and direct
  `module::function()`/`crate::module::function()` calls through an explicit chain of source-file-root
  out-of-line `mod` declarations, selected by semantic path or source position. Every module in the
  chain must use one default-layout source file and the terminal file must contain one matching
  top-level function. Unshadowed bare calls through exact `use crate::module::function;`,
  `use self::module::function;`, or `use super::...` bindings, including grouped `use` paths and explicit
  `as` aliases, resolve through the unique out-of-line parent/module chain. Crate-root imports from
  out-of-line children and repeated `super::` ancestor navigation are supported. Equivalent qualified
  `crate::...` and `super::...` calls from out-of-line children use the same conservative parent/module
  chain. Malformed source, `#[path]` semantics, duplicate declarations/import aliases, ambiguous layouts,
  and ambiguous parent chains fail closed; wildcard imports are not considered. Trait-implementation members are not indexed, and inline-module, Cargo,
  and import resolution beyond those exact bindings remains unavailable. Patching returns an explicit
  unsupported-operation error.
- Go: `.go` — Tree-sitter parsing, raw queries, semantic skeletons, and conservative declaration
  indexing for named type specifications and aliases, functions, and methods with named local receiver
  types, selected by semantic path or source position. Static imports strictly below the nearest valid
  simple `go.mod` module path refresh importers when direct `.go` files in their package directory change;
  matching production sources in one local package also refresh conservatively together. It traces unshadowed
  bare direct calls to top-level functions declared in the same source file or in one matching production
  source in the same directory and package, plus unambiguous direct calls through local package imports using
  an explicit alias or the imported package's declared name, and calls through named composite literals such as
  `Counter{}.Value()`, `(&Counter{}).Value()`, or `Box[int]{}.Value()` to one matching production method in the
  same local package; a direct named type-conversion receiver such as `Scalar(value).Value()`, `(*Scalar)(value).Value()`, `(Scalar)(value).Value()`, or `Box[int](value).Value()` when its base type is one unique same-package production `type` specification; a direct named type-assertion receiver such as `value.(Scalar).Value()` when `Scalar` is one unique same-package production `type` specification; a simple local alias receiver such as `type Alias = Counter; Alias{}.Value()`, `Alias(value).Value()`, or `value.(Alias).Value()` when its alias chain reaches one unique same-package production named `type` declaration without a cycle; or an unshadowed named local receiver, local-type parameter, or directly declared function-body local variable. If the conversion-shaped receiver name has no matching local type specification, it retains a direct factory-function dependency rather than guessing a method target. Module-root imports, external
  modules, `replace`, `go.work`, vendoring, build tags,
  general cross-file/package/import resolution, qualified imported conversions, interface dispatch, and other method dispatch remain unavailable;
  patch operations return explicit unsupported-operation errors.
- C#: `.cs` — Tree-sitter parsing, raw queries, semantic skeletons, declaration indexing, and
  conservative tracing of unshadowed unqualified calls, explicit `this.` method calls, inherited bare/explicit-`this.` instance calls through a unique class/record ancestor chain, and `: this(...)` constructor initializers,
  and conservative `base(...)` constructor initializers and `base.Method()` calls through a unique class/record ancestor chain with simple or generic, unshadowed qualified, `global::`, local, or root-level global type-alias/namespace-import base types,
  globally namespace-qualified `global::...` static calls, simple same-namespace and enclosing-namespace
  `Type.Method()` static calls, including balanced generic type spellings for direct receivers, type aliases, and static imports such as `Outer<int>.Method()`, `using Alias = Outer<int>; Alias.Method()`, or `using static Outer<int>; Method()` that normalize to their declaration paths without type-argument selection, plus `Outer.Helper.Method()` or `AliasOuter.Helper.Method()` static calls when the outer type resolves uniquely in the caller's namespace, an enclosing namespace, or an imported namespace, or a type alias resolves uniquely in those scopes or a root-level global using, and every nested type is unique, including from nested source types, explicit type-alias calls of the form `Alias.Method()`,
  `using static Fully.Qualified.Type;` bare method calls, root-level `global using static Fully.Qualified.Type;` bare
  method calls contributed by any scanned C# source file (including directive-only files), and file-root
  `using Fully.Qualified.Namespace;` calls of the form `Type.Method()`, and root-level `global using Fully.Qualified.Namespace;`
  calls contributed by any scanned C# source file (including directive-only files), and root-level `global using Alias = Fully.Qualified.Type;`
  calls contributed by any scanned C# source file (including directive-only files). Type aliases, static imports, and namespace
  imports may appear at file root or directly in a block/file-scoped namespace; aliases are resolved from the caller's namespace through enclosing
  namespaces to file root, with the nearest binding taking precedence, while static/ordinary imports from each
  of those scopes are considered.
  Same-type unqualified/`this.`
  forms require a unique same-file, exact-arity, non-`params` declaration. When no same-type method name is indexed, a non-static caller may resolve a bare or explicit `this.` call through the same unique class/record ancestor chain used by `base.Method()`. Globally qualified, simple same-namespace,
  and imported static calls may resolve one unique workspace declaration. Imported targets must resolve to one indexed
  type. Duplicate aliases within the same scope, duplicate imports, same-file type-name collisions, competing
  imported targets, and ambiguous type declarations fail closed. Static imports are considered only when no same-type
  method has the bare
  call's name; namespace imports are considered only when the caller's namespace has no type of the receiver name
  anywhere in the workspace. Simple type receivers must not be shadowed by a local, parameter, type parameter, or
  type member. It also traces `receiver.method(...)` calls whose leading receiver is a locally bound value (formal parameter, typed local, enclosing-type field, or enclosing-type property/event) to a unique non-static, non-`params`, exact-arity instance method on the receiver's declared class or record type (generic declared types such as `Box<int>` normalize to the raw base type), resolved in the caller's namespace, an enclosing namespace, an imported namespace, or an explicit `global::` spelling, with dotted declared types such as `Outer.Inner` resolving through the caller's namespace ancestors and then the global scope, and walked up a unique class/record ancestor chain when the declared type itself does not index the method, while a struct-typed receiver dispatches directly to the struct's own non-static, non-`params`, exact-arity method (structs have no ancestor chain), and struct-typed member chains walk field/property/event hops declared on the struct itself before dispatching the final member with no class/record ancestor fallback. A nullable reference-type declared spelling such as Helper? or Outer.Inner? dispatches on the underlying class, record, or interface type, and nullable factory return types and nullable field/property hop declared types pin the same underlying type, while nullable value types such as Point? or int? do not expose the underlying type's members directly and fail closed. Bound receivers shadow same-named types; receivers bound without a usable declared type (`var` locals, lambda parameters, `foreach` variables, local functions, and type parameters), unknown or primitive declared types, non-class/record/struct declared types, and nearer same-name static, `params`, ambiguous, or arity-mismatched methods fail closed instead of falling through to same-named static type calls. A `var` local infers its receiver type from a constructor initializer such as `var helper = new Helper()` or `var helper = new Outer.Inner()`, or from a factory call initializer such as `var helper = MakeHelper()`, `var helper = this.MakeHelper()`, `var helper = holder.MakeHelper()`, `var helper = holder.GetInner().MakeHelper()`, `var helper = base.MakeHelper()`, `var helper = base.inner.MakeHelper()`, or `var helper = Factories.MakeHelper()` when the factory call resolves uniquely as an enclosing-type instance call, an instance method call on a bound receiver's declared type after walking any field/property/event hops through the declared type or its unique class/record ancestor chain (nearest declaring ancestor pins the hop) or arity-matched non-static method-call hops on the receiver, a `base.`-rooted instance call on the unique base chain (directly or after walking any field/property/event hops through the unique class/record ancestor chain or arity-matched non-static method-call hops on the base type), a base-type method, a static-imported method, or a type-qualified static method and its declared return type resolves to a unique indexed type in the factory's own file and enclosing scope (canonicalized to its global-qualified semantic path so callers in other namespaces dispatch the final member on the canonical declared type independently of their own imports), after which the inferred receiver dispatches through the same member-chain and method-call-hop rules as any other typed receiver, or from a field/property-access initializer such as `var helper = helper`, `var helper = this.holder.helper`, `var helper = holder.helper`, `var helper = base.helper`, or `var helper = new Holder().helper`, or from a static type-qualified field root such as `var helper = Util.STATIC_HELPER`, `var helper = global::Demo.Util.STATIC_HELPER`, or `var helper = Outer.Util.NESTED_HELPER`, `var helper = Util.MakeHelper().entry`, or a static-imported member root such as `var helper = STATIC_HELPER` or `var helper = STATIC_HELPER.entry` with `using static Demo.Util;`, or a bare inherited member root such as `var helper = holder` or `var helper = holder.entry` where `holder` is a field or property declared on an ancestor of the enclosing type, or a bare factory-call field root such as `var helper = MakeHelper().entry` or `var helper = MakeHelper(1).entry` where the leading call resolves as a unique arity-matched factory method on the enclosing type, the unique base chain, or a static-imported type and its declared return type pins the receiver before the trailing hops walk, when every root and hop resolves to a uniquely declared field, property, or arity-matched non-static method-call hop whose declared type resolves to a unique indexed type (a bare name pins the declared type of the bound field, property, local, or parameter, `this.`-rooted chains walk field/property/event hops through the unique class/record ancestor chain so the nearest declaring ancestor pins each hop, resolved in the declaring type's scope so a cross-namespace caller dispatches the final member on the canonical declared type, bound receivers walk hops on the declared type or its unique class/record ancestor chain, `base.`-rooted chains walk field/property/event hops through the unique class/record ancestor chain so the nearest declaring ancestor pins each hop, resolved in the declaring type's scope so a cross-namespace caller dispatches the final member on the canonical declared type, while unknown, ambiguous, unresolvable, static, or arity-mismatched hops and missing or static final members still fail closed, and walk method-call hops on the unique base type (resolved in the base type's scope so a cross-namespace caller whose base type is imported from another namespace dispatches the final member on the canonical declared type independently of its own namespace), `Type()`-rooted chains walk field/property/event hops through the unique class/record ancestor chain of the constructed type (nearest declaring ancestor pins the hop) and method-call hops on the constructed type (resolved in the constructed type's scope so a cross-namespace caller dispatches the final member on the canonical declared type), generic receiver spellings such as `Box<int>` or `new Box<int>()` normalize to the raw type before the chain walks, so hops declared on the raw type or its unique class/record ancestor chain still pin the next hop and the final member while type-parameter hop declared types fail closed, bare factory-call roots resolve the leading call as a unique arity-matched factory method on the enclosing type, the unique base chain, or a static-imported type and walk field/property/event hops through the unique class/record ancestor chain of its declared return type (nearest declaring ancestor pins the hop), resolved in the called method's scope so a cross-namespace caller dispatches the final member on the canonical declared type (a leading call-shaped segment that resolves both as a constructed type and as a factory method fails closed when the two interpretations end on different declared types), bound receivers walk field/property/event hops on the declared type or its unique class/record ancestor chain (nearest declaring ancestor pins the hop, resolved in the declaring type's scope so a cross-namespace caller dispatches the final member on the canonical declared type), type-qualified roots resolve the first member as a uniquely declared static field or property, or a unique arity-matched static factory method, on the resolved type (a generic root such as `Util<int>.INSTANCE` or `Util<int>.MakeBox()` normalizes to the raw type before the chain walks, so hops declared on the raw type still pin the next hop and the final member while type-parameter hop declared types fail closed) (`global::`-qualified and dotted type roots resolve through the caller's namespace ancestors and then the global scope, and roots that do not resolve as a plain declared type still resolve through the same alias and namespace-import rules as receiver type references such as `using U = Demo.Util;` or `using Demo;`, with the receiver pinned to the canonical resolved type so a cross-namespace caller dispatches the final member independently of its own namespace), static-imported roots resolve the first member as a uniquely declared static field or property on the single imported type, and bare inherited roots resolve the first member on the nearest ancestor that declares it, with trailing field/property/event hops walking through the unique class/record ancestor chain of the declared member type (nearest declaring ancestor pins the hop), resolved in the declaring type's scope so a cross-namespace caller (`using static Demo.Util;` from another namespace, or a base type imported from another namespace) dispatches the final member on the canonical declared type independently of its own namespace), while non-constructor, non-factory, non-field-access initializers, target-typed creations, array creations, malformed type spellings, unknown, ambiguous, arity-mismatched, `void`, or primitive factories, unknown, ambiguous, untyped, `void`, or primitive field/property-access chains, missing, primitive-returning, arity-mismatched, or ambiguous bare factory-call roots, static type-qualified roots that are unknown, ambiguous, or name a non-static, primitive, or `void` member or an instance, arity-mismatched, primitive, or `void` factory method, or that resolve through an unknown or ambiguous alias or namespace import, static-imported roots whose imported type is unknown, ambiguous, or declares the member as an instance, primitive, or `void` member, and bare inherited roots with no declaring ancestor or with a primitive, `void`, or unresolvable member, fail closed in both same-namespace and cross-namespace callers. A direct call chain rooted at a static type-qualified field or property such as `Util.STATIC_HELPER.entry.Run(1)` or `global::Demo.Util.STATIC_HELPER.entry.Run(1)` walks the static root and each intermediate hop in the declaring type's own scope before dispatching the final member as a unique non-static, non-`params`, exact-arity instance call on the canonical receiver type, so same-namespace, namespace-imported, and `global::`-qualified callers resolve consistently; unknown, ambiguous, or instance-member roots, primitive static members, unresolvable hops, and missing or static final members fail closed. A dotted static root whose first segment comes from a namespace import, such as `Outer.Util.NESTED_HELPER.entry.Run(1)` with `using Demo;` when the nested type is `Demo.Outer.Util`, resolves the nested type through exactly one imported namespace in the declaring type's own scope and walks the same static-member and hop rules, so `var` initializers such as `var helper = Outer.Util.NESTED_HELPER` or `var helper = Outer.Util.MakeHelper().entry` and direct factory-rooted calls dispatch the final member on the canonical declared type consistently across namespaces; unknown or ambiguous namespace roots, missing nested types or members, instance members reached through a nested type name, unresolvable hops, and missing or static final members fail closed. A direct call chain rooted at a bare factory method call such as `MakeHelper().entry.Run(1)`, `MakeHelper().Run(1)`, or `MakeHelper(1).entry.Run(1)` resolves the leading arity-matched factory method on the enclosing type, the unique base chain, or a static-imported type (including root-level global static imports), pins the receiver to its declared return type, walks any remaining hops in the declaring scope, and dispatches the final member as a unique non-static, non-`params`, exact-arity instance call on the canonical declared type, while missing or arity-mismatched factories, primitive return types, unresolvable hops, and missing or static final members fail closed. A parenthesized receiver or hop such as `(MakeFactory()).entry.Run(1)`, `(MakeHelper()).Run(1)`, `(group).inner().entry.Run(1)`, `(this).MakeFactory().entry.Run(1)`, `(new Helper()).entry.Run(1)`, or `(Util.MakeHelper()).entry.Run(1)` unwraps the parentheses and keeps the same chain spelling as the unparenthesized form, so bound, `this`-, constructor-, and type-qualified roots and bare factory-call roots all dispatch the final member on the canonical declared type, and a parenthesized `var` initializer such as `var helper = (MakeFactory())`, `var helper = (new Helper())`, `var helper = (MakeHelper()).entry`, or `var helper = (group)` unwraps to the same binding shape as the unparenthesized form. A direct call chain rooted at a static-imported member such as `STATIC_HELPER.entry.Run(1)` or `STATIC_HELPER.Run(1)` with `using static Demo.Util;` walks the static field or property root and each intermediate hop in the declaring type's own scope before dispatching the final member as a unique non-static, non-`params`, exact-arity instance call on the canonical declared type, so same-namespace and cross-namespace callers resolve consistently; a missing `using static` import, unknown, ambiguous, or instance members, primitive static members, unresolvable hops, and missing or static final members fail closed. A direct call chain that includes method-call hops and is rooted at a static type-qualified factory method or static member, such as `Util.MakeHelper().entry.Run(1)`, `global::Demo.Util.MakeHelper().entry.Run(1)`, `Util.STATIC_HELPER.inner().entry.Run(1)`, or `STATIC_HELPER.inner().entry.Run(1)` with `using static Demo.Util;`, spells the full chain so the resolver dispatches the leading unique arity-matched static factory method or static field root in the declaring type's own scope, walks each method-call and field hop through the same member-chain rules, and then dispatches the final member as a unique non-static, non-`params`, exact-arity instance call on the canonical receiver type, while instance or missing factories, arity mismatches, unknown or instance static roots, unresolvable hops, and missing or static final members fail closed. A constructor-call receiver such as `new Helper().run(...)` or `new Outer.Inner().run(...)` dispatches the member as a unique non-static, non-`params`, exact-arity instance call on the constructed class or record type (dotted constructed types resolve through the caller's namespace ancestors and then the global scope), while static methods reached through fresh instances, unknown or ambiguous constructed types, missing members, and anonymous creations fail closed. A constructor-rooted member chain such as `new Group().Make().Run(1)`, `new Group().holder.Make().Run(1)`, `new Group().GetWorker().Run(1)`, or `new Outer.Inner().Make().Run(1)` walks each intermediate field, property, or event hop through the unique class/record ancestor chain of the constructed type (nearest declaring ancestor pins the hop) and each arity-matched non-static method-call hop on the constructed type, through the same member-chain and method-call-hop rules, with unknown, ambiguous, unresolvable, static, or arity-mismatched hops and missing or static final members failing closed. A member chain such as `group.member.helper(...)` whose leading receiver is a bound value resolves each intermediate hop to a uniquely declared field, property, or event on the current type or its unique class/record ancestor chain (the nearest declaring ancestor pins the hop, and the hop's declared type resolves in the declaring type's own file and import scope, so class/record, struct, and interface-typed hops dispatch the final member with the same unique-dispatch rules), while unknown, ambiguous, or unresolvable intermediate hops, hops whose declared type is not indexed, and missing or static final members fail closed. A member chain whose intermediate hops are method calls, such as `group.inner().helper(...)` or `group.inner(1).helper(...)`, dispatches each hop method as an arity-matched non-static instance call and continues on its declared return type (resolved in the called method's own file and enclosing scope, with generic return types normalized to their raw base type) before dispatching the final member with the same class/record, struct, and interface rules; unknown, ambiguous, or arity-mismatched hops, static hops, and primitive or `void` return types fail closed. A `this.`-rooted member chain such as `this.member.helper(...)` walks each intermediate field, property, or event hop through the unique class/record ancestor chain so the nearest declaring ancestor pins the hop (the enclosing type must be uniquely declared in the source file), with the same fail-closed rules for unknown or unresolvable hops and missing or static final members. An array-access receiver such as `items[0].helper(...)` on a single-level array-typed parameter, typed local, or enclosing-type field or property such as `Helper[] items` dispatches the final member on the array's element component type, including `this.`-rooted element chains such as `this.fieldItems[0].helper(...)` and member chains after an element hop such as `groups[0].item.helper(...)` or `groups[0].inner().helper(...)` with each subsequent hop resolved through the same member-chain and method-call-hop rules on the element type, while a direct member call on the array itself, primitive-component arrays, multi-dimensional or jagged arrays, and non-array or unresolvable element component types fail closed. An interface-typed receiver such as `IWorker worker` dispatches to a unique non-static, non-`params`, exact-arity method declared on that interface or on one branch of its unique interface-extends chain (the interface resolves through the caller's namespace, enclosing namespaces, namespace imports, dotted type paths, or an explicit `global::` spelling, and each parent interface resolves through the declaring interface's own namespace and import scope), a declaration on an interface shadows inherited declarations so a same-name static, `params`, or arity-mismatched method blocks parent lookup, a declaration reached identically through multiple parent branches still resolves once, and static interface members, methods missing from the interface and its extends chain, competing declarations across parent branches, cyclic or unresolvable parent interfaces, and unresolved or ambiguous interface types fail closed. An interface-typed member chain walks field/property/event hops declared on the interface or on one branch of its unique interface-extends chain with the same shadowing, diamond, competing, and cyclic fail-closed rules, resolving each hop's declared type in the declaring interface's own namespace and import scope (so a parent interface imported from another namespace resolves canonically), and a generic interface receiver such as `IBox<int>` normalizes to the raw interface before the walk, before dispatching the final member. Block and file-scoped namespaces, classes, structs, interfaces, enums, records, methods, and
  constructors are supported. `base(...)` and `base.Method()` require one unique class/record base declaration. Simple or generic, `global::`, and exact unshadowed qualified base names, plus unique unshadowed aliases or namespace imports declared at file root, in the caller's namespace or an enclosing namespace, or as root-level global usings contributed by scanned C# sources (including directive-only files), are supported. A qualified name fails closed when its first segment is an alias or when a source-namespace-relative type could shadow it. Constructor targets must have an exact-arity, non-`params` match. `base.Method()` walks only a unique class/record ancestor chain when no nearer indexed method has the same name; the first indexed method found must be one non-static, exact-arity, non-`params` method, and a `base.`-rooted member chain such as `base.member.helper(...)` or `base.inner().helper(...)` walks each intermediate field, property, or event hop through the unique class/record ancestor chain so the nearest declaring ancestor pins the hop, and each arity-matched non-static method-call hop on the unique class/record base type, through the same member-chain and method-call-hop rules before dispatching the final member, with unknown, ambiguous, unresolvable, static, or arity-mismatched hops and missing or static final members failing closed. Generic base spellings with balanced type-argument lists are normalized to their declaration path and resolve only when one indexed class/record path remains. Generic arity and type-argument selection, ambiguous or colliding alias/import, non-class/record namespace-import base types, cycles, and nearer static, `params`, ambiguous, or arity-mismatched methods fail closed. When any C# source file
  changes, refresh conservatively re-resolves every indexed C# symbol against all tracked C# sources, including directive-only
  global-using files; unchanged C# source files are not reindexed, and stale byte ranges from a changed refreshed source that has shrunk do not block its rebuild. Overload type selection and patch operations return
  explicit unsupported-operation errors until dedicated C# adapter
  slices establish their contracts and fixtures.
- Kotlin: `.kt`, `.kts` — Tree-sitter parsing, raw query execution, package-qualified semantic skeletons, and declaration indexing for top-level and nested classes, interfaces, enums, named objects, functions, simple properties, and type aliases. Local declarations inside function bodies are intentionally omitted because they have no stable file-level semantic path. Kotlin now refreshes unique explicit import dependents against `.kt` files that declare the imported package and traces unqualified direct calls to enclosing-type functions, same-package top-level functions, or unique explicitly imported top-level functions from other packages, and traces qualified receiver calls whose receiver type is pinned to a local class, interface, or type alias by a constructor initializer, explicit type annotation, parameter type, or enclosing-class property; named-object receivers such as `Config.helper(...)` dispatch to the object's members when the name resolves to a uniquely declared same-package or explicitly imported object, class-name receivers such as `Config.helper(...)` dispatch to companion-object members when the name resolves to a uniquely declared class or interface and the member is declared in its companion object while instance members and unknown companion members fail closed, explicit companion chains such as `Config.Companion.helper(...)` dispatch directly to companion members when the class name resolves to a uniquely declared type and no local binding shadows it while instance members, unknown companion members, and extension fallbacks fail closed, named companion objects such as `companion object Factory` are indexed under their declared name so calls such as `Config.Factory.helper(...)` or `Config.Factory.holder.run(...)` dispatch through the same canonical companion scope as the `Companion` spelling while unknown companion names and companion chains rooted at object declarations fail closed, companion property chains such as `Config.Companion.holder.run(...)` resolve each intermediate property's declared type within the companion scope before dispatching the final member or extension while chains through instance properties fail closed, nested companion receivers such as `Outer.Inner.helper(...)`, `Outer.Inner.Companion.helper(...)`, or `Outer.Inner.Factory.holder.run(...)` dispatch through the canonical companion scope of a nested class or interface when the first hop resolves to a uniquely declared type and the second hop names exactly one nested class or interface that hosts a companion while nested types without companions, unknown or ambiguous nested types, and locally shadowed outer names fail closed, and chained receiver calls such as `group.member.helper(...)` or `Config.holder.run(...)` additionally resolve each intermediate property's declared type, falling back to a bare-identifier constructor initializer such as `val member = Other()` when the property has no explicit type, or to the declared return type of a uniquely resolved same-file, same-package, or explicitly imported initializer function such as `val member = makeOther()`, a dotted factory return type such as `fun makeInner(): Outer.Inner` pins a nested receiver through the same dotted type-path rules while missing nested targets and factories without a declared return type fail closed, and a local receiver binding such as `val other = makeOther()` resolves the same way through the factory's declared return type, with the first hop either locally bound or a named object, before dispatching the final member or extension, and generic declared types such as `Box<String>`, generic property hops such as `val member: Box<Entry>`, generic method-return hops such as `fun inner(): Box<Entry>`, generic factory returns such as `fun makeBox(): Box<Entry>`, and generic superclass specifiers such as `class Derived : Base<Entry>()` normalize to their raw dotted base types before dispatch while generic arrays such as `Array<Helper>` stay capability-gated for the array slices, and nullable declared spellings such as `Box?`, `Outer.Inner?`, or `Box<String>?` normalize to the same raw base types for bound parameters, property hops, method-return hops, factory returns, and factory-inferred local bindings while nullable generic arrays such as `Array<Helper>?` and nullable value types such as `Int?` fail closed, and when the pinned type has no matching member, an unambiguous top-level extension function for that receiver type declared in the same file, the same package, or explicitly imported resolves the call, while member functions shadow extensions and qualified non-constructor initializers, function-call initializers whose function has no declared return type or is unknown or ambiguous, unknown chain hops, and ambiguous targets fail closed, and bare calls to constructible class names such as `Other(...)` resolve to the class declaration through the same scope/import rules, qualified nested constructors such as `Outer.Inner(...)` and nested receiver paths such as `Outer.Inner` in local bindings, property initializers, declared property types, or parameter types resolve through the same dotted type-path rules, dotted type-alias targets such as `typealias Helper = Outer.Inner` expand through the same rules while missing nested targets and cyclic alias chains fail closed, nested object receivers such as `Outer.Inner.helper(...)` dispatch to the nested object's members when the first hop resolves to a uniquely declared class or object and the second hop names exactly one nested object declaration while unknown or ambiguous nested objects, nested classes or interfaces that share a nested object's name, and locally shadowed outer names fail closed, and constructor-call receivers such as `Outer.Inner().helper(...)` or `Group().member.helper(...)` resolve the constructed type path through the same constructible-class rules and then dispatch the member chain like any other instance receiver while a bare factory-call root such as `makeGroup().entry.helper(...)` or `makeInner().helper(...)` resolves the leading call through the same factory rules as a `var` initializer (a unique same-file, same-package, or explicitly imported top-level function with a declared return type, including dotted nested return types such as `Outer.Inner`) and dispatches the trailing member chain on the factory's declared return type, while unknown or missing types, unknown or ambiguous factories, factories without a declared return type, dotted factory roots, and non-constructible bases fail closed, and interfaces, enums, and sealed/abstract/annotation/inner classes, unknown or missing nested types, and nested type aliases fail closed. An element-access receiver such as `items[0].helper(...)` on a single-level generic array-typed parameter, local property, or enclosing-class property such as `items: Array<Helper>` dispatches the final member on the array's element component type when the base binds uniquely with a usable single-level component, including a simple identifier subscript such as `items[index].helper(...)`, while a direct member call on the array itself such as `items.helper(...)`, primitive-component arrays such as `counts[0].helper(...)`, multi-dimensional arrays such as `matrix[0][0].helper(...)`, member chains after an element hop, and unbound element-access bases fail closed. A bare factory-call element-access receiver such as `makeItems()[0].helper(...)` resolves the leading call through the same factory rules as a property initializer (a unique same-file, same-package, or explicitly imported top-level function with a declared return type) and dispatches the final member on the factory return array's element component type, while unknown factories, primitive- or multi-dimensional-returning factories, non-array-returning factories, and multi-dimensional element access on a factory-returned array fail closed. A `val` local bound from a single-level element access such as `val first = items[0]` inherits the base array's element component type when the base is an array-typed parameter, local property, or enclosing-class property such as `items: Array<Helper>`, and dispatches the final member on that element type, while primitive-component bases such as `val fromCounts = counts[0]`, multi-dimensional element access such as `val fromMatrix = matrix[0][0]`, unknown bases, and plain identifier initializers fail closed. A `val` bound from an element access with a qualified base such as `val first = group.fieldItems[0]` or `val second = group.holder.fieldItems[0]` walks each intermediate property's declared type and dispatches the final member on the terminal array field's element component type, while `this`-rooted bases, method-call bases, unknown receivers, and primitive- or multi-dimensional-component bases fail closed. A `val` bound from a factory-call element access such as `val first = makeItems()[0]` resolves the leading call through the same factory rules as a property initializer (a unique same-file, same-package, or explicitly imported top-level function with a declared return type) and dispatches the final member on the factory return array's element component type, while overloaded factories, qualified callees, unknown factories, primitive- or multi-dimensional-returning factories, and non-array-returning factories fail closed. A `val` local initialized from a factory call whose declared return type is a single-level array, such as `val items = makeItems()` with `fun makeItems(): Array<Helper>` or `val items = Util.makeItems()` with a companion-object, named-object, or bound-receiver callee, dispatches an element access on the array's element component type through the same factory rules, with qualified callees resolving through companion, explicit-companion (`Util.Companion.makeItems` or `Util.Factory.makeItems`), object, or bound-receiver member rules, while direct member calls on the array, unknown factories, non-array return types, constructor initializers, and unresolvable qualified callees fail closed. Parenthesized `val` initializers such as `val constructed = (Helper())`, `val items = (makeItems())`, or `val first = (group.fieldItems[0])` unwrap to the same receiver type or terminal array element component type as the unparenthesized form, including nested parentheses, while parenthesized primitive-array or multi-dimensional element accesses, `this`-rooted bases, unknown factories, and unknown qualified callees fail closed. Parenthesized receivers in member-chain calls such as `(group).entry.helper(...)`, `(group).inner().entry.helper(...)`, `(makeGroup()).entry.helper(...)`, `(Group()).entry.helper(...)`, or `((group)).entry.helper(...)` unwrap to the same chain spelling as the unparenthesized form and dispatch the trailing member or method-call hop on the same resolved receiver, while nullable parenthesized receivers such as `(group)?.entry.helper(...)` and unknown chain hops fail closed. A `this`-rooted receiver such as `this.entry.helper(...)`, `this.helper(...)`, or `this.makeGroup(...)` dispatches on the enclosing type through the same member-chain rules as bound receivers, while unknown `this`-rooted hops and missing final members fail closed. A `super`-rooted receiver such as `super.entry.helper(...)` or `super.baseHelper(...)` dispatches on the direct superclass path through the same member-chain rules, while unknown `super`-rooted hops, missing final members, and classes without a resolvable superclass fail closed. Method-call hops inside member chains such as `group.inner().entry.helper(...)`, `this.makeInner().entry.helper(...)`, `super.inner().entry.helper(...)`, `makeGroup().inner().entry.helper(...)`, or `Group().inner().entry.helper(...)` dispatch a unique member or extension function with a declared return type (member functions shadow extensions, and ambiguous overload or extension sets fail closed) and continue the chain on the hop's declared return type, while unknown hops, primitive or unresolvable return types, and unbound receiver roots fail closed. A method-call hop on an interface-typed receiver such as `builder.inner().helper(...)` with `interface Builder { fun inner(): Entry }` dispatches a method directly declared on that interface from a bound parameter, a property hop that pins an interface type, or an imported interface in another file, while interface methods returning unknown or primitive types and unknown interface method hops fail closed. An interface-typed receiver also dispatches members, property hops, and method-call hops declared on a parent interface in its extends chain, resolving exactly one uniquely declared branch through any number of intermediate interfaces while competing declarations across branches, cyclic or unresolvable parent interfaces, and unknown or ambiguous receiver types fail closed. A class-typed receiver also dispatches a member declared on a parent class in its direct superclass chain when neither the class nor any nearer superclass declares it, resolving each supertype in the class's own file and package scope through any number of intermediate classes while nearer declarations shadow inherited ones and unknown members, ambiguous classes, cyclic or unresolvable superclass chains, and competing overload sets fail closed, and member-chain property and method-call hops on a class-typed receiver resolve through the same direct superclass chain and implemented-interface fallbacks before the extension fallback, so inherited hops such as `derived.entry.helper(...)` or `derived.inner().helper(...)` continue the chain on the inherited declared type, including hops reached through a diamond-shaped implemented-interface graph such as `class Impl : Diamond` with `interface Diamond : Left, Right`, `interface Left : Root`, and `interface Right : Root` resolving the shared-ancestor hop exactly once while competing or blocked diamond branches fail closed, while unknown hops fail closed, and a class-typed receiver whose class and direct superclass chain do not declare the method dispatches a uniquely arity-matched member declared on one of its directly implemented interfaces when exactly one direct-interface chain provides it and every other chain proves it has no declaration, where a member reached through multiple branches of the same diamond-shaped implemented-interface graph such as `class DiamondImpl : Diamond` with `interface Diamond : Left, Right`, `interface Left : Root`, and `interface Right : Root`, or through two direct interfaces that share a common ancestor declaration, still resolves exactly once, and generic implemented-interface spellings such as `class Impl : IBox<String>` (including cross-file spellings resolved through the explicit import in the class file such as `import org.util.IBox`, dispatching interface members and method-call hops declared in the imported package), generic superclass specifiers such as `class Derived : Base<Entry>()`, and nullable class-typed receiver spellings such as `Derived?` or `Impl?` normalize to their raw base types or class declarations for the same class-hierarchy dispatch while unresolvable raw generic bases fail closed, and same-name declarations in the receiver class hierarchy, competing or unresolved interface chains, and classes without implemented interfaces fail closed. A generic interface-typed receiver such as `IBox<String>` or a nullable generic interface spelling such as `IBox<String>?` normalizes to its raw interface before walking the same interface extends chain for members, property hops, and method-call hops, while competing generic branches, unresolvable generic parent interfaces, and unknown generic hops fail closed. A member reachable through multiple branches of the same diamond-shaped interface hierarchy, or through one branch that declares it while every other branch proves it absent, resolves exactly once, and property and method-call hops reached through the same diamond-shaped extends chain continue on the shared ancestor declared type exactly once, while competing declarations on different branches and any blocked (unresolvable or cyclic) branch fail closed. It still makes no Java/JVM source-linkage assumptions and does not advertise patch operations.
- Java: `.java` — Tree-sitter parsing, raw queries, semantic skeletons, declaration indexing, and
  conservative dependency refresh for explicit local type imports (including nested type imports such as
  `import com.example.Outer.Inner;`), single-member `import static`
  imports (including nested static-member imports such as `import static com.example.Outer.Inner.method;`), direct superclass links whose base resolves from the same package, a unique explicit local type import, or an exact qualified local source spelling, and direct interface links whose interface resolves by the same local-source rules. Those links require an owning type that maps to a local
  `.java` file under an ancestor source root. Classes,
  interfaces, enums, annotation types, fields, methods, and constructors use package-qualified paths. It
  traces an explicit `this(...)` constructor initializer only when one same-type, same-file, non-varargs constructor has the call arity; a direct local-source `super(...)` constructor initializer only when one unique direct base-class non-varargs constructor has the call arity; plus unqualified and `this.method()` calls only when one same-type, same-file, non-varargs method has the call arity; `Type.method()` through a unique explicit non-static local type import
  with an unshadowed type name; and a bare call through a unique explicit local static-method import
  only when no same-type method has that name. It also traces a `Type.method()` call from a top-level caller class to a unique same-package top-level class or interface static method with an exact,
  non-varargs arity match, plus `Outer.Helper.method()` through a unique same-package or explicitly imported outer type and nested class. It also traces `receiver.method(...)` calls whose leading receiver is a locally bound value (formal parameter, declared local, or enclosing-class field) to a unique non-static, non-varargs instance method with a unique arity match on the receiver's declared class type (generic declared types such as `Box<String>` normalize to the raw base type), resolved from the same package, an explicit local type import, a nested scope, or an exact qualified spelling and walked up a unique local-source superclass chain; a `var` local receiver infers its class type from a constructor initializer such as `var helper = new Helper()`, including dotted nested types such as `new Outer.Inner()`, or from the declared return type of a unique same-file same-type factory method or unique explicit static-method import when the initializer is a bare method call such as `var value = makeFoo()`, or of a unique instance method call on a locally bound receiver such as `var value = group.makeFoo()`, `var value = new Group().makeFoo()`, `var value = this.makeFoo()`, `var value = super.makeFoo()`, or `var value = group.inner().makeFoo()` with each receiver hop resolved through the same member-chain rules (while factory-inferred `var` receiver hops, unbound receivers, and unknown or ambiguous qualified callees fail closed), or of a unique directly declared non-varargs static method on a same-package, explicitly imported, exact-qualified, or nested class or interface type such as `var value = Util.make()` or `var value = Util.Nested.nestedMake()`, with the factory return type resolved in the factory's own file and package scope, and then dispatches like any other typed receiver, and a `var` local receiver also infers its class type from a field-access initializer such as `var value = this.helper`, `var value = helper`, `var value = Util.STATIC_HELPER`, or a unique explicit static field import such as `import static com.example.Util.STATIC_HELPER;`, including chains off a statically imported field such as `var value = STATIC_HELPER.entry`, with `this.`-rooted and `super.`-rooted chains such as `var value = this.holder.entry` or `var value = super.holder.entry`, bare field chains such as `var value = holder.entry`, and static member chains such as `var value = Util.REGISTRY.entry` or `var value = Util.Inner.STATIC_ENTRY`, and field chains through arity-matched method-call hops such as `var value = this.inner().entry`, `var value = holder.inner(1).entry`, or `var value = super.holder.inner(2).entry` with each hop's declared type resolved through the same field-chain and method-return-type rules, resolving each field hop's declared type through the same field-chain rules, with bare names and bare field chains also resolving fields inherited from a unique local-source direct-superclass chain, and bound receivers (formal parameters, declared locals, or enclosing-class fields) with a usable declared type resolving field chains such as `var value = local.entry` on that type (bound names shadow same-named qualified type receivers), and constructor-rooted chains such as `var value = new Holder().group.entry` resolving on the constructed class type, and static factory-method-call chains such as `var value = Util.factory().entry` or `var value = Util.Nested.factory().entry` resolving the first hop through a unique directly declared non-varargs static method's declared return type, and factory-method-call chains such as `var value = makeFoo().entry` or `var value = makeFoo(1).entry` resolving the first hop's declared return type through the same same-type, static-method-import, static-type-factory, or bound-receiver-factory rules as a `var` factory initializer with the hop arity matched against the factory's non-varargs parameters, and then dispatching like any other typed receiver, while unknown, ambiguous, or non-static qualified field hops, non-static method-call first hops under qualified `Type.field` references, bound-name shadowing of qualified type receivers, and factory-inferred or otherwise untyped bound names fail closed, while constructor-call receivers such as `new Helper().helper(...)` or `new Outer.Inner().helper(...)` dispatch directly on the constructed type path, and direct anonymous-class constructor receivers such as `new Helper() { }.helper(...)` and anonymous-rooted chains such as `new Group() { }.inner.helper(...)`, `new Group() { }.inner2().inner.helper(...)`, or `new Group() { }.inner2(1).inner.helper(...)` dispatch on the constructed class type when the anonymous body declares neither the final member, a field hop, nor any method-call hop in the chain, with method-call hops arity-matched against the constructed type's methods, including `var` initializers such as `var helper = new Helper() { }`, and anonymous constructor-rooted `var` field-initializer chains such as `var v = new Group() { }.entry`, `var v = new Group() { }.holder.entry`, `var v = new Group() { }.entry2().entry`, or `var v = new Group() { }.entry2(1).entry` canonicalize to `new Group().entry` (or the equivalent chain) and dispatch `v.helper(...)` on the constructed class type when the anonymous body declares no accessed field or method-call hop that would shadow the constructed type's member and method-call hops are arity-matched against the constructed type's methods, receivers declared with an interface type resolve to a method directly declared on that interface (abstract or default), or to a method declared on a uniquely resolved direct local super-interface chain when the interface itself does not declare the method, while branching super-interface chains resolve only when exactly one branch provides a uniquely arity-matched method and every other branch proves it has no declaration (a declaration reached identically through multiple branches still resolves once), and competing, ambiguous, cyclic, or unresolvable branches fail closed, and member chains such as `group.member.helper(...)` resolve each intermediate field's declared class or interface type before dispatching the final method, and a statically imported field root such as `STATIC_HELPER.helper(...)` or `STATIC_HELPER.entry.helper(...)` dispatches through the same member-chain rules on the imported field's declared type while missing or ambiguous fields, methods used as field roots, and unknown hops fail closed and a type-qualified static root such as `Util.STATIC_HELPER.helper(...)`, `Util.STATIC_HELPER.entry.helper(...)`, or `Util.MakeHelper().helper(...)` resolves the leading class or interface prefix through the same same-package, explicit-import, fully-qualified, and nested type rules as other Java receivers, requires the first chain hop to be a uniquely declared static field (walking the direct-superclass chain) or an arity-matched static factory call, and dispatches the trailing member chain on that root's declared type while missing or ambiguous types, fields, or factories, methods used as field roots, non-static roots, arity mismatches, and unknown hops fail closed and generic declared or factory-return root types such as `static Box<String> STATIC_HELPER` or `static Box<String> MakeBox()` normalize to the raw base type before the trailing member chain dispatches, while type-argument-prefix spellings such as `Box<Integer>.STATIC_HELPER.helper(...)` that the Java grammar does not parse as method calls produce no fact and fail closed and a bare factory-call root such as `makeFoo().helper(...)`, `makeFoo().entry.helper(...)`, or `MakeHelper(1).helper(...)` resolves the leading call through the same factory rules as a `var` initializer (a unique same-type method or explicit static-method import with matching non-varargs arity) and dispatches the trailing member chain on the factory's declared return type while methods used as field roots (without call parens), unknown or arity-mismatched factories, and unknown hops fail closed, and constructor-call chains such as `new Group().inner.helper(...)` dispatch the same way through the constructed type path, and member chains through arity-matched method-call hops such as `group.inner().helper(...)`, `group.inner(1).helper(...)`, or `new Group().inner().helper(...)` resolve each intermediate call's declared return type (generic return types normalize to the raw base type) in the called method's own file and enclosing scope before dispatching the final member, and `this.`-rooted chains such as `this.member.helper(...)` or `this.inner().helper(...)` dispatch on the enclosing type through the same member-chain rules while plain `this.method()` calls keep the same-type contract, and `super.`-rooted chains such as `super.inner().helper(...)` or `super.member.helper(...)` dispatch on the unique local-source direct superclass type path through the same member-chain rules while plain `super.method()` calls keep the direct-base-chain contract. A parenthesized receiver such as `(group).helper(...)`, `(group).inner().entry.helper(...)`, `(makeFoo()).entry.helper(...)`, `(MakeHelper()).helper(...)`, `(new Group()).entry.helper(...)`, `(this).makeFoo().entry.helper(...)`, or `(Util.MakeHelper()).entry.helper(...)` unwraps to the same chain spelling as the unparenthesized form, so bound, same-type factory, static-imported factory, constructor-, `this`-, and type-qualified factory roots all dispatch the final member on the canonical declared type; malformed or empty parentheses and parenthesized forms the Java grammar does not parse (such as `(super).member.helper(...)`) produce no fact and fail closed. A `var` local with a parenthesized initializer such as `var constructed = (new Helper())`, `var factory = (makeHelper())`, `var field = (this.fieldHelper)`, or `var bare = (fieldHelper)` unwraps to the same receiver binding as the unparenthesized form, so the local dispatches the final member on the constructed, factory-returned, or field-declared type while parenthesized arity-mismatched factories, primitive-returning factories, array creations, and unknown factories fail closed. An array-access receiver such as `items[0].helper(...)` on a single-level array-typed parameter, local, or enclosing-class field such as `Helper[] items` dispatches the final member on the array's element component type, including `this.`-rooted element chains such as `this.fieldItems[0].helper(...)`, a parenthesized array root such as `(group)[0].helper(...)` unwrapping to the same element-access spelling, and member chains after an element hop such as `groups[0].item.helper(...)` or `groups[0].inner().helper(...)` with each subsequent hop resolved through the same field-chain and method-call-hop rules on the element type, while a direct member call on the array itself such as `items.helper(...)`, primitive-component arrays such as `counts[0].helper(...)`, multi-dimensional arrays such as `matrix[0][0].helper(...)` fail closed. A bare factory-call root with an element-access suffix such as `makeItems()[0].helper(...)` or `makeGroups()[0].inner().helper(...)` resolves the leading call through the same factory rules as a `var` initializer (a unique same-type method or explicit static-method import with matching non-varargs arity) whose declared return type is a single-level array, then dispatches the trailing member chain on the array's element component type in the factory's own file and enclosing scope; unknown or arity-mismatched factories, primitive or multi-dimensional return arrays, and multi-dimensional element access such as `makeItems()[0][0].helper(...)` fail closed. A `var` local initialized from a factory call whose declared return type is a single-level array, such as `var items = makeItems()` with `Helper[] makeItems()` or `var items = Util.makeItems()` with `static Helper[] makeItems()`, dispatches an element access on the array's element component type through the same factory rules as other `var` initializers, including qualified callees through static type receivers, `this`/`super`-rooted callees, constructed types, and bound-receiver chains, and member chains after the element hop such as `var groups = makeGroups(); groups[0].inner().helper(...)`, while a direct member call on the array, primitive- or multi-dimensional-returning factories, unknown factories, and unresolvable qualified factory callees fail closed. A `var` local bound from an element access infers the base array's element component type and dispatches the final member on that type, for plain-identifier bases such as `var first = items[0]`, `var second = local[1]`, or `var third = fieldItems[0]`, qualified field-chain bases such as `var fourth = this.fieldItems[0]`, `var sixth = super.inheritedItems[0]` on the direct superclass, `var seventh = Util.fieldItems[0]` on a static type (requiring a static terminal field), or `var fifth = group.holder.fieldItems[0]` that walk each intermediate field's declared type before the terminal array field, and factory-call bases such as `var factory = makeItems()[0]` or `var qualifiedFactory = Util.makeItems()[0]` that resolve the factory through the same rules as other `var` initializers before dispatching on the array's element component type, while a primitive-array base such as `var unbound = counts[0]`, multi-dimensional element access such as `var matrixAccess = matrix[0][0]`, an unresolvable `super` base, non-static fields on a static type receiver, and unknown, primitive- or multi-dimensional-returning, or arity-mismatched factory-call bases fail closed. Class-typed receivers whose class and direct superclass chain do not declare the method dispatch a uniquely arity-matched non-varargs `default` method through resolved direct local interfaces when exactly one interface chain or branch provides it and every other chain or branch proves it has no declaration, while competing, unresolved, or ambiguous chains and nearer same-name class declarations fail closed. Bound receivers with unknown, primitive, or varargs declared types, malformed generic or array spellings, non-constructor, non-factory `var` initializers and unresolvable factory initializers (unknown or ambiguous factories, factories without a usable declared return type, or arity mismatches), qualified initializer callees with factory-inferred `var` or unbound receiver hops, anonymous-class constructor receivers whose body declares the invoked member, anonymous-rooted chains whose body declares the final member, a field hop, or a method-call hop, anonymous `var` field-initializer bodies that declare an accessed field or method-call hop, unknown anonymous constructed types, interface receivers whose interface and direct super-interface chain do not declare the called method, unknown or ambiguous chain hops, unknown `this.`-rooted or `super.`-rooted chain hops, and method-call hops with unknown or arity-mismatched argument lists, static hops, or primitive or void return types, and static methods reached through instance references fail closed. Matching callers are re-resolved during refresh without reindexing
  unchanged Java source files. As a deliberately limited interface-dispatch slice, a bare or explicit `this.` call in a class with no explicit `extends` clause, or with one uniquely resolved direct local superclass chain that declares no method of that name, and one or more uniquely resolved direct local interfaces, including lexical outer-scope interface references, trace only when exactly one interface chain or branch provides a directly declared, uniquely arity-matched non-varargs `default` method and every other chain or branch proves it has no declaration of that method. Explicit `super.method()` calls and bare calls without a same-type
  declaration also walk a unique local-source chain of direct base classes, resolved from the same package, a unique explicit local type import, or an exact qualified local source spelling; cycles, ambiguous
  classes, nested/outer generic bases, and nearer nonmatching declarations fail closed. A generic direct base such as `Base<String>` or `com.base.Base<String>` is normalized to its simple or exact qualified base name without type-argument selection. A same-package or explicitly imported outer-qualified direct base such as `Outer.Base` is supported only when one local outer source file and one indexed nested `class_declaration` remain. Wildcard outer imports and broader nested/outer scope semantics beyond this direct source or explicit-import form remain unsupported. Imported targets require a
  unique static-method arity match. Wildcard imports, static wildcard imports, static field imports, missing or ambiguous
  imports, general interface dispatch with competing defaults or abstract declarations, branching or ambiguous interface-inheritance chains, superclass chains that are unresolved, ambiguous, cyclic, or declare the called method name, and broader member dispatch, instance/member dispatch other than explicit simple `super.method()` calls, inherited bare calls across unique local-source base chains, overloaded-call selection, and patch operations return explicit
  unsupported-operation errors.

C++ files use the dedicated `tree-sitter-cpp` grammar. C-family symbol
indexing, tracing, raw-query owner metadata, and patch target resolution cover
free functions in named namespaces, named methods declared or defined in class
bodies, and header/source families. Symbols use qualified semantic paths, such
as `outer::inner::function` and `outer::Class::method`; same-scope calls prefer
matching symbols during graph resolution. Named methods defined outside their
class are matched to the same semantic path as their class-body declarations.
Class definitions are indexed with their namespace and enclosing-class scope.
Explicit constructors and destructors use `Class::Class` and `Class::~Class`
paths. Defaulted/deleted methods retain their full declaration signature.
Named function and class-method templates are indexed, traced, and exposed to
raw query owner metadata with their `template <...>` declaration text. Template
function and class/method specializations have distinct paths such as
`increment<int>` and `Box<int>::value`. Non-type template parameters are local
bindings during patch validation and reference tracing. C++ callable
`semantic_path` values remain overload-set paths such as `api::convert`, while
their stable `symbol_id` values use normalized parameter types and member
qualifiers, such as `api::convert(int)`, `api::convert(double)`, and
`api::Counter::value() const`. Use the precise ID to read, trace, patch, or
expand one overload; the semantic path remains a compatibility selector.
For direct C++ function calls, graph resolution filters callable overloads by
argument count before applying its existing scope ranking. Defaulted and
variadic parameters are included in that check. Namespace-qualified calls such
as `api::convert(value)` first resolve through enclosing namespaces, then use
the same overload filtering.
Explicit template calls such as `convert<int>(value)` prefer an indexed exact
specialization and otherwise fall back to the primary template for graph
resolution.
Calls through `this->method(value)`, `(*this).method(value)`, and dependent
member-template syntax such as `this->template method<T>(value)` resolve
against the enclosing class's method overloads by argument count; `const`
member callers prefer matching `const` overloads, including declarations whose
top-level cv qualifiers are written as either `const volatile` or `volatile const`.
Because `this` receivers are lvalues, matching `&` and `const &` member overloads
are preferred over `&&` overloads. Explicit rvalue self calls through
`std::move(*this).method(value)`
or `static_cast<T&&>(*this).method(value)` prefer matching `&&` member
overloads; `const`-qualified casts select matching `const &` or `const &&`
overloads. `std::as_const(*this).method(value)` selects a matching `const &`
member overload. `std::forward<T>(*this).method(value)` follows the explicit
template argument's value category and top-level `const` qualification.
Direct type constructions such as `Counter(value)`, `Counter{value}`, and
`new api::Counter` and `new api::Counter(value)` resolve to the matching
constructor overload by argument count. Template constructions such as
`api::Box<int>{value}` fall
back to the primary class template when an explicit specialization is not
indexed; this applies to `new api::Box<int>(value)` as well.
Member calls on direct temporary constructions, such as
`api::Counter{}.adjust(value)`, resolve against the constructed type's member
overloads and prefer matching `&&` qualifiers; the same applies when the
temporary is wrapped in `std::move` or an explicit `static_cast<T&&>`. A
`static_cast<const T&>` or `static_cast<const T&&>` temporary selects matching
const-qualified member overloads; `std::forward<T>` follows its template
argument's value category and const qualification.
Type aliases are expanded for direct temporary member calls, so `using Alias =
api::Counter; Alias{}.adjust(value)` resolves against `api::Counter` overloads.
Member calls on explicitly typed local C++ objects and function parameters are
resolved too: after `Alias current{};` or `Alias& current`,
`current.adjust(value)` follows the `&` overload, while `const Alias current{}`
or `const Alias& current` follows `const &` and
`std::move(current).adjust(value)` follows `&&`. Local bindings are selected
lexically, so an inner declaration with the same name shadows an outer object
for graph tracing; range-for bindings follow the same rules. Directly typed raw pointers are also resolved through `->`,
so `Alias* current; current->adjust(value)` follows the pointee's `&` overload
and `const Alias* current` follows `const &`; the equivalent
`(*current).adjust(value)` form is resolved as well.
`auto` bindings from `std::addressof(value)` or `&value` retain the same
pointee receiver behavior.
`auto&`, `const auto&`, `auto const&`, and named `auto&&` bindings retain the referenced
object's lvalue and const receiver behavior, including bindings initialized
with `std::move(value)`, `std::as_const(value)`, `std::forward<T>(value)`, or
`static_cast<T&>(value)`. Bindings from `*pointer` retain the raw pointee's
lvalue and const receiver behavior.
`decltype(auto)` bindings preserve the same local receiver behavior for
parenthesized lvalues, xvalues, pointer and optional dereferences, and
reference-wrapper `.get()` calls; a bare identifier follows its declared
`decltype` type, including top-level `const`.
Equivalent address-expression aliases such as `*std::addressof(value)`,
`*std::addressof(std::as_const(value))`, and `*&value` retain the addressed
object's lvalue and const receiver behavior. Direct `->` calls through those
same address expressions are resolved as well. An explicit
`static_cast<T&>(value)` inside the address expression preserves `T` as the
member lookup type, including when combined with `std::as_const`.
For `std::forward<T>(value)`, the explicit `T` determines the alias's static
member lookup type and const receiver behavior.
Reference-returning `.get()` calls can also initialize these aliases:
`std::reference_wrapper<T>::get()`, `std::ref(value).get()`, and
`std::cref(value).get()` retain the wrapped object's receiver behavior.
Bindings from `std::optional<T>::value()` or `*optional` retain the selected
value's lvalue and const receiver behavior, including `std::move`,
`std::as_const`, and `std::forward<T>` wrappers around the selected value.
Bindings from `*std::unique_ptr<T>` or `*std::shared_ptr<T>` retain the
pointee's lvalue and const receiver behavior.
Standard local wrappers follow their established access operations too:
`std::unique_ptr<T>` and `std::shared_ptr<T>` resolve through `->`, `.get()`,
and dereference; `std::reference_wrapper<T>::get()` and
`std::ref(value).get()` resolve as `T`, while `std::cref(value).get()` and
`std::ref(std::as_const(value)).get()` resolve as `const T`; and
`std::optional<T>` resolves through `->`, `.value()`, and
dereference while preserving the selected value category. Direct `auto`
constructions of these standard wrappers, and `auto` bindings from
`std::ref` or `std::cref`, retain the same receiver behavior.
`std::expected<T, E>` follows the same selected-value receiver behavior
through `->`, `.value()`, and dereference, including const and rvalue wrappers
and direct `auto` construction. Its `.error()` accessor resolves against `E`
with the error object's own const and value category; references bound from it
retain the same behavior. `std::expected<T, std::unique_ptr<U>>` and
`std::expected<T, std::shared_ptr<U>>` also resolve `.error()->member()`
against `U`.
`std::weak_ptr<T>::lock()` resolves through the returned shared pointer, both
for direct `lock()->member()` calls and `auto` bindings. Const qualification on
the weak pointer wrapper does not change the pointee type.
Direct `std::get<N>(tuple_like)` calls resolve member calls on supported
`std::tuple`, `std::pair`, and `std::variant` elements. The analyzer preserves
the container expression's const and value category through `std::move`,
`std::as_const`, and `std::forward`, including `.value()` / `.error()` on
selected `std::optional` and `std::expected` elements; `operator->` continues
to model the pointed-to object as an lvalue. Type-based `std::get<T>` follows
the same rules only when `T` identifies exactly one top-level element, avoiding
false edges for invalid or ambiguous tuple-like calls.
The supported composition
`std::optional<std::unique_ptr<T>>` or `std::optional<std::shared_ptr<T>>`
also resolves `(*current)->member()` and `current.value()->member()` against
`T`.
Braced local initializers such as `api::Counter counter{value}` and
`api::Box<int> box{value}` also resolve to constructor overloads by argument
count. Indexed `using` and `typedef` aliases declared earlier in the same
source file or in a local header included before the caller, such as
`using Alias = api::Counter; Alias counter{value};` or
`typedef api::Counter CounterAlias;`, resolve to the aliased constructor;
alias chains are expanded transitively. Template aliases such as
`template <typename T> using BoxAlias = api::Box<T>;` resolve to the primary
template constructor. Top-level `const` and `volatile` qualifiers are ignored
for construction lookup; pointer and reference aliases do not create
constructor dependencies. For conditional local includes, static analysis
follows only branches with literal `#if 0` or `#if 1` conditions and leaves
macro-dependent branches unresolved.
Direct qualified calls also expand indexed namespace aliases, so
`namespace vendor = detail;` lets `vendor::convert(value)` resolve to
`detail::convert` before overload filtering, including chained aliases.
Qualified calls through `using api::function;` declarations resolve to
the imported callables rather than the declaration symbols themselves; local
and imported overloads remain part of the same argument-count-filtered set.
Both qualified alias and `using` resolution require a declaration before the
caller in the same source file or in a local header included before it.
Unqualified direct calls also resolve through scoped `using api::function;`
declarations before global fallback candidates are considered, including
declarations from local headers included before the caller.
Direct unqualified C++ calls also honor `using namespace vendor;` imports from
the enclosing namespace scopes before falling back to global candidates, including
namespace-alias targets such as `using namespace alias;` when the alias is
declared earlier in the same source file.
Basic operator and conversion methods use paths such as `Class::operator+` and
`Class::operator bool`; their callable IDs use the same signature convention.
C++ `using` aliases and declarations are indexed with their enclosing namespace
and class scope, for example `api::Size`, `api::Config::Count`, and
`api::convert` for `using vendor::convert;` inside `namespace api`.
Multiple declarations of one overload retain that overload's shared
`symbol_id`; different parameter types remain separate symbols.
Namespace aliases are indexed at their definition scope, for example
`api::vendor` for `namespace vendor = third_party::vendor;` inside `namespace api`.
C++20 concept definitions are indexed by qualified name, such as
`api::Incrementable`.
Named enum definitions and members are indexed with namespace and enclosing-class
scope. Scoped members use paths such as `api::Status::ready` and
`api::Task::State::queued`; non-scoped members use their enclosing namespace or
class scope, such as `api::pending` and `api::Task::paused`.
Named struct and union definitions are indexed with the same namespace and
enclosing-type scope, such as `api::Counter` and `api::Counter::Storage`.
Named C definitions such as `struct Packet { ... };` and `union Payload { ... };`,
along with C enum members such as `STATUS_READY`, are also available as patch and
trace targets without requiring a `typedef`.
C++ anonymous-namespace members have file-anchored identities, preventing
same-name symbols in separate translation units from being merged in traces.
Functions declared or defined through `extern "C"` linkage specifications are
indexed, traceable, query-ownable, and patchable like ordinary free functions.
Declarations in `#if`/`#else` branches are indexed without evaluating
preprocessor conditions, including class methods in conditional branches.
Inline friend functions, including function templates, are indexed in their
enclosing namespace, so a friend inside `api::Token` has a path such as
`api::inspect` rather than a class method path.
Explicit class and function template instantiations are indexed with specialized
paths such as `api::Vector<int>` and `api::increment<int>`.

## Read And Discovery Tools

`batch` runs up to 32 read-only Arborist calls in order and accepts an optional
shared `timeout_ms` budget capped at `300000` milliseconds. Before execution,
the gateway validates every inner call's structure and any explicit inner
timeout. Every batch-eligible tool accepts a cooperative timeout, so the gateway
forwards the smaller of the caller's explicit inner timeout and the batch's
remaining budget, or injects the remaining budget when none was supplied. A
single blocking step inside an inner tool remains non-preemptible. Expiration
fails the whole batch without returning partial results, and input argument
objects are not modified.

`get_semantic_skeleton` returns both `available_paths` and
`available_symbols`. Each symbol includes stable `symbol_id`, `semantic_path`,
optional `scope_path`, `node_kind`, `byte_range`, structured `parameters`,
optional `return_type`, and optional `signature` / `docstring`.
Its optional `depth_limit` defaults to `2` and is capped at `64`; use
`expand_nodes` to include selected deeper symbols. An optional cooperative
`timeout_ms` budget capped at `300000` milliseconds spans file-backed source
reads, parsing boundaries, Python query iteration, C/C++ symbol collection,
skeleton rendering, and result validation. A single blocking source read or
parse remains non-preemptible.

`execute_tree_query` runs raw Tree-sitter queries and returns optional
`owner_symbol_id`, `owner_semantic_path`, and `owner_scope_path` fields when a
capture belongs to a semantic symbol. Results are bounded by `max_captures`
(default `10000`) so broad arbitrary queries fail closed instead of returning
unbounded capture sets. `max_captures` is capped at `100000`, Tree-sitter match
expansion is capped internally, and queries use a cooperative `timeout_ms`
budget capped at `300000` milliseconds. Omitting `timeout_ms` preserves the
default `500ms` budget. The budget can stop source parsing, Tree-sitter progress,
and capture collection cooperatively; a native call already in progress returns
when its parser or cursor progress callback is next invoked. Query text is also
capped at 64 KiB before compilation,
which keeps accidental or adversarial raw
Tree-sitter queries from consuming unbounded parser resources. Its MCP
`outputSchema` describes each capture field explicitly, including byte ranges
and start/end points.

`read_symbol` and `read_symbol_at_position` bridge discovery and action by
returning structured symbol metadata plus the exact source snippet and start/end
points. They and the context/discovery variants described below accept an
optional cooperative `timeout_ms` budget capped at `300000` milliseconds.

The `list_symbols*` and `search_symbols*` families use the same structured
symbol shape as skeleton, trace, and patch flows. All four `list_symbols*`
tools, the base `search_symbols` tool, `search_symbols_context`,
`search_symbols_neighborhood_context`, and `search_symbols_discovery_context`
accept an optional cooperative `timeout_ms` budget capped at `300000`
milliseconds; omitting it preserves the existing behavior. Search matches are
case-insensitive and can include matched-field metadata for ranking.

## Source Overlays

One-shot skeleton, query, patch, trace-context, and position-based read/trace
requests can analyze an optional `source` buffer without writing it to disk.

Selector-based symbol reads, graph reads, list, and search families also accept
one-shot unsaved `source` overlays when callers provide the workspace
`file_path` that buffer should replace.

When `index_db_path` is supplied with a source overlay, Arborist resolves against
the persisted index plus the in-memory replacement for that one anchored file.
The overlay file must be inside the indexed workspace; out-of-workspace paths
are rejected rather than silently ignored.
When `index_db_path` is omitted, Arborist resolves against the live workspace and
active VFS buffers.

Explicit source overlays must name a supported source file outside ignored
workspace directories (such as `.venv` or `node_modules`). Invalid overlay paths
are rejected rather than silently omitted. VFS buffers in those locations remain
excluded from workspace analysis.

Use the VFS methods (`did_open`, `did_change`, `did_close`,
`list_virtual_files`, `read_virtual_file`, `patch_virtual_ast_node`,
`patch_virtual_ast_node_at_position`, `commit_virtual_file`, and
`discard_virtual_file`) when the caller wants a longer-lived editor session.
Snapshot and list-status outputs have precise MCP schemas for file path, source,
dirty state, version, and syntax error counts.

`did_open`, `read_virtual_file`, and `list_virtual_files` accept an optional
cooperative `timeout_ms` budget capped at `300000` milliseconds. Open and read
cover path validation, source loading, disk reads, parsing, clean-buffer refresh,
syntax-result construction, and final result validation. If they fail or time
out, Arborist restores the exact prior entry, including its source, tree, dirty
state, and version, or removes an entry loaded only for that failed request.
Listing processes loaded paths in deterministic order and covers per-file
refresh, syntax-error collection, sorting, and result validation. A failed timed
listing rolls every refreshed entry back to the pre-request state.

`apply_buffer_edit` and `did_change` accept the same optional budget. The byte
edit path covers source loading and refresh, range and size validation, source
splicing, incremental parsing, syntax collection, result validation, and a final
gate before replacing the entry. `did_change` applies its sequential position
edits under one shared deadline, checking between position resolution and each
staged edit. Any timeout or error restores the exact pre-request entry, including
its tree, dirty state, and version, or removes an entry loaded only for the
failed request. A position-edit batch therefore never leaves a partial update.
Individual source splices, parses, position scans, and syntax-tree traversals
remain non-preemptible.

`commit_virtual_file`, `discard_virtual_file`, and `did_close` accept the same
budget. Commit retains a final gate immediately before persistence. Discard
covers the latest disk-source read and parse plus a final gate before replacing
the virtual entry. `did_close` uses the commit path when `persist=true` and the
discard path otherwise; a timeout leaves the entry open. Once an atomic write or
buffer replacement begins, Arborist performs no later deadline check. A
registered-index synchronization failure after persistence also leaves the clean
entry open with a pending retry instead of losing synchronization state.
Individual blocking reads, parses, tree traversals, writes, and index operations
remain non-preemptible.

## Patch And Preview Tools

`preview_patch_ast_node` and `preview_patch_ast_node_at_position` run the same
semantic patch validation path as normal patching, but they do not write to
disk. They return:

- `patch`: the full patch validation result.
- `unified_diff`: a compact unified diff from original source to preview source.
- `changed`: whether the preview changes source text.

Both single-file preview tools accept an optional cooperative `timeout_ms`
budget capped at `300000` milliseconds. The budget spans file-backed source
reads, semantic or position target resolution, replacement preparation, updated
source parsing, syntax and reference validation, commit-gate evaluation, diff
generation, and result validation. A single blocking source read or parse
remains non-preemptible.

`preview_workspace_position_edits` extends previewing to a batch of up to 32
files. It accepts sequential `PositionEdit` values per file and returns each
updated source, unified diff, and syntax diagnostics without writing any file.
The entire request fails when any position is invalid, so callers never receive
a partial batch preview. Optional per-file `source` values support unsaved
buffers. An optional cooperative `timeout_ms` budget capped at `300000`
milliseconds spans file validation, source reads, edit application, parsing,
diff generation, syntax diagnostics, and result validation; a single blocking
source read or parse remains non-preemptible.

`patch_ast_node` and `patch_ast_node_at_position` perform semantic replacement
with validation. Patch responses include `resolved_symbol_id`, `resolved_path`,
`updated_source`, and `validation`.

Both write tools, plus `patch_virtual_ast_node` and
`patch_virtual_ast_node_at_position`, accept an optional cooperative
`timeout_ms` budget capped at `300000` milliseconds. It spans path and source
setup, semantic or position target resolution, replacement preparation, updated
source parsing, syntax/reference validation, commit-gate evaluation, result
validation, and the VFS edit up to the final pre-write gate. File-backed write
tools preserve an already-open dirty VFS buffer and restore that exact entry if
the budget expires before persistence; blocked patches also leave both the
buffer and disk unchanged. Once the atomic source write starts, Arborist no
longer returns a timeout for that request, avoiding an "error after successful
write" result. Registered-index synchronization then completes or reports its
own error; a synchronization failure leaves the persisted source marked for a
later retry. A single blocking source read or parse remains non-preemptible.

For Python, repeated definitions that share one semantic path receive distinct
IDs. A standard `typing.overload` group uses IDs such as
`/repo/lokdb.py::LokDB.get#overload[1]`,
`/repo/lokdb.py::LokDB.get#overload[2]`, and
`/repo/lokdb.py::LokDB.get#implementation`. Decorators written as
`@overload`, `@typing.overload`, or `@typing_extensions.overload` are recognized;
`import typing as t` and `import typing_extensions as te` also enable
`@t.overload` and `@te.overload`. Direct overload imports and module aliases are
tracked in source order through top-level control-flow bodies and the enclosing
class/function scopes, so `typing` and `typing_extensions` wildcard imports, later loop
targets, assignments, deletes, match captures, parameters, and ordinary imports can
invalidate them without retroactively changing earlier definitions. Nested functions
inherit aliases from enclosing function scopes, and `global`/`nonlocal` declarations
preserve the corresponding rebinding behavior. Wildcard imports from unknown modules
conservatively invalidate the bare
`overload` name, and rebinding it also invalidates later `@overload` decorators. Arbitrary
qualified names such as `@custom.overload` are not treated as standard overloads. Read,
trace, expansion, and patch callers may use those exact IDs. A
non-unique semantic path such as `LokDB.get` is rejected with the candidate
IDs instead of silently selecting the first declaration. Indexes created before
this identity behavior should be rebuilt before using exact Python overload
IDs. Incremental refreshes rewrite every affected file when a cross-file
collision changes its ID.

For C, patch selectors may be a plain name such as `helper` or a precise
`symbol_id` such as `E:/repo/include/zeta.h::helper`. When a file contains both
a forward declaration and a definition for the same symbol, Arborist prefers the
definition by default.

For C++, selectors may use a qualified overload-set path such as
`api::convert` for compatibility, or an exact callable `symbol_id` such as
`api::convert(double)` to target one overload deterministically. Patch results
keep `resolved_path` as the semantic path and return the exact identity in
`resolved_symbol_id`.

Patch validation reports:

- `resolved_identifiers`
- `ambiguous_identifiers`
- `binding_decisions`
- `commit_gate`
- `evidence_invariants`

`commit_gate` records whether the patch was allowed, rejected, or allowed only
through an explicit bypass. Bypass reasons must be nonblank.

## Trace And Context Tools

`trace_symbol_graph` accepts either a plain semantic path such as `orchestrate`
or a precise `symbol_id` when duplicate C globals or C++ overloads need exact
targeting. It returns the traced symbol, callers, callees, and `evidence_keys`.

`trace_symbol_neighborhood` expands a trace into a bounded graph. Callers can
control `direction`, `max_depth`, and `max_nodes`; `truncated` indicates the
bounded expansion omitted reachable symbols. `max_depth` is capped at `64`, and
`max_nodes` is capped at `10000` across trace, context, and patch-impact tools.
The four direct trace tools also accept an optional `timeout_ms` cooperative
budget for symbol selection, graph summarization, and neighborhood expansion,
capped at `300000` milliseconds. The budget is checked while scanning candidate
symbols and between expansion phases and BFS edges; index loading, source parsing,
and a single blocking operation remain
non-preemptible.

`read_symbol_context`, `read_symbol_neighborhood_context`, and
`read_symbol_discovery_context` combine source reads with trace and neighborhood
data to reduce multi-call orchestration. All eight direct read tools, including
their position variants, accept the same optional `timeout_ms` budget as
neighborhood traces. The budget covers workspace or persisted-index loading,
symbol or position resolution, source reads, bounded graph expansion, and
per-node source reads where applicable; a single blocking source or overlay
parse remains a non-preemptible boundary.

`validate_patch_with_trace_context` runs patch validation, traces the patched
symbol with the updated file held in memory, and returns the trace-backed
validation decision in one response. It and its position variant accept an
optional cooperative `timeout_ms` budget capped at `300000` milliseconds. The
budget spans file or overlay setup, patch validation, baseline and updated trace
queries, impact calculation, and result validation; a single blocking source
read or parse remains non-preemptible. If syntax validation or the patch gate
rejects first, tracing is skipped and `trace_error` explains why.

Successful live-workspace and persisted-index trace-backed patch results also
include `impact`: direct callers/callees added or removed by the proposed
change, plus a distinct affected-symbol count. It is a one-hop comparison, not
a transitive impact analysis; callers should use the neighborhood variants when
they need bounded multi-hop context. `impact` is `null` when tracing is skipped
or when a VFS-backed operation cannot retain a pre-patch trace baseline.

The graph, neighborhood, and discovery context variants and their position
forms accept the same optional `timeout_ms` budget. It covers file or overlay
setup, patch validation, the updated trace, bounded graph or source-context
expansion, and result validation; a single blocking source read or parse remains
non-preemptible. These variants add bounded impact analysis and aligned source
snippets for reachable symbols. `*_at_position` variants resolve the target from
`file_path + position` before running the same workflow.

`replay_patch_evidence_against_trace` compares patch evidence invariants against
trace graph evidence. `validate_patch_commit_with_trace` turns that replay into
a single allowed/status/reason decision. Both accept an optional cooperative
`timeout_ms` budget capped at `300000` milliseconds. The shared native budget
covers patch and trace validation boundaries, updated-source parsing boundaries,
syntax-tree traversal, evidence-key collection and normalization, invariant
replay, status summarization, and final result validation. The strict JSON decode,
a single source parse or model-validation call, and result serialization remain
non-preemptible.

## Symbol Index Tools

`rebuild_symbol_index` creates a missing persisted SQLite symbol index or
rebuilds an existing valid Arborist index for the same workspace. Existing
non-index databases, incomplete schemas, unsupported schema versions, and
indexes from other workspaces are rejected before any schema initialization or
rewrite. `refresh_symbol_index` incrementally synchronizes the complete
workspace: unchanged files are reused by fingerprint, changed and new files are
reparsed, and deleted files are removed. It is the preferred operation for
polling or watch integrations. `refresh_symbol_index_for_file` reparses one
changed file, removes deleted file state when needed, reuses stored symbols for
unchanged files, and persists a partial SQLite update. Workspace scans are
bounded by `max_files` (default `20000`) on rebuilds and missing-index refresh
fallbacks so unexpectedly large workspaces fail with an actionable limit error
instead of scanning without bound. Rebuild and refresh calls can also provide
`max_file_bytes` to reject oversized source files before indexing reads them;
this optional limit is capped at `67108864`. Index registration, rebuild,
and refresh `timeout_ms` values add an optional cooperative budget for directory
traversal and per-file indexing, capped at `300000` milliseconds.
`list_symbol_indexes` and `unregister_symbol_index` accept the same cap. Listing
checks it while collecting entries, around deterministic sorting, and during
result validation. Unregister checks after path normalization immediately before
removing the registration, so a pre-mutation timeout preserves the entry and no
late check can misreport a completed removal. `max_files` is
capped at `200000`; symbol list/search `limit` values are capped at `10000`. When
a scan budget expires, the operation returns an error before persisting a new
index snapshot.

`arborist-index-watch` is a polling console command for one index database or a
JSON manifest of multiple registered workspace/index pairs. `--version` reports
the installed Arborist package version without requiring a watch target. It uses
`inspect_symbol_index` between refreshes, so healthy indexes do not incur
SQLite writes. `--once` performs one inspect-and-reconcile pass for CI or a
supervisor probe. The command refreshes only a missing index or a current-schema
index with freshness issues, and migrates supported v1-v3 indexes in place;
foreign, incomplete, and unknown schemas are reported and left unchanged.
`--dry-run` follows the same inspection and fail-closed decisions but reports
`would_refresh` or `would_migrate` without changing an index.
`--check` runs this no-write pass once and exits nonzero when any target is not
healthy, while emitting each target's status for CI diagnostics. Watch event
health summaries include issue, stale, missing, unreadable, and unindexed file counts.
`inspect_symbol_index`, `migrate_symbol_index`, and the watch command accept
the optional cooperative `timeout_ms` / `--timeout-ms` budget. Inspection uses
it for indexed-file freshness reads and the unindexed workspace scan; watch
reconciliation forwards the same budget to inspection, migration preflight, or
refresh as applicable.
Manifest paths are resolved relative to the manifest file, targets are ordered
by workspace root, and unknown fields, duplicate keys, empty target lists,
duplicate workspace roots, or duplicate database paths are rejected before the
first refresh.

`register_symbol_index`, `unregister_symbol_index`, and `list_symbol_indexes`
manage session-scoped index registrations. Registered indexes are refreshed when
a committed file belongs to that workspace. `refresh_registered_symbol_indexes`
polls every registered workspace using the same fingerprint-based incremental
refresh path, so clients can reconcile externally changed files without
repeating registration or managing database paths themselves. It returns one
refresh statistic object per registered index in deterministic workspace order.

`inspect_symbol_index` is read-only. It reports whether an index exists, whether
its schema and metadata are healthy, the response schema version, the expected
index schema version, a machine-readable migration recommendation, the stored
workspace root, indexed file/symbol counts, file-state row count, fresh indexed
file count, stale indexed files whose fingerprints no longer match disk,
missing indexed files, unreadable indexed files, source files that are not yet
indexed, and diagnostic issues. Persisted index queries fail closed when a new
workspace source file has not been indexed, preventing silently incomplete
search and trace results. The
migration recommendation is intentionally advisory: Arborist does not rewrite
unrecognized SQLite databases during inspection.

`migrate_symbol_index` applies the migration only when inspection recommends
`action: "migrate"`. The current v1-v3-to-v4 migration recreates the symbols
table with a `(symbol_id, file_path, start_byte, end_byte)` primary key, creates
the `symbols(file_path)` index used by partial file refreshes, then reparses the
indexed workspace so persisted direct-call arity metadata and graph edges match
the current sources. It updates `schema_version` in one SQLite transaction. It
rejects missing databases,
foreign or incomplete schemas, missing required metadata, current indexes, and
unknown versions without rewriting them. Its optional cooperative `timeout_ms`
budget, capped at `300000`, covers path and database setup, schema and workspace
metadata checks, legacy file-state and symbol-row loading, persisted-path
validation, and a final deadline gate immediately before the first schema
mutation. A timeout through that gate leaves the database unchanged. Once the
schema transaction starts, Arborist performs no further deadline checks: it
finishes the required source rebuild and final health inspection and returns
their actual outcome instead of reporting a timeout after the database may have
changed. Individual SQLite queries, source reads, the schema transaction, and
rebuild persistence remain non-preemptible. The result is the same complete
health report returned by `inspect_symbol_index` after the attempted migration.

`export_patch_diagnostics_sarif` converts a prior `patch_ast_node` result into
a SARIF 2.1.0 log for CI systems. Syntax issues retain UTF-8 byte-column source
locations; unresolved or ambiguous bindings and non-allowed commit-gate states
are emitted as Arborist rules in the SARIF run. It accepts the same optional
`timeout_ms` cap for patch validation, updated-source parse boundaries,
syntax-tree traversal, diagnostic collection, and final SARIF construction.
Strict JSON decoding, a single parse or model-validation call, artifact-URI
encoding, and result serialization remain non-preemptible.

Persisted trace reads and single-file refreshes fail closed on missing indexes,
non-index databases, incomplete schema, missing or unsupported schema versions,
metadata issues, indexed-file count mismatches, incompatible column types,
damaged symbol identity fields, persisted paths outside the indexed workspace,
unsupported persisted source paths, invalid byte ranges, invalid JSON
graph/list columns, or empty persisted file-state paths. These checks avoid
silently initializing or partially
migrating unrelated SQLite databases. Inspection and persisted query loading
use read-only SQLite connections; schema creation and migration helpers are
restricted to explicit index write paths.

## C Graph Behavior

C symbol graphs tolerate header declarations plus source definitions sharing the
same semantic path, including uppercase `.H`/`.C` and `.HPP` families. Duplicate
globals keep distinct file-backed `symbol_id` values.

C patch validation follows local `#include` chains when checking accessible
symbols. Ambiguous C bindings include visible include-family context and exact
candidate `symbol_id` hints.

File-local C `static` symbols get file-qualified semantic paths so cross-file
traces do not collapse them together.
