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
  same local package; a direct named type-conversion receiver such as `Scalar(value).Value()`, `(*Scalar)(value).Value()`, `(Scalar)(value).Value()`, or `Box[int](value).Value()` when its base type is one unique same-package production `type` specification; a direct named type-assertion receiver such as `value.(Scalar).Value()` when `Scalar` is one unique same-package production `type` specification; or an unshadowed named local receiver, local-type parameter, or directly declared function-body local variable. If the conversion-shaped receiver name has no matching local type specification, it retains a direct factory-function dependency rather than guessing a method target. Module-root imports, external
  modules, `replace`, `go.work`, vendoring, build tags,
  general cross-file/package/import resolution, qualified imported conversions, aliases, interface dispatch, and other method dispatch remain unavailable;
  patch operations return explicit unsupported-operation errors.
- C#: `.cs` — Tree-sitter parsing, raw queries, semantic skeletons, declaration indexing, and
  conservative tracing of unshadowed unqualified calls, explicit `this.` method calls, `: this(...)` constructor initializers,
  and conservative `base(...)` constructor initializers and `base.Method()` calls through a unique class/record ancestor chain with simple or generic, unshadowed qualified, `global::`, local, or root-level global type-alias/namespace-import base types,
  globally namespace-qualified `global::...` static calls, simple same-namespace
  `Type.Method()` static calls, including from nested source types, explicit type-alias calls of the form `Alias.Method()`,
  `using static Fully.Qualified.Type;` bare method calls, root-level `global using static Fully.Qualified.Type;` bare
  method calls contributed by any scanned C# source file (including directive-only files), and file-root
  `using Fully.Qualified.Namespace;` calls of the form `Type.Method()`, and root-level `global using Fully.Qualified.Namespace;`
  calls contributed by any scanned C# source file (including directive-only files), and root-level `global using Alias = Fully.Qualified.Type;`
  calls contributed by any scanned C# source file (including directive-only files). Type aliases, static imports, and namespace
  imports may appear at file root or directly in a block/file-scoped namespace; aliases are resolved from the caller's namespace through enclosing
  namespaces to file root, with the nearest binding taking precedence, while static/ordinary imports from each
  of those scopes are considered.
  Same-type unqualified/`this.`
  forms require a unique same-file, exact-arity, non-`params` declaration. Globally qualified, simple same-namespace,
  and imported static calls may resolve one unique workspace declaration. Imported targets must resolve to one indexed
  type. Duplicate aliases within the same scope, duplicate imports, same-file type-name collisions, competing
  imported targets, and ambiguous type declarations fail closed. Static imports are considered only when no same-type
  method has the bare
  call's name; namespace imports are considered only when the caller's namespace has no type of the receiver name
  anywhere in the workspace. Simple type receivers must not be shadowed by a local, parameter, type parameter, or
  type member. Block and file-scoped namespaces, classes, structs, interfaces, enums, records, methods, and
  constructors are supported. `base(...)` and `base.Method()` require one unique class/record base declaration. Simple or generic, `global::`, and exact unshadowed qualified base names, plus unique unshadowed aliases or namespace imports declared at file root, in the caller's namespace or an enclosing namespace, or as root-level global usings contributed by scanned C# sources (including directive-only files), are supported. A qualified name fails closed when its first segment is an alias or when a source-namespace-relative type could shadow it. Constructor targets must have an exact-arity, non-`params` match. `base.Method()` walks only a unique class/record ancestor chain when no nearer indexed method has the same name; the first indexed method found must be one non-static, exact-arity, non-`params` method. Generic base spellings with balanced type-argument lists are normalized to their declaration path and resolve only when one indexed class/record path remains. Generic arity and type-argument selection, ambiguous or colliding alias/import, non-class/record namespace-import base types, cycles, and nearer static, `params`, ambiguous, or arity-mismatched methods fail closed. When any C# source file
  changes, refresh conservatively re-resolves every indexed C# symbol against all tracked C# sources, including directive-only
  global-using files; unchanged C# source files are not reindexed. Other member dispatch, overload type selection, and patch operations return
  explicit unsupported-operation errors until dedicated C# adapter
  slices establish their contracts and fixtures.
- Java: `.java` — Tree-sitter parsing, raw queries, semantic skeletons, declaration indexing, and
  conservative dependency refresh for explicit local type imports and single-member `import static`
  imports whose owning type maps to a local `.java` file under an ancestor source root. Classes,
  interfaces, enums, annotation types, methods, and constructors use package-qualified paths. It
  traces unqualified and `this.method()` calls only when one same-type, same-file, non-varargs
  method has the call arity; `Type.method()` through a unique explicit non-static local type import
  with an unshadowed type name; and a bare call through a unique explicit local static-method import
  only when no same-type method has that name. Imported targets require a unique static-method arity
  match. Wildcard imports, static wildcard imports, static field/type imports, missing or ambiguous
  imports, instance/member dispatch, overloaded-call selection, and patch operations return explicit
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
