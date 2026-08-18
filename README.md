# Arborist MCP

Arborist MCP is a mixed Rust + Python workspace for semantic code analysis,
patch validation, persisted symbol indexing, and a lightweight stdio MCP
gateway.

Current layers:

- `crates/arborist-core`: Rust parsing core with Tree-sitter based semantic
  extraction, symbol graph indexing, patch validation, VFS state, and SQLite
  persistence.
- `crates/arborist-py`: PyO3 bridge that exposes the Rust core to Python as
  `_arborist_core`.
- `python/arborist_mcp`: MCP-compatible JSON-RPC gateway over stdio.

## Documentation

- [Development guide](docs/development.md): setup, checks, CI profiles, test
  suites, build artifacts, and common failures.
- [Protocol guide](docs/protocol.md): MCP usage, legacy JSON-RPC compatibility,
  tool catalog generation, and protocol validation.
- [Tool guide](docs/tools.md): supported tool families, source overlays, patch
  preview, symbol indexes, trace/context workflows, and C/C++ status.
- [Multi-language support design](docs/multi-language-support-design.md):
  proposed adapter architecture, capability model, migration plan, and language
  expansion roadmap.
- [Generated tool catalog](docs/tool-catalog.json): exact `tools/list` snapshot.

## Language Support

Arborist uses extension-based routing with explicit per-language capabilities:

- Python: `.py`, `.pyi` — semantic skeletons, indexing, tracing, patching, and queries.
- C grammar: `.c`, `.h` — semantic skeletons, indexing, tracing, patching, and queries.
- C++ grammar: `.cc`, `.cpp`, `.cxx`, `.c++`, `.tpp`, `.tcc`, `.ipp`, `.inl`,
  `.hpp`, `.hh`, `.hxx`, `.h++` — semantic skeletons, indexing, tracing, patching, and queries.
- JavaScript: `.js`, `.jsx`, `.mjs`, `.cjs` — semantic skeletons, indexing,
  conservative direct-call tracing through local named imports, named re-export chains,
  default imports and namespace `default` members that name a module's default export or
  its CommonJS export-object default member (object-literal `{ default: ... }` entries and
  spread re-exports), namespace member calls that
  follow the bound module's named and star re-export chains and default exports,
  namespace-object calls that resolve CommonJS callable exports, CommonJS
  constructor calls through `new` that resolve local, named-import,
  default-import, namespace-member, and CommonJS class/function exports,
  `require` bindings (including destructured members with default values) and TypeScript `import name = require(...)` bindings, object-literal and member-assignment export members,
  module-valued export members that alias another module's export object or a named member,
  inline `require(...)` member and namespace-object calls,
  object-literal spread re-exports (`module.exports = { ...require(...) }`),
  CommonJS interop default exports, `module.exports` replacement shadowing of `exports` alias members,
  wholesale `module.exports = require(...)` re-export chains, star re-export chains,
  static local dependency refresh, structural patching, and
  queries.
- TypeScript: `.ts`, `.mts`, `.cts`; TSX: `.tsx` — the same initial capabilities as
  JavaScript.
- Rust: `.rs` — Tree-sitter parsing, raw queries, semantic skeletons, declaration indexing,
  and conservative local-module dependency refresh through unambiguous out-of-line `mod`
  declarations. It also provides conservative graph tracing for unshadowed bare direct calls to
  functions in the same source-file module, qualified direct calls to functions in inline modules in
  the same source file, and `module::function()` or `crate::module::function()` calls through one
  chain of source-file-root out-of-line `mod module;` declarations. Each module in the chain must use
  one unambiguous default-layout file (`module.rs` or `module/mod.rs`); the terminal file must contain
  one matching top-level function. It also traces unshadowed bare calls through exact source-file-root
  `use crate::module::function;`, `use self::module::function;`, or exact `use super::...` bindings, including grouped
  `use` paths and explicit `as` aliases when their targets are reachable through the unique out-of-line
  parent/module chain. Crate-root imports from out-of-line children and repeated `super::` ancestor
  navigation are supported. Equivalent qualified `crate::...` and `super::...` calls from out-of-line
  children use the same conservative parent/module chain. Malformed source, `#[path]` semantics,
  duplicate declarations/import aliases, ambiguous layouts, and ambiguous parent chains fail closed; wildcard imports are not considered. Trait-implementation members are not
  indexed, and inline-module, Cargo, and import resolution beyond those exact bindings remains unavailable.
  Patching remains explicitly unavailable.
- Go: `.go` — Tree-sitter parsing, raw queries, semantic skeletons, and conservative declaration
  indexing for named type specifications and aliases, functions, and methods with named local
  receiver types, selected by semantic path or source position. Static imports strictly below the
  nearest valid simple `go.mod` module path refresh the importing file when a direct `.go` file in
  the imported package directory changes. It also traces unshadowed bare direct calls to top-level
  functions declared in the same source file or in one matching production source in the same directory and
  package, plus unambiguous direct calls through local package imports using an explicit alias or the imported
  package's declared name, and calls through a named composite literal such as `Counter{}.Value()`, `(&Counter{}).Value()`, or `Box[int]{}.Value()` to one matching
  production method in the same local package; a direct named type-conversion receiver such as `Scalar(value).Value()`, `(*Scalar)(value).Value()`, `(Scalar)(value).Value()`, or `Box[int](value).Value()` when its base type is one unique same-package production `type` specification; a direct named type-assertion receiver such as `value.(Scalar).Value()` when `Scalar` is one unique same-package production `type` specification; a simple local alias receiver such as `type Alias = Counter; Alias{}.Value()`, `Alias(value).Value()`, or `value.(Alias).Value()` when its alias chain reaches one unique same-package production named `type` declaration without a cycle; or an unshadowed named local receiver, local-type parameter, or directly declared function-body local variable. If the conversion-shaped receiver name has no matching local type specification, it retains a direct factory-function dependency rather than guessing a method target. Same-package production sources refresh conservatively as a group. Module-root imports, external modules,
  `replace`, `go.work`, vendoring, build tags, general cross-file/package/import resolution, qualified imported conversions, interface dispatch, other method dispatch,
  and patching remain unavailable or capability-gated.
