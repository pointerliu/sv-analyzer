# AGENTS.md

This file is for agentic coding agents working in this repository.

## Hard rule

- DO NOT modify or create files anywhere under `sv-analysis/`.
- DO NOT use text string to identity ast elements, use the AST elements in `sv-parser`

## Project snapshot

- Language: Rust (`edition = "2021"`).
- Crate name: `dac26_mcp`.
- Binary target: `main` at `src/bin/main.rs`.
- User-facing CLI name: `dataflow-engine`.
- Main library modules: `ast`, `block`, `coverage`, `slicer`, `types`, `wave`.
- The codebase is a trait-oriented HDL dataflow analysis engine with a CLI wrapper.
- Important external crates: `anyhow`, `clap`, `serde`, `serde_json`, `sv-parser`, `wellen`.

## Repo-specific rules files

- Existing repository rule: do not touch `sv-analysis/`.
- No `.cursorrules` file was found.
- No `.cursor/rules/` directory was found.
- No `.github/copilot-instructions.md` file was found.

## Build commands

- Build everything: `cargo build`
- Build with verbose output: `cargo build -v`
- Check without producing binaries: `cargo check`
- Run the CLI help: `cargo run -- --help`
- Run a subcommand help page: `cargo run -- slice --help`

## Formatting and linting

- Format the whole repo: `cargo fmt --all`
- Check formatting without rewriting: `cargo fmt --all -- --check`
- Run clippy with the repo's expected strictness: `cargo clippy --all-targets --all-features -- -D warnings`

## Test commands

- Run the full test suite: `cargo test --all-targets -v`
- Run all integration tests only: `cargo test --tests -v`
- Run a single integration test target: `cargo test --test smoke_cli -v`
- Run one exact test in one integration test target:
  `cargo test --test smoke_cli cli_shows_help -- --exact`
- Run tests by substring if exact name is not needed:
  `cargo test cli_shows_help -- --nocapture`
- Run one library/module-oriented test if unit tests are added later:
  `cargo test module_name::test_name -- --exact`
- Show test output for debugging: `cargo test --test cli_wave -- --nocapture`

## Current named integration test targets

- `smoke_cli`
- `types_json`
- `trait_object_compile`
- `parse_demo_sv`
- `block_models`
- `extract_assign_dataflow`
- `blockize_demo_design`
- `read_demo_wave`
- `read_trace_coverage`
- `static_slice_demo`
- `blues_demo`
- `slice_json`
- `cli_blockize`
- `cli_coverage`
- `cli_wave`
- `cli_slice`

## Testing workflow guidance

- Prefer the smallest relevant command first, then widen scope.
- For CLI behavior changes, start with the most relevant `tests/integration/*.rs` target.
- For serialization changes, run targeted JSON tests before the full suite.
- After a focused test passes, run at least `cargo test --all-targets -v` if the change is broad.
- If you modify public CLI flags or output, also run the matching CLI integration tests.

## Code organization

- Keep the library/CLI split intact: reusable logic belongs in `src/`, argument parsing and printing belong in `src/bin/main.rs`.
- Follow the existing module layout instead of creating ad hoc files.
- Trait boundaries are intentional and central to the design:
  `AstProvider`, `Blockizer`, `CoverageTracker`, `WaveformReader`, `Slicer`.
- New backend-style functionality should usually slot behind an existing trait before changing call sites.
- Re-export concrete implementations from the module root when that matches current patterns.

## Imports

- Group imports in this order when possible:
  standard library, external crates, then `crate::...` imports.
- Keep imports explicit; avoid wildcard imports.
- Use grouped `use` statements for related items, matching current style.
- Alias only when needed to disambiguate types, as in `BlockJson as StableBlockJson`.

## Formatting style

- Use `rustfmt`; do not hand-format against it.
- Prefer multi-line struct literals and match arms when they improve readability.
- Keep chained iterator code readable; line breaks are preferred over dense one-liners.
- Preserve the existing brace style and trailing comma style that `rustfmt` produces.

## Types and data modeling

- Prefer small strong types for domain values, e.g. `BlockId(pub u64)` and `Timestamp(pub i64)`.
- Derive standard traits aggressively when useful: `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`, `Hash`.
- Use `#[serde(...)]` attributes to keep serialized shapes stable and explicit.
- Preserve the current stable JSON contracts; many tests assert exact JSON structure.
- Prefer enums with explicit tagging for serialized variant types.
- Keep DTOs simple and serializable; avoid mixing CLI presentation concerns into core types.

## Naming conventions

- Types and traits: `UpperCamelCase`.
- Functions and methods: `snake_case`.
- Modules and files: `snake_case`.
- Test functions should describe behavior, e.g. `cli_shows_help`.
- Use domain-specific names over generic ones: `block_set`, `signal_lookup`, `annotation_query_times`.
- Keep CLI argument field names descriptive, even when clap exposes different flag names.

## Error handling

- Use `anyhow::Result<T>` for fallible application code.
- Prefer `anyhow::bail!` for direct validation failures.
- Prefer `anyhow!` or `ok_or_else(...)` when constructing context-rich errors.
- Add `Context` / `with_context` around file IO, parsing, waveform loading, and other boundary operations.
- Return early on invalid inputs or impossible states.
- Avoid panics in production code unless the invariant is truly impossible and already established.
- In tests, `unwrap()` is acceptable and already used.

## Collections, determinism, and serialization

- `HashMap` and `HashSet` are common internally.
- When output order matters, sort explicitly before serializing or returning results.
- Maintain deterministic ordering for nodes, edges, block ids, names, and map entries.
- The repo already sorts vectors before emitting stable JSON; preserve that behavior.

## Trait and API design

- Keep traits small and capability-focused.
- Accept borrowed inputs like `&[PathBuf]`, `&SignalNode`, and `&str` when ownership is unnecessary.
- Return slices (`&[T]`) from getters when possible.
- Keep constructors validating invariants, as `Block::new` and `Block::with_signals` do.
- Prefer methods like `named`, `literal`, and `new` on domain types for ergonomic construction.

## CLI conventions

- Keep clap definitions near the top of `src/bin/main.rs`.
- Use dedicated `Args` structs per subcommand.
- Convert CLI inputs into core domain types before invoking library code.
- Emit JSON with `serde_json::to_string_pretty(...)` for machine-readable outputs.
- Validate mutually dependent flags explicitly and fail with clear messages.

## Testing conventions

- Most tests are integration tests under `tests/integration/`.
- CLI tests use `Command::new(env!("CARGO_BIN_EXE_main"))`.
- Serialization tests often compare against `serde_json::json!(...)` values directly.
- Keep tests focused on observable behavior and stable output formats.
- Prefer exact assertions for JSON keys, enum tags, and important field values.
- Add a targeted regression test for every bug fix when practical.

## Change guidance for agents

- Keep changes minimal and local.
- Do not refactor unrelated modules while fixing a targeted issue.
- Preserve public JSON shapes and CLI output unless the task explicitly changes them.
- If you introduce a new behavior path, add or update the narrowest relevant integration test.
- If you change trait contracts or core graph structures, expect multiple downstream tests to need updates.
- Before finishing, run formatting, clippy, and the smallest relevant tests; broaden to the full suite for wider changes.
