# Assignment Statement Coverage Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Report covered vs uncovered assignment statements from the demo waveform/design at specific times, matching the oracle for times 45 and 65.

**Architecture:** Add a small library layer that extracts assignment statements plus their control context from parsed SystemVerilog, then computes statement coverage from waveform values and event timing. Keep the current line-based `coverage` command intact, and add a CLI path that reports assignment statements instead of only raw instrumented trace lines.

**Tech Stack:** Rust 2021, integration tests, `sv-parser`, `wellen`, existing `AstProvider` and `DataflowBlockizer` plumbing.

---

### Task 1: Lock the oracle into regression tests

**Files:**
- Modify: `tests/integration/read_demo_wave.rs`
- Modify: `tests/integration/cli_coverage.rs`

**Step 1: Write the failing test**

Add regression assertions for assignment statement lines `[25, 27, 32, 36, 38, 41, 44, 47, 55, 56, 60, 61, 62, 65, 66, 67, 70, 71, 74, 75]` with oracle coverage:
- time 45 -> covered `{27, 32, 36}`
- time 65 -> covered `{27, 32, 44, 65, 66, 67}`

**Step 2: Run tests to verify they fail**

Run: `cargo test --test read_demo_wave assignment_statement_coverage_matches_oracle -- --exact`

Expected: FAIL because assignment coverage is not implemented yet.

**Step 3: Add CLI regression**

Write a CLI integration test for the new report command and assert that its JSON output includes the same covered/uncovered line sets.

**Step 4: Run tests to verify they fail**

Run: `cargo test --test cli_coverage cli_reports_assignment_statement_coverage_for_demo_wave -- --exact`

Expected: FAIL because the command does not exist yet.

### Task 2: Implement assignment statement extraction and coverage evaluation

**Files:**
- Create or modify in `src/coverage/`
- Modify: `src/lib.rs`
- Modify: `src/bin/main.rs`

**Step 1: Add minimal statement model**

Represent assignment statements with source line, snippet, block kind, and enough guard metadata to evaluate whether the statement is active at a query time.

**Step 2: Reuse parsed AST and waveform values**

For combinational statements, evaluate guards using waveform values at the query time.
For sequential statements, first verify the sensitivity edge occurred from `time-1` to `time`, then evaluate guards using pre-edge waveform values.

**Step 3: Add CLI reporting**

Add a command that accepts `--sv`, `--vcd`, and `--time`, and prints all assignment statements split into covered and uncovered sets.

**Step 4: Run targeted tests to verify they pass**

Run:
- `cargo test --test read_demo_wave assignment_statement_coverage_matches_oracle -- --exact`
- `cargo test --test cli_coverage cli_reports_assignment_statement_coverage_for_demo_wave -- --exact`

Expected: PASS.

### Task 3: Verify focused and neighboring coverage behavior

**Files:**
- Modify: `tests/integration/read_demo_wave.rs`
- Modify: `tests/integration/cli_coverage.rs`

**Step 1: Run focused suites**

Run:
- `cargo test --test read_demo_wave -v`
- `cargo test --test cli_coverage -v`
- `cargo test --test read_trace_coverage -v`

Expected: PASS.

**Step 2: Run format and broader checks**

Run:
- `cargo fmt --all -- --check`
- `cargo test --all-targets -v`

Expected: pass except for any already-existing unrelated failures, which must be reported explicitly.