- C#: `.cs` — Tree-sitter parsing, raw queries, semantic skeletons, declaration indexing, and
  conservative tracing of unshadowed unqualified calls, explicit `this.` method calls, inherited bare/explicit-`this.` instance calls through a unique class/record ancestor chain, and `: this(...)` constructor initializers,
  and conservative `base(...)` constructor initializers and `base.Method()` calls through a unique class/record ancestor chain with simple or generic, unshadowed qualified, `global::`, local, or root-level global type-alias/namespace-import base types,
  globally namespace-qualified `global::...` static calls, simple same-namespace and enclosing-namespace
  `Type.Method()` static calls, including balanced generic type spellings for direct receivers, type aliases, and static imports such as `Outer<int>.Method()`, `using Alias = Outer<int>; Alias.Method()`, or `using static Outer<int>; Method()` that normalize to their declaration paths without type-argument selection, plus `Outer.Helper.Method()` or `AliasOuter.Helper.Method()` static calls when the outer type resolves uniquely in the caller's namespace, an enclosing namespace, or an imported namespace, or a type alias resolves uniquely in those scopes or a root-level global using, and every nested type is unique, including from nested source types, explicit type-alias calls of the form `Alias.Method()`,
  `using static Fully.Qualified.Type;` bare method calls, root-level `global using static Fully.Qualified.Type;`
  bare method calls contributed by any scanned C# source file (including directive-only files), and file-root
  `using Fully.Qualified.Namespace;` calls of the form `Type.Method()`, and root-level `global using Fully.Qualified.Namespace;`
  calls contributed by any scanned C# source file (including directive-only files), and root-level `global using Alias = Fully.Qualified.Type;`
  calls contributed by any scanned C# source file (including directive-only files). Type aliases, static imports, and namespace imports may appear at file root or
  directly in a block/file-scoped namespace; aliases are resolved from the caller's namespace through enclosing
  namespaces to file root, with the nearest binding taking precedence, while static/ordinary imports from each
  of those scopes are considered.
  Same-type unqualified/`this.` forms require a unique same-file, exact-arity, non-`params` declaration. When no same-type method name is indexed, a non-static caller may resolve a bare or explicit `this.` call through the same unique class/record ancestor chain used by `base.Method()`. Globally
  qualified, simple same-namespace, and imported static calls may resolve one unique workspace declaration. Imported
  targets must resolve to one indexed type. Duplicate aliases within the same scope, duplicate imports, same-file
  type-name collisions, competing imported targets, and ambiguous type declarations fail closed. Static imports are
  considered only when no same-type method has the bare call's name; namespace imports are considered only when the
  caller's namespace has no type of the receiver name anywhere in the workspace. Simple type receivers must not be
  shadowed by a local, parameter, type parameter, or type member. Block and file-scoped namespaces, classes, structs,
  interfaces, enums, records, methods, and constructors are supported. When any C# source file changes, refresh
  conservatively re-resolves every indexed C# symbol against all tracked C# sources, including directive-only global-using files;
  unchanged C# source files are not reindexed; a refreshed source that has changed may safely shrink because its stale byte ranges do not block validation before the rebuilt index replaces them. Structural patch targeting covers classes, structs, interfaces, enums, records, methods, and constructors by semantic path or source position with syntax-level validation; language-specific C# patch binding validation remains deferred. Outer-namespace alias/import inheritance, other member dispatch, and overload type selection remain explicitly
  unavailable until dedicated C# adapter slices establish their contracts and fixtures.
