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
```

## Conventions

- Files are LF-only and intentionally tiny.
- `direct_calls.*` covers direct calls plus imports/qualified calls.
- `shadowing.*` covers shadowed names and nested scopes.
- `overloads.*` (languages with meaningful overloading) covers overloads.
- `malformed.*` is intentionally invalid syntax for parser robustness tests.
- Incremental-refresh scenarios need paired before/after edits and are written
  as explicit test edits rather than static files.
