# AGENTS.md

## Non-Negotiables

- Do not modify or create anything under `sv-analysis/`. It is ignored and treated as read-only reference material.
- When you need AST identity or source locations, use `sv_parser` nodes and `Locate` data already carried through `sva-core`; do not identify AST elements by raw source text.

## Workspace Map

- This repo is a Cargo workspace. Members: `crates/sva-core`, `crates/sva-cli`, `crates/sva-mcp`, `crates/sva-vscode`.
- `crates/sva-core` is the shared engine. Public modules are re-exported from `crates/sva-core/src/lib.rs`; the shared frontend entrypoints live in `crates/sva-core/src/services.rs`.
- `crates/sva-cli/src/main.rs` is the Clap CLI. The help text uses `dataflow-engine`, but the Cargo binary and test binary name is `sva_cli`.
- `crates/sva-mcp` is the `rmcp` stdio server. `crates/sva-vscode` is a line-oriented JSON-RPC backend. Keep both as thin adapters over `sva_core::services`.
- `viewer/` is a standalone static slice-graph viewer, not part of the Cargo workspace.

## Commands

- From the repo root, plain `cargo run` is ambiguous. Always pass `-p sva_cli`, `-p sva_mcp`, or `-p sva_vscode`.
- Full workspace verification: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, then `cargo test --all-targets -v`.
- CLI smoke check: `cargo run -p sva_cli -- --help`
- Subcommand help: `cargo run -p sva_cli -- slice --help`
- Run one CLI integration target: `cargo test -p sva_cli --test cli_slice -v`
- Run one core integration target: `cargo test -p sva_core --test slice_json -v`
- Run one exact CLI test: `cargo test -p sva_cli --test smoke_cli cli_shows_help -- --exact`
- Run one exact core test: `cargo test -p sva_core --test slice_json static_slice_graph_serializes_without_time_annotations -- --exact`
- For `viewer/`, serve it with `python3 -m http.server 4173 --directory viewer`; `file://` will not work because the page uses ES modules and fetches fixtures.

## Test And Fixture Quirks

- Integration tests live under each crate: `crates/sva-cli/tests/` and `crates/sva-core/tests/`. There is no top-level `tests/` directory.
- CLI integration tests use `env!("CARGO_BIN_EXE_sva_cli")`. Do not use the stale `main` binary name from older docs.
- `demo/` is ignored by Git. `demo/multi_submodule_demo/*.sv` is tracked, but the `demo/trace_coverage_demo/` assets used by wave and coverage tests are local ignored files.
- If `demo/trace_coverage_demo/logs/sim.vcd` or `demo/trace_coverage_demo/logs/coverage.dat` is missing or stale, regenerate them with `make -C demo/trace_coverage_demo` (requires `verilator`; this also refreshes `logs/annotated/`).
- Because `demo/` is ignored, regenerated fixtures will not appear in `git status` unless force-added.

## Parser And JSON Gotchas

- `ParseOptions.project_path` and CLI `--project-path` recurse only over `.sv` files. Tops outside that tree still need explicit `--sv` arguments.
- CLI `--include-paths` is one comma-delimited flag. Parser include search paths are built from each source file's parent, the `project_path` (or its parent if `project_path` is a file), plus the explicit include paths.
- Parsing always defines `RVFI` in `crates/sva-core/src/ast/sv_parser.rs`.
- JSON shape and ordering are part of the contract. `types_json`, `slice_json`, and the CLI JSON tests assert exact output, so keep serialization deterministic and update the narrowest affected tests when schema changes are intentional.

## Repo-Local Skills

- Load `sv-parser-usage` when changing parser or blockization code.
- Load `wellen` when changing waveform or coverage code.
- Load `rmcp-usage` when changing `crates/sva-mcp`.
