# Development Guide

This document covers local setup, validation, CI profiles, and release-oriented
builds for Arborist MCP. The repository is a mixed Rust + Python project:

- `crates/arborist-core`: Rust parsing, indexing, tracing, patching, and VFS logic.
- `crates/arborist-py`: PyO3 bridge exposed as `_arborist_core`.
- `python/arborist_mcp`: stdio JSON-RPC and MCP gateway.

## Local Setup

On Windows:

```powershell
python -m venv .venv
. .\.venv\Scripts\Activate.ps1
python -m pip install --upgrade pip
python -m pip install "maturin>=1.7,<2.0"
maturin develop --locked
.\scripts\sync-extension.ps1 -SkipBuild
```

Or run the bootstrap helper:

```powershell
.\scripts\bootstrap.ps1
```

`bootstrap.ps1` and `sync-extension.ps1` resolve the repository root
themselves. `sync-extension.ps1` keeps the repo-local generated gateway
extension in sync with the latest Rust build, so `python -m
arborist_mcp.gateway` works from the repository root.

On Linux and macOS, the PowerShell helpers are optional:

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

Windows is the primary development environment today. Linux and macOS CI also
run cross-platform metadata checks for version consistency and the generated
tool catalog, plus Rust formatting, linting, tests, and native-extension
gateway smoke. Release wheel builds run on Windows, Linux, and macOS; the
fuller native-extension profile matrix still runs on Windows.

## Common Checks

For the full local gate:

```powershell
.\scripts\check.ps1
```

The full gate checks PowerShell syntax, version consistency, the generated tool
catalog snapshot, Rust tests and linting, native extension build/sync, and
gateway smoke behavior.

Focused profiles are available:

```powershell
.\scripts\check.ps1 -ListProfiles
.\scripts\check.ps1 -Profile sanity
.\scripts\check.ps1 -Profile rust
.\scripts\check.ps1 -Profile gateway-fast
.\scripts\check.ps1 -Profile python-fast
.\scripts\check.ps1 -Profile gateway-native
.\scripts\check.ps1 -Profile python-discovery
.\scripts\check.ps1 -Profile gateway-smoke
.\scripts\check.ps1 -Profile python-native
.\scripts\check.ps1 -Profile sanity,rust
.\scripts\check.ps1 -Profile full,python-native -ShowPlan
```

The GitHub Actions workflow uses these same profiles in parallel on Windows.
The matrix is derived from the shared profile helper, which keeps local script
surface and CI job definitions aligned.

## Test Runner

For the everyday inner loop:

```powershell
.\scripts\test.ps1
```

The default `inner-loop` suite runs Rust plus the `python-fast` group. Native
extension suites build and sync the PyO3 module automatically unless
`-SyncExtension never` is supplied.

Useful suite selectors:

```powershell
.\scripts\test.ps1 -Suite rust
.\scripts\test.ps1 -Suite python-fast
.\scripts\test.ps1 -Suite python-native
.\scripts\test.ps1 -Suite gateway
.\scripts\test.ps1 -Suite gateway-fast
.\scripts\test.ps1 -Suite gateway-native
.\scripts\test.ps1 -Suite gateway-request-validation
.\scripts\test.ps1 -Suite gateway-symbol-routes
.\scripts\test.ps1 -Suite gateway-execution
.\scripts\test.ps1 -Suite gateway-trace-payloads
.\scripts\test.ps1 -Suite gateway-management-routes
.\scripts\test.ps1 -Suite gateway-runtime
.\scripts\test.ps1 -Suite python
.\scripts\test.ps1 -Suite all
.\scripts\test.ps1 -ListSuites
.\scripts\test.ps1 -Suite rust -RustFilter read_symbol_at_position
.\scripts\test.ps1 -Suite rust,gateway-fast
.\scripts\test.ps1 -Suite gateway-native -SyncExtension always
.\scripts\test.ps1 -Suite rust,inner-loop -ShowPlan
```

The gateway protocol tests live under `tests/gateway_protocol/` and remain
available through the legacy `tests.test_gateway_protocol` module.

## Direct Commands

The underlying commands are still useful for focused debugging:

```powershell
cargo fmt --check
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
python -m pip install .
python scripts/version_consistency.py
python scripts/gateway_smoke.py --require-core
python -m unittest tests.test_gateway_protocol
python -m unittest tests.gateway_protocol.request_validation
python -m unittest discover -s tests
python -m arborist_mcp.gateway --help
python -m arborist_mcp.gateway --version
python scripts\tool_catalog.py --check
```

The gateway smoke helper can run without loading the native extension unless
`--require-core` is supplied. Use `python -m pip install .` first when you want
Linux/macOS validation to exercise the compiled PyO3 extension path:

```powershell
python scripts\gateway_smoke.py
python scripts\gateway_smoke.py --require-core
```

## Lightweight Benchmarks

Use the benchmark helper after building the native extension with
`maturin develop --locked` or `python -m pip install .`. It generates a small Python workspace and measures
the core index and lookup workflows through the same gateway path used by MCP:

```powershell
python scripts\benchmark_core.py --iterations 10 --warmup 2 --modules 50
python scripts\benchmark_core.py --iterations 10 --json
```

The benchmark currently covers `rebuild_symbol_index`,
`refresh_symbol_index_for_file`, `trace_symbol_graph`, and `search_symbols`.
It is intended for local regression checks and comparative profiling rather
than CI pass/fail gating.

## Build And Release Artifacts

The current consumable artifact is a Python package with a PyO3 native
extension and the `arborist-mcp` console script.

Source checkouts can build locally with:

```bash
maturin develop --locked
python -m pip install .
```

Release wheels should be built with:

```bash
python -m pip install "maturin>=1.7,<2.0"
maturin build --locked --release
```

Generated wheels land under `target/wheels/` and can be installed with:

```bash
python -m pip install target/wheels/arborist_mcp-*.whl
```

GitHub Actions provides a manual `wheels` workflow and runs the same workflow
for `v*` tags across Windows, Linux, and macOS. A standalone binary server is
not published yet.

## Common Failures

- Python: make sure the active interpreter is Python 3.10 or newer and
  `python -m pip --version` points inside the intended virtual environment.
- Rust: install a stable toolchain with `rustup`, then retry `cargo test
  --locked`.
- Native extension: rerun `maturin develop --locked` or `python -m pip install .`
  after Rust changes.
- Repo-root gateway: `python -m arborist_mcp.gateway --help` requires the
  native `_arborist_core` module to be built or synced.
- Virtual, shared, or network drives: retry from a local non-synced path if
  file locking or path normalization behaves oddly.
- Slow dependency downloads: prefetch with `cargo fetch --locked`, keep Cargo's
  cache warm, or use the normal Cargo/PyPI mirror configuration.
