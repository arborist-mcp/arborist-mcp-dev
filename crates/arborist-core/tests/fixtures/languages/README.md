# Language fixtures

Small, targeted source fixtures used by the multi-language adapter contract
suite (design doc §17.2). Each file stays small and exercises one concern so a
contract test can consume it directly.

## Layout

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
  kotlin/
  csharp/
  tsx/
```

## Conventions

- Files are LF-only and intentionally tiny.
- `direct_calls.*` covers direct calls plus imports/qualified calls for the fixture smoke suite.
- `resolver_direct_calls.*` and `resolver_unresolved_calls.*` are minimal same-file
  positive/negative resolver fixtures consumed by the common adapter contract.
- `shadowing.*` covers shadowed names and nested scopes.
- `overloads.*` (languages with meaningful overloading) covers overloads.
- `malformed.*` is intentionally invalid syntax for parser robustness tests.
- Incremental-refresh scenarios need paired before/after edits and are written
  as explicit test edits rather than static files.