- Kotlin: `.kt`, `.kts` — Tree-sitter parsing, raw query execution, package-qualified semantic skeletons, and declaration indexing for top-level and nested classes, interfaces, enums, named objects, functions, simple properties, and type aliases. Local declarations inside function bodies are intentionally omitted because they have no stable file-level semantic path. Kotlin now refreshes unique explicit import dependents against `.kt` files that declare the imported package and traces unqualified direct calls to enclosing-type functions, same-package top-level functions, or unique explicitly imported top-level functions from other packages, and traces qualified receiver calls whose receiver type is pinned to a local class, interface, or type alias by a constructor initializer, explicit type annotation, parameter type, or enclosing-class property; named-object receivers such as `Config.helper(...)` dispatch to the object's members when the name resolves to a uniquely declared same-package or explicitly imported object, class-name receivers such as `Config.helper(...)` dispatch to companion-object members when the name resolves to a uniquely declared class or interface and the member is declared in its companion object while instance members and unknown companion members fail closed, explicit companion chains such as `Config.Companion.helper(...)` dispatch directly to companion members when the class name resolves to a uniquely declared type and no local binding shadows it while instance members, unknown companion members, and extension fallbacks fail closed, named companion objects such as `companion object Factory` are indexed under their declared name so calls such as `Config.Factory.helper(...)` or `Config.Factory.holder.run(...)` dispatch through the same canonical companion scope as the `Companion` spelling while unknown companion names and companion chains rooted at object declarations fail closed, companion property chains such as `Config.Companion.holder.run(...)` resolve each intermediate property's declared type within the companion scope before dispatching the final member or extension while chains through instance properties fail closed, nested companion receivers such as `Outer.Inner.helper(...)`, `Outer.Inner.Companion.helper(...)`, or `Outer.Inner.Factory.holder.run(...)` dispatch through the canonical companion scope of a nested class or interface when the first hop resolves to a uniquely declared type and the second hop names exactly one nested class or interface that hosts a companion while nested types without companions, unknown or ambiguous nested types, and locally shadowed outer names fail closed, and chained receiver calls such as `group.member.helper(...)` or `Config.holder.run(...)` additionally resolve each intermediate property's declared type, falling back to a bare-identifier constructor initializer such as `val member = Other()` when the property has no explicit type, or to the declared return type of a uniquely resolved same-file, same-package, or explicitly imported initializer function such as `val member = makeOther()`, a dotted factory return type such as `fun makeInner(): Outer.Inner` pins a nested receiver through the same dotted type-path rules while missing nested targets and factories without a declared return type fail closed, and a local receiver binding such as `val other = makeOther()` resolves the same way through the factory's declared return type, with the first hop either locally bound or a named object, before dispatching the final member or extension, and when the pinned type has no matching member, an unambiguous top-level extension function for that receiver type declared in the same file, the same package, or explicitly imported resolves the call, while member functions shadow extensions and nullable receivers, generic property types, qualified non-constructor initializers, function-call initializers whose function has no declared return type or is unknown or ambiguous, unknown chain hops, and ambiguous targets fail closed, and bare calls to constructible class names such as `Other(...)` resolve to the class declaration through the same scope/import rules, qualified nested constructors such as `Outer.Inner(...)` and nested receiver paths such as `Outer.Inner` in local bindings, property initializers, declared property types, or parameter types resolve through the same dotted type-path rules, dotted type-alias targets such as `typealias Helper = Outer.Inner` expand through the same rules while missing nested targets and cyclic alias chains fail closed, nested object receivers such as `Outer.Inner.helper(...)` dispatch to the nested object's members when the first hop resolves to a uniquely declared class or object and the second hop names exactly one nested object declaration while unknown or ambiguous nested objects, nested classes or interfaces that share a nested object's name, and locally shadowed outer names fail closed, and constructor-call receivers such as `Outer.Inner().helper(...)` or `Group().member.helper(...)` resolve the constructed type path through the same constructible-class rules and then dispatch the member chain like any other instance receiver while function-call bases such as `makeOther().helper(...)`, unknown or missing types, and non-constructible bases fail closed, and interfaces, enums, and sealed/abstract/annotation/inner classes, unknown or missing nested types, and nested type aliases fail closed. It still makes no Java/JVM source-linkage assumptions. Structural patch targeting covers classes, interfaces, enums, named objects, functions, simple properties, companion objects, and type aliases by semantic path or source position with syntax-level validation; language-specific Kotlin patch binding validation remains deferred.
- Java: `.java` — Tree-sitter parsing, raw queries, semantic skeletons, declaration indexing, and
  conservative dependency refresh for explicit local type imports (including nested type imports such as
  `import com.example.Outer.Inner;`), single-member `import static`
  imports (including nested static-member imports such as `import static com.example.Outer.Inner.method;`), direct superclass links whose base resolves from the same package, a unique explicit local type import, or an exact qualified local source spelling, and direct interface links whose interface resolves by the same local-source rules. Those links require an owning type that resolves to a local
  `.java` file under an ancestor source root. Classes,
  interfaces, enums, annotation types, fields, methods, and constructors are indexed by package-qualified
  paths. It traces an explicit `this(...)` constructor initializer to a single same-type, same-file, non-varargs constructor with a unique arity match; a direct local-source `super(...)` constructor initializer to a unique direct base-class non-varargs constructor with a matching arity; plus unqualified and `this.method()` calls to a single same-type, same-file, non-varargs method with a unique arity match; `Type.method()` calls through a unique explicit
  non-static local type import when the type name is unshadowed; and bare calls through a unique
  explicit local static-method import when no same-type method has that name. It also traces a
  `Type.method()` call from a top-level caller class to a unique same-package top-level class or interface static method with an exact,
  non-varargs arity match, plus `Outer.Helper.method()` through a unique same-package or explicitly imported outer type and nested class. It also traces `receiver.method(...)` calls whose leading receiver is a locally bound value (formal parameter, declared local, or enclosing-class field) to a unique non-static, non-varargs instance method with a unique arity match on the receiver's declared class type (generic declared types such as `Box<String>` normalize to the raw base type), resolved from the same package, an explicit local type import, a nested scope, or an exact qualified spelling and walked up a unique local-source superclass chain; a `var` local receiver infers its class type from a constructor initializer such as `var helper = new Helper()`, including dotted nested types such as `new Outer.Inner()`, or from the declared return type of a unique same-file same-type factory method or unique explicit static-method import when the initializer is a bare method call such as `var value = makeFoo()`, or of a unique instance method call on a locally bound receiver such as `var value = group.makeFoo()`, `var value = new Group().makeFoo()`, `var value = this.makeFoo()`, `var value = super.makeFoo()`, or `var value = group.inner().makeFoo()` with each receiver hop resolved through the same member-chain rules (while factory-inferred `var` receiver hops, unbound or static type receivers, and unknown or ambiguous qualified callees fail closed), with the factory return type resolved in the factory's own file and package scope, and then dispatches like any other typed receiver, while constructor-call receivers such as `new Helper().helper(...)` or `new Outer.Inner().helper(...)` dispatch directly on the constructed type path, receivers declared with an interface type resolve to a method directly declared on that interface (abstract or default), or to a method declared on a uniquely resolved direct local super-interface chain when the interface itself does not declare the method, while branching super-interface chains resolve only when exactly one branch provides a uniquely arity-matched method and every other branch proves it has no declaration (a declaration reached identically through multiple branches still resolves once), and competing, ambiguous, cyclic, or unresolvable branches fail closed, and member chains such as `group.member.helper(...)` resolve each intermediate field's declared class or interface type before dispatching the final method, and constructor-call chains such as `new Group().inner.helper(...)` dispatch the same way through the constructed type path, and member chains through zero-argument method-call hops such as `group.inner().helper(...)` or `new Group().inner().helper(...)` resolve each intermediate call's declared return type (generic return types normalize to the raw base type) in the called method's own file and enclosing scope before dispatching the final member, and `this.`-rooted chains such as `this.member.helper(...)` or `this.inner().helper(...)` dispatch on the enclosing type through the same member-chain rules while plain `this.method()` calls keep the same-type contract, and `super.`-rooted chains such as `super.inner().helper(...)` or `super.member.helper(...)` dispatch on the unique local-source direct superclass type path through the same member-chain rules while plain `super.method()` calls keep the direct-base-chain contract. Class-typed receivers whose class and direct superclass chain do not declare the method dispatch a uniquely arity-matched non-varargs `default` method through resolved direct local interfaces when exactly one interface chain or branch provides it and every other chain or branch proves it has no declaration, while competing, unresolved, or ambiguous chains and nearer same-name class declarations fail closed. Bound receivers with unknown, primitive, or varargs declared types, malformed generic or array spellings, non-constructor, non-factory `var` initializers and unresolvable factory initializers (unknown or ambiguous factories, factories without a usable declared return type, or arity mismatches), qualified initializer callees with factory-inferred `var`, unbound, or static type receiver hops, anonymous-class constructor receivers, interface receivers whose interface and direct super-interface chain do not declare the called method, unknown or ambiguous chain hops, unknown `this.`-rooted or `super.`-rooted chain hops, and method-call hops with non-empty argument lists, static hops, or primitive or void return types, and static methods reached through instance references fail closed. Matching callers are re-resolved during refresh without reindexing
  unchanged Java source files. As a deliberately limited interface-dispatch slice, a bare or explicit `this.` call in a class with no explicit `extends` clause, or with one uniquely resolved direct local superclass chain that declares no method of that name, and one or more uniquely resolved direct local interfaces, including lexical outer-scope interface references, trace only when exactly one interface chain or branch provides a directly declared, uniquely arity-matched non-varargs `default` method and every other chain or branch proves it has no declaration of that method. Explicit `super.method()` calls and bare calls without a same-type
  declaration also walk a unique local-source chain of direct base classes, resolved from the same package, a unique explicit local type import, or an exact qualified local source spelling; cycles, ambiguous
  classes, nested/outer generic bases, and nearer nonmatching declarations fail closed. A generic direct base such as `Base<String>` or `com.base.Base<String>` is normalized to its simple or exact qualified base name without type-argument selection. A same-package or explicitly imported outer-qualified direct base such as `Outer.Base` is supported only when one local outer source file and one indexed nested `class_declaration` remain. Wildcard outer imports and broader nested/outer scope semantics beyond this direct source or explicit-import form remain unsupported. Imported targets must be static with a unique exact arity match. Wildcard imports, static wildcard imports, static
  field imports, missing or ambiguous imports, general interface dispatch with competing defaults or abstract declarations, branching or ambiguous interface-inheritance chains, superclass chains that are unresolved, ambiguous, cyclic, or declare the called method name, and broader member dispatch, instance/member dispatch other than explicit simple `super.method()` calls, inherited bare calls across unique local-source base chains, overloaded-call
  selection, and patch operations remain capability-gated.

Python overload groups retain one compatibility `semantic_path` while exposing
unique IDs for each declaration and implementation, such as
`/repo/store.py::Store.get#overload[1]` and
`/repo/store.py::Store.get#implementation`. Arborist recognizes standard
`typing` and `typing_extensions` overload decorators, including directly
imported aliases and module aliases declared before the decorated definition.
Those aliases are tracked in source order through top-level control-flow bodies
and the enclosing class/function scopes, including `typing` and
`typing_extensions` wildcard imports, imports, loop targets, assignments, deletes,
match captures, parameters, and other rebinding events. Nested functions inherit
overload aliases from enclosing function scopes, while `global` and `nonlocal`
declarations keep
rebinding behavior aligned with Python name resolution; wildcard imports
from unknown modules conservatively invalidate the bare `overload` name. Rebinding
the bare `overload` name also invalidates later `@overload` decorators. Arbitrary
decorators such as `custom.overload` are not treated as standard overloads. Non-unique Python
semantic-path selectors are rejected with
candidate IDs rather than silently selecting the first overload. Rebuild
indexes created by older Arborist builds to materialize these identities.
Incremental refreshes rewrite every affected file when a cross-file collision
changes its ID.

C++ files use the dedicated Tree-sitter C++ grammar. C-family indexing,
tracing, query ownership, and patch targets support free functions in named
namespaces plus named methods declared or defined in class bodies, with
qualified semantic paths such as `outer::Class::method`. Class definitions are
also indexed with their namespace and enclosing-class scope. Named class methods
defined outside the class are also matched to their declarations. Explicit
constructors and destructors are supported as `Class::Class` and
`Class::~Class`; defaulted/deleted methods are indexed with their full
declaration signatures. Named function and class-method templates are indexed
and traced with their template declaration text. Explicit function template
specializations have distinct paths such as `increment<int>` and `Box<int>::value`.
Non-type template parameters are treated as local bindings during patch validation
and reference tracing. C++ callable `semantic_path` values remain overload-set
paths, while exact `symbol_id` values include normalized parameter types and
member qualifiers, such as `api::convert(int)`, `api::convert(double)`, and
`api::Counter::value() const`. Basic operator methods use paths such as
`Class::operator+` and `Class::operator bool` with the same exact-ID convention.
C++ graph resolution filters direct function calls by argument count before
choosing an overload; defaulted and variadic parameters are considered when
matching candidates. Namespace-qualified calls such as `api::convert(value)`
are resolved relative to enclosing namespaces before overload filtering.
Explicit template calls such as `convert<int>(value)` prefer an indexed exact
specialization and otherwise fall back to the primary template through the
same direct-call graph path.
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
Direct C++ type constructions such as `Counter(value)`, `Counter{value}`, and
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
Bindings from `std::reference_wrapper<T>::get()`, `std::ref(value).get()`, and
`std::cref(value).get()` retain the wrapped object's receiver behavior.
Bindings from `std::optional<T>::value()` or `*optional` retain the selected
value's lvalue and const receiver behavior, including `std::move`,
`std::as_const`, and `std::forward<T>` wrappers around the selected value.
`std::expected<T, E>` follows the same selected-value receiver behavior
through `->`, `.value()`, and dereference, including const and rvalue wrappers
and direct `auto` construction. Its `.error()` accessor resolves against `E`
with the error object's own const and value category; references bound from it
retain the same behavior. `std::expected<T, std::unique_ptr<U>>` and
`std::expected<T, std::shared_ptr<U>>` also resolve `.error()->member()`
against `U`.
Bindings from `*std::unique_ptr<T>` or `*std::shared_ptr<T>` retain the
pointee's lvalue and const receiver behavior.
`std::weak_ptr<T>::lock()` resolves through the returned shared pointer, both
for direct `lock()->member()` calls and `auto` bindings; const on the weak
pointer wrapper does not make `T` const.
Direct `std::get<N>(tuple_like)` calls resolve member calls on supported
`std::tuple`, `std::pair`, and `std::variant` elements. The analyzer preserves
the container expression's const and value category through `std::move`,
`std::as_const`, and `std::forward`, including `.value()` / `.error()` on
selected `std::optional` and `std::expected` elements; `operator->` continues
to model the pointed-to object as an lvalue. Type-based `std::get<T>` follows
the same rules only when `T` identifies exactly one top-level element, avoiding
false edges for invalid or ambiguous tuple-like calls.
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
Namespace aliases are expanded for direct qualified calls, so an alias such as
`namespace vendor = detail;` resolves `vendor::convert(value)` to `detail`;
alias chains are expanded transitively. Qualified namespace aliases and `using`
declarations must be declared before the caller in the same source file or in a
local header included before it.
Qualified calls through `using api::function;` declarations resolve to
the imported callables rather than the declaration symbols themselves; local
and imported overloads remain part of the same argument-count-filtered set.
Unqualified direct calls also resolve through scoped `using api::function;`
declarations before global fallback candidates are considered, including
declarations from local headers included before the caller.
Direct unqualified C++ calls also honor `using namespace vendor;` imports from
the enclosing namespace scopes before falling back to global candidates, including
namespace-alias targets such as `using namespace alias;` when the alias is
declared earlier in the same source file.
C++ `using` aliases and declarations are indexed with namespace and class scope,
for example `api::Size`, `api::Config::Count`, and `api::convert`. Namespace
aliases are indexed at their definition scope, for example `api::vendor`. See the [tool
guide](docs/tools.md#language-support) for the current scope. C++20 concept
definitions, named enum definitions and members, and named struct/union definitions are
also indexed by qualified name, such as `api::Incrementable`, `api::Status`,
`api::Status::ready`, `api::Counter`, and `api::Counter::Storage`. C definitions such as `struct
Packet { ... };`, `union Payload { ... };`, and named enum members are indexed without a `typedef`
alias. C++ anonymous-namespace members use file-anchored identities so symbols
with the same name in separate translation units remain isolated. `extern "C"`
function declarations and definitions are indexed through their linkage wrapper.
Declarations in `#if`/`#else` branches are also indexed without evaluating
preprocessor conditions.
Inline friend functions, including function templates, are indexed in their
enclosing namespace rather than as class methods.
Explicit class and function template instantiations are indexed with their
specialized paths, such as `api::Vector<int>` and `api::increment<int>`.

## Implemented Tool Families

The MCP catalog currently returns 58 tools:

- Read tools: 29, including batch reads, semantic skeletons, patch previews,
  bounded raw Tree-sitter queries with cooperative timeout budgets, symbol
  reads (including bounded neighborhood reads), symbol list/search, and
  graph-backed read bundles.
- Write tools: 2, `patch_ast_node` and `patch_ast_node_at_position`.
- VFS tools: 10, including open/change/close, virtual patching, byte edits, commit/discard,
  and virtual reads.
- Index tools: 9, covering register, unregister, list, inspect, migrate,
  rebuild, workspace refresh, and file refresh for persisted symbol indexes.
- Trace tools: 8, covering graph/neighborhood traces plus trace-backed replay and validation.

`batch` runs up to 32 read-only Arborist calls in order and accepts an optional
shared `timeout_ms` budget capped at `300000` milliseconds. Before execution,
the gateway validates every inner call's structure and any explicit inner
timeout. Every batch-eligible tool accepts a cooperative timeout, so the gateway
forwards the smaller of the caller's explicit inner timeout and the batch's
remaining budget, or injects the remaining budget when none was supplied. A
single blocking step inside an inner tool remains non-preemptible. Expiration
fails the whole batch without returning partial results, and input argument
objects are not modified.

The two write patch tools and the two VFS-only patch tools accept an optional
cooperative `timeout_ms` budget capped at `300000` milliseconds. The budget
covers target resolution and patch validation, and VFS-backed writes restore
the prior buffer when it expires before persistence. Once an atomic disk write
starts, the operation reports the write/index-sync outcome rather than a timeout
after the source may already have changed.

`did_open`, `did_change`, `read_virtual_file`, `list_virtual_files`,
`apply_buffer_edit`, `commit_virtual_file`, `discard_virtual_file`, and
`did_close` accept the same optional timeout cap. Open and read budgets cover
loading, parsing, clean-buffer refresh, and result validation; a timeout restores
the exact prior entry or removes one loaded only for the failed request. Listing
refreshes loaded entries in deterministic order and rolls all refreshes back if
its budget expires. Byte and position edits share one request budget across
loading, range and position validation, source splicing, incremental parsing,
syntax collection, and result validation. They stage each edit before a final
mutation gate, and a failed batch restores the exact prior buffer. Commit and
discard retain their final pre-mutation gates, while `did_close` follows the
commit path when `persist=true` and the discard path otherwise. After persistence
or buffer replacement starts, these operations report their final outcome
instead of a late timeout.

`list_symbol_indexes` and `unregister_symbol_index` accept the same timeout cap.
Listing checks it while collecting and validating registrations and around
deterministic sorting. Unregister retains a final gate after path normalization
and immediately before mutation; a timeout through that gate preserves the
registration, while a started removal returns its actual outcome.

`migrate_symbol_index` also accepts the `timeout_ms` cap. Its cooperative budget
covers path and database setup, schema and workspace metadata checks, legacy row
loading, persisted-path validation, and a final gate before the schema migration
transaction. A timeout before that gate leaves the database unchanged. Once the
transaction begins, Arborist completes the required source rebuild and final
health inspection and returns their actual outcome rather than a late timeout.
Individual SQLite queries, source reads, the schema transaction, and rebuild
persistence remain non-preemptible.

The offline `replay_patch_evidence_against_trace`,
`validate_patch_commit_with_trace`, and `export_patch_diagnostics_sarif` tools
accept the same timeout cap. Their cooperative native budgets cover validated
patch/trace traversal and result construction after strict JSON decoding;
individual decodes, source parses, model-validation calls, and serialization
steps remain non-preemptible.

Use `python -m arborist_mcp.gateway --dump-tool-catalog` or read
[`docs/tool-catalog.json`](docs/tool-catalog.json) for exact names, input
schemas, output schemas, defaults, and categories.

## Quick Start

On Windows:

```powershell
python -m venv .venv
. .\.venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
.\scripts\sync-extension.ps1 -SkipBuild
python scripts\gateway_smoke.py --require-core
python -m arborist_mcp.gateway --help
```

Or run:

```powershell
.\scripts\bootstrap.ps1
```

## Index Watch

Use the polling watch command to keep one persisted index synchronized without
rewriting it while it is healthy:

```powershell
arborist-index-watch --version
arborist-index-watch --workspace-root . --db-path .\symbols.db
arborist-index-watch --workspace-root . --db-path .\symbols.db --once
arborist-index-watch --workspace-root . --db-path .\symbols.db --once --timeout-ms 5000
arborist-index-watch --workspace-root . --db-path .\symbols.db --once --dry-run
arborist-index-watch --workspace-root . --db-path .\symbols.db --check
```

`--version` reports the installed Arborist package version without requiring a
watch target. The watcher refreshes missing indexes and current-schema
freshness issues
through the incremental workspace refresh path. It exits without writing when
inspection requires manual intervention, such as an unsupported or foreign
SQLite schema. `--timeout-ms` bounds health freshness reads and workspace
reconciliation scans as well as refresh indexing work.
`--dry-run` reports `would_refresh` or `would_migrate` without changing an
index. `--check` performs that no-write inspection once and returns a nonzero
exit status unless every target is healthy, which is useful for CI and
deployment checks. `--check` is mutually exclusive with `--once` and cannot be
combined with `--dry-run` or a non-default `--interval-seconds`. Emitted health
summaries include issue, stale, missing, unreadable, and unindexed file counts.

To watch several registered workspace/index pairs, provide a JSON manifest:

```json
{
  "indexes": [
    {"workspace_root": "./workspace-a", "db_path": "./indexes/a.db"},
    {"workspace_root": "./workspace-b", "db_path": "./indexes/b.db"}
  ]
}
```

Run it with `arborist-index-watch --config .\watch.json`. Relative paths in
the manifest are resolved from the manifest directory. Each target is
inspected and reconciled in deterministic workspace order; an unsupported or
foreign index stops the command without rewriting it. Duplicate workspace or
database paths are rejected before the first refresh.

On Linux or macOS:

```bash
python3 -m venv .venv
. .venv/bin/activate
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
python -m pip install .
python scripts/gateway_smoke.py --require-core
python -m arborist_mcp.gateway --help
```

## Quick Validation

For the normal local loop:

```powershell
.\scripts\test.ps1 -Suite inner-loop
```

Useful suite variants:

```powershell
.\scripts\test.ps1 -Suite python-fast
.\scripts\test.ps1 -Suite python-native
.\scripts\test.ps1 -Suite python
.\scripts\test.ps1 -Suite rust,inner-loop -ShowPlan
python scripts/python_suite_manifest.py
```

For the full gate:

```powershell
.\scripts\check.ps1
```

Useful profile variants:

```powershell
.\scripts\check.ps1 -Profile python-fast
.\scripts\check.ps1 -Profile gateway-fast
.\scripts\check.ps1 -Profile gateway-native
.\scripts\check.ps1 -Profile python-discovery
.\scripts\check.ps1 -Profile gateway-smoke
.\scripts\check.ps1 -Profile python-native
.\scripts\check.ps1 -Profile full,python-native -ShowPlan
```

Useful direct commands:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python scripts\tool_catalog.py --check
python scripts\gateway_smoke.py --require-core
python scripts\gateway_smoke.py --launcher console --require-core
python -m unittest tests.gateway_protocol.request_validation
python -m arborist_mcp.gateway --help
```

See the [development guide](docs/development.md) for profiles, suite names,
native-extension sync behavior, CI coverage, and release wheel builds.

## MCP Usage

Minimal MCP server configuration:

```json
{
  "mcpServers": {
    "arborist": {
      "command": "python",
      "args": ["-m", "arborist_mcp.gateway"],
      "cwd": "E:/workspace/arborist-mcp"
    }
  }
}
```

If Arborist is installed as a package, `arborist-mcp` is equivalent:

```json
{
  "mcpServers": {
    "arborist": {
      "command": "arborist-mcp",
      "args": []
    }
  }
}
```

Minimal MCP messages:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"example-client","version":"0.1.0"}}}
{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"arborist/get_semantic_skeleton","arguments":{"file_path":"tests/fixtures/sample.py","depth_limit":2}}}
```

Legacy direct `arborist/*` JSON-RPC calls remain supported over the same
newline-delimited stdio transport. See the [protocol guide](docs/protocol.md)
for response shapes, error behavior, and examples.

## Core Capabilities

- Semantic skeletons with stable selectors, symbol IDs, signatures, byte ranges,
  parameters, return types, and docstrings when available.
- One-shot source overlays for unsaved-file analysis, including persisted-index
  read/trace/list/search overlays when `index_db_path` is supplied.
- Patch preview tools that return validation plus unified diff without writing
  to disk, with optional cooperative budgets across target resolution,
  validation, and diff generation.
- Semantic skeleton extraction with bounded depth and expansion selectors plus
  cooperative budgets for file reads and post-parse symbol traversal.
- MCP resources expose the generated tool catalog snapshot for clients that
  prefer resource reads over `tools/list`.
- Semantic patching with structured binding decisions, commit gates, bypass
  auditing, trace-backed replay validation, and cooperative budgets across
  context validation and multi-file edit previews.
- Session-scoped VFS with open/change/close, virtual patching, commit/discard,
  and incremental Tree-sitter edits.
- Python/C workspace symbol graph indexing, listing, searching, reading,
  tracing, bounded neighborhood context, and optional cooperative budgets for
  direct read and trace queries.
- JavaScript, TypeScript, and TSX Tree-sitter parsing, query execution,
  semantic skeletons, conservative direct-call tracing through local named imports,
  named re-export chains, default imports that name a module's default export,
  namespace member calls that follow the bound module's named and star re-export
  chains and default exports, namespace-object calls that resolve CommonJS
  callable exports, star re-export chains for named imports, static
  local module dependency refresh, and structural patching with syntax-level
  validation; language-specific reference-binding validation remains deferred.
- Rust Tree-sitter parsing, raw query execution, semantic skeletons, declaration indexing, and
  conservative local module dependency refresh through unambiguous out-of-line `mod` declarations.
  It traces unshadowed bare direct calls to functions in the same source-file module, qualified direct
  calls to functions in inline modules in the same source file, and direct
  `module::function()`/`crate::module::function()` calls through an explicit chain of source-file-root
  out-of-line `mod` declarations. Each module in the chain must have one default-layout source file
  and the terminal file must contain one matching top-level function. Unshadowed bare calls through
  exact `use crate::module::function;`, `use self::module::function;`, or `use super::...` bindings,
  including grouped `use` paths and explicit `as` aliases, resolve through the unique out-of-line
  parent/module chain. Crate-root imports from out-of-line children and repeated `super::` ancestor
  navigation are supported. Equivalent qualified `crate::...` and `super::...` calls from out-of-line
  children use the same conservative parent/module chain. Malformed source, `#[path]` semantics,
  duplicate declarations/import aliases, ambiguous layouts, and ambiguous parent chains fail closed; wildcard imports are not considered.
  Trait-implementation members are not indexed, and inline-module, Cargo, and import resolution beyond
  those exact bindings remains unavailable. Structural patching targets Rust functions, methods, and
  declaration items by semantic path or source position with syntax-level validation; language-specific
  patch binding validation remains deferred.
- Go Tree-sitter parsing, raw query execution, semantic skeletons, and conservative declaration
  indexing for named type specifications and aliases, functions, and methods with named local receiver
  types, selected by semantic path or source position. Static imports strictly below the nearest valid
  simple `go.mod` module path refresh importers when direct `.go` files in their package directory
  change. It traces unshadowed bare direct calls to top-level functions declared in the same source
  file or in one matching production source in the same directory and package, plus unambiguous direct calls
  through local package imports using an explicit alias or the imported package's declared name, and calls through a
  named composite literal such as `Counter{}.Value()`, `(&Counter{}).Value()`, or `Box[int]{}.Value()` to one matching production method in the same local package; a direct named type-conversion receiver such as `Scalar(value).Value()`, `(*Scalar)(value).Value()`, `(Scalar)(value).Value()`, or `Box[int](value).Value()` when its base type is one unique same-package production `type` specification; a direct named type-assertion receiver such as `value.(Scalar).Value()` when `Scalar` is one unique same-package production `type` specification; a simple local alias receiver such as `type Alias = Counter; Alias{}.Value()`, `Alias(value).Value()`, or `value.(Alias).Value()` when its alias chain reaches one unique same-package production named `type` declaration without a cycle; or an unshadowed named local receiver, local-type parameter, or directly declared function-body local variable. If the conversion-shaped receiver name has no matching local type specification, it retains a direct factory-function dependency rather than guessing a method target. Same-package production sources refresh conservatively as a group. Module-root imports, external modules, `replace`, `go.work`, vendoring, build tags,
  general cross-file/package/import resolution, qualified imported conversions, interface dispatch, and other method dispatch remain unavailable.
  Structural patching targets Go functions, methods, and type specifications and aliases by semantic
  path or source position with syntax-level validation; language-specific patch binding validation
  remains deferred.
- C# Tree-sitter parsing, raw query execution, semantic skeletons, declaration indexing, and
  conservative tracing for unshadowed unqualified calls, explicit `this.` method calls, inherited bare/explicit-`this.` instance calls through a unique class/record ancestor chain, and `: this(...)` constructor initializers,
  and conservative `base(...)` constructor initializers and `base.Method()` calls through a unique class/record ancestor chain with simple, unshadowed qualified, `global::`, local, or root-level global type-alias/namespace-import base types,
  globally namespace-qualified `global::...` static calls, unique simple same-namespace or enclosing-namespace
  `Type.Method()` static calls across the workspace, plus `Outer.Helper.Method()` or `AliasOuter.Helper.Method()` static calls through one unique local/imported outer type or unique local/global type alias and unique nested types in the caller's namespace, an enclosing namespace, imported namespaces, or root-level global usings, including from nested source types, type aliases and static imports at file
  root or directly in block/file-scoped namespaces, root-level `global using static` imports from any scanned source file
  (including directive-only files), root-level global namespace-import `Type.Method()` calls and global type-alias calls from any scanned source file,
  and namespace-import `Type.Method()` calls. Aliases resolve from the caller namespace through enclosing
  namespaces to file root, with the nearest binding taking precedence; static and ordinary imports from each
  of those scopes are considered; same-type unqualified/`this.` forms remain same-file. When no same-type method name is indexed, a non-static caller may trace a bare or explicit `this.` call through the same unique class/record ancestor chain used by `base.Method()`. `base(...)` and `base.Method()` require one unique class/record base declaration. Simple or generic, `global::`, and exact unshadowed qualified base names, plus unique unshadowed aliases or namespace imports declared at file root, in the caller's namespace or an enclosing namespace, or as root-level global usings contributed by scanned C# sources (including directive-only files), are supported. A qualified name fails closed when its first segment is an alias or when a source-namespace-relative type could shadow it. Constructor targets must have an exact-arity, non-`params` match. `base.Method()` walks only a unique class/record ancestor chain when no nearer indexed method has the same name; the first indexed method found must be one non-static, exact-arity, non-`params` method. Generic base spellings with balanced type-argument lists are normalized to their declaration path and resolve only when one indexed class/record path remains. Generic arity and type-argument selection, ambiguous or colliding alias/import, non-class/record namespace-import base types, cycles, and nearer static, `params`, ambiguous, or arity-mismatched methods fail closed. Imported and qualified static targets must be unique, static,
  exact-arity, and non-`params`; ambiguous imports/types and shadowed receivers fail closed. When any C# source file
  changes, refresh conservatively re-resolves every indexed C# symbol against all tracked C# sources, including directive-only
  global-using files; unchanged C# source files are not reindexed. Structural patch targeting covers classes, structs, interfaces, enums, records, methods, and constructors by semantic path or source position with syntax-level validation; language-specific C# patch binding validation remains deferred. Other member dispatch and overload type selection
  remain capability-gated pending dedicated C# adapter slices.
- Java Tree-sitter parsing, raw query execution, semantic skeletons, declaration indexing, and
  conservative refresh for explicit local type imports, single-member `import static` imports, and
  direct superclass links whose base resolves from the same package, a unique explicit local type import, or an exact qualified local source spelling and whose owning type maps to a local `.java` file under
  an ancestor source root. It traces an explicit `this(...)` constructor initializer when one same-type, same-file, non-varargs constructor matches the call arity; a direct local-source `super(...)` constructor initializer only when one unique direct base-class non-varargs constructor matches the call arity; plus unqualified and `this.method()` calls when one same-type, same-file, non-varargs method matches the call arity; `Type.method()` through a unique explicit non-static local type import with an
  unshadowed type name; and a bare call through a unique explicit local static-method import only
  when no same-type method has that name. It also traces a `Type.method()` call from a top-level caller class to a unique same-package top-level class or interface static method with an exact,
  non-varargs arity match, plus `Outer.Helper.method()` through a unique same-package or explicitly imported outer type and nested class. Matching callers are re-resolved during refresh without reindexing
  unchanged Java source files. Imported targets require a
  unique static-method arity match. Wildcard imports, static wildcard imports, static field imports, missing or ambiguous
  imports, instance/member dispatch other than explicit simple `super.method()` calls and inherited bare calls across unique local-source base chains, overloaded-call selection, and patching remain capability-gated
  pending dedicated Java resolution fixtures.
- SQLite-backed persisted symbol indexes with transactional v1-v5-to-v6 schema
  migration, persisted analysis provenance, source reindexing, health inspection,
  response schema versioning, stale/missing/unreadable/unindexed file diagnostics,
  bounded workspace scans, optional per-file byte limits and cooperative time
  budgets, partial refresh, and fail-closed handling for damaged, stale, or
  unrelated databases.
- C include-family tracing and patch disambiguation for header/source projects,
  including duplicate globals and file-local `static` symbols.

## Current Status

The multi-language adapter substrate is implemented through Phase 4: Python,
C, C++, JavaScript, TypeScript, and TSX use the registry and explicit
capabilities. JavaScript-family adapters provide semantic skeletons, indexing,
conservative local-module tracing, structural patching, source overlays, and
persisted-index coverage. Phase 5 now includes Rust parsing, raw queries, semantic skeletons,
conservative declaration indexing, local module dependency refresh, conservative bare and
inline-module-qualified direct-call graph tracing, position identity, and structural patch
targeting with syntax-level validation. Go now has parsing, raw
Tree-sitter queries, semantic skeletons, conservative declaration indexing, source-position identity,
static local-package dependency refresh under the nearest valid simple `go.mod` module path,
same-file bare plus unambiguous local-package imported-function direct-call graph tracing, and
structural patch targeting with syntax-level validation. Java
now contributes extension routing, raw Tree-sitter query execution, and package-qualified
semantic skeletons and declaration indexing for top-level and nested Java declarations, structural
patch targeting with syntax-level validation, plus conservative refresh for explicit local type
imports, single-member `import static` imports, and
direct superclass links whose base resolves from the same package, a unique explicit local type import, or an exact qualified local source spelling, and direct interface links whose interface resolves by the same local-source rules. Those links require an owning type that maps to a local `.java` file under
an ancestor source root. It traces an explicit `this(...)` constructor initializer to a unique same-type, same-file nonvarargs constructor with a matching arity, plus a direct local-source `super(...)` constructor initializer to a unique direct base-class non-varargs constructor with a matching arity, plus unqualified and `this.method()` calls to a unique same-type, same-file nonvarargs method with a matching arity,
`Type.method()` calls through a unique unshadowed explicit local type import, and bare calls through
unique explicit local static-method imports only when no same-type method has that name. It also
traces a `Type.method()` call from a top-level caller class to a unique same-package top-level class or interface static method with an exact,
non-varargs arity match, plus `Outer.Helper.method()` through a unique same-package or explicitly imported outer type and nested class. Matching callers are re-resolved during refresh without reindexing
unchanged Java source files. A bare or explicit `this.` call in a class with no explicit `extends` clause, or with one uniquely resolved direct local superclass chain that declares no method of that name, and one or more uniquely resolved direct local interfaces, including lexical outer-scope interface references, also trace only when exactly one interface chain or branch provides a directly declared, uniquely arity-matched nonvarargs `default` method and every other chain or branch proves it has no declaration of that method. Imported trace targets must be static with an exact unique arity. General cross-file/package/import resolution,
general interface dispatch beyond that limited default-method case, instance/member dispatch other than explicit simple `super.method()` calls and inherited bare calls across unique local-source base chains remain deliberately capability-gated. Structural patching targets Java classes, interfaces,
enums, records, annotation types, methods, constructors, and nested types by semantic path or source
position with syntax-level validation; language-specific patch binding validation remains deferred.
C# now contributes extension routing, raw Tree-sitter query execution, namespace-qualified semantic skeletons and declaration indexing
for block and file-scoped namespaces, structural patch targeting with syntax-level validation, and conservative trace
coverage for unqualified, `this.`, `base.`, `: this(...)`, static `Type.Method()`, alias/import, and root-level global-using
bindings. Kotlin now contributes extension routing, raw Tree-sitter query execution, package-qualified
semantic skeletons and declaration indexing, structural patch targeting with syntax-level validation, and conservative
local direct-call and companion-object trace coverage.
Go module replacements, workspaces, vendoring, and build tags do not influence
these capabilities.

Remaining larger work includes:

- Adding carefully-scoped Rust inline-module, Cargo, and import trace resolution beyond current
  bindings, plus language-specific Rust patch binding validation.
- Extending Go package/import trace resolution beyond direct local function calls, plus
  language-specific Go patch binding validation, only after dedicated fixtures establish safe
  behavior.
- Extending Java trace resolution beyond unique same-type calls and explicitly imported local static
  methods, plus language-specific Java patch binding validation, only after dedicated fixtures
  establish safe behavior.
- Extending C# member dispatch and overload type selection beyond conservative same-type, `base.`, static,
  alias/import, and global-using bindings, plus language-specific C# patch binding validation, only after
  dedicated fixtures establish safe behavior.
- Extending Kotlin trace resolution beyond conservative same-package, imported, and companion-object bindings,
  plus language-specific Kotlin patch binding validation, only after dedicated fixtures establish safe
  behavior.
- Language-specific JavaScript/TypeScript patch binding validation.
- Splitting large Rust modules such as `lib.rs`, `symbols.rs`, and `model.rs`.
- Reducing PyO3 wrapper repetition with parameter/context objects.
- Extending C++ semantic support beyond overload-aware callable identities to
  fuller language-aware overload resolution and remaining grammar coverage.
- Adding broader fuzz/property coverage and cancellation for remaining native
  symbol-resolution operations (C++ qualified/unqualified reference path and
  type-alias walkers now check cooperative scan deadlines).
