# Trace Coverage Regression Tests Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add regression coverage tests for the `demo/trace_coverage_demo` waveform so block-level and line-level coverage decisions at time 45 remain stable.

**Architecture:** Extend the existing integration coverage tests instead of adding new production APIs. Reuse the real `demo/trace_coverage_demo` design and waveform with `SvParserProvider`, `DataflowBlockizer`, and `VcdCoverageTracker`, then assert the specific covered and uncovered branch lines for the three target `always` blocks at the annotation time that corresponds to time 45 in the waveform.

**Tech Stack:** Rust 2021, `cargo test`, integration tests under `tests/integration`, `sv-parser`, `wellen`.

---

### Task 1: Add the first failing regression test for the state register block

**Files:**
- Modify: `tests/integration/read_demo_wave.rs`
- Test: `tests/integration/read_demo_wave.rs`

**Step 1: Write the failing test**

Add a test that opens `demo/trace_coverage_demo/logs/sim.vcd`, blockizes `demo/trace_coverage_demo/design.sv`, finds the sequential state-register block on lines 23-28, and asserts at logical time 45 that line 25 is uncovered while line 27 is covered.

**Step 2: Run test to verify it fails**

Run: `cargo test --test read_demo_wave state_register_block_marks_else_but_not_reset_branch_at_time_45 -- --exact`

Expected: FAIL if the current fixture-to-time mapping or block/coverage lookup does not yet expose the requested behavior.

**Step 3: Write minimal implementation**

Add only the smallest shared helper code needed inside the test file to load the design blocks and the waveform-backed coverage tracker.

**Step 4: Run test to verify it passes**

Run: `cargo test --test read_demo_wave state_register_block_marks_else_but_not_reset_branch_at_time_45 -- --exact`

Expected: PASS.

### Task 2: Add the second failing regression test for the next-state combinational block

**Files:**
- Modify: `tests/integration/read_demo_wave.rs`
- Test: `tests/integration/read_demo_wave.rs`

**Step 1: Write the failing test**

Add a test that finds the combinational next-state block on lines 31-50 and asserts at logical time 45 that the `ST_IDLE` case line 34 is covered, the `if (op != 2'b00)` branch line 35 is covered, and the `else` line 38 is uncovered.

**Step 2: Run test to verify it fails**

Run: `cargo test --test read_demo_wave next_state_block_marks_idle_if_but_not_else_at_time_45 -- --exact`

Expected: FAIL until the regression expectation is encoded correctly.

**Step 3: Write minimal implementation**

Reuse the helper from Task 1; do not add production code.

**Step 4: Run test to verify it passes**

Run: `cargo test --test read_demo_wave next_state_block_marks_idle_if_but_not_else_at_time_45 -- --exact`

Expected: PASS.

### Task 3: Add the third failing regression test for the ALU sequential block

**Files:**
- Modify: `tests/integration/read_demo_wave.rs`
- Test: `tests/integration/read_demo_wave.rs`

**Step 1: Write the failing test**

Add a test that finds the sequential ALU block on lines 53-79 and asserts at logical time 45 that the block is covered, the line-57 condition is covered only through the `else` side of the `else if`, the `if` side is not taken, and none of the case arm lines 59, 64, 69, or 73 are covered at that time.

**Step 2: Run test to verify it fails**

Run: `cargo test --test read_demo_wave alu_exec_case_is_not_covered_at_time_45 -- --exact`

Expected: FAIL until the expected branch coverage is expressed correctly.

**Step 3: Write minimal implementation**

Keep the assertions local to the new test and reuse shared fixture helpers only.

**Step 4: Run test to verify it passes**

Run: `cargo test --test read_demo_wave alu_exec_case_is_not_covered_at_time_45 -- --exact`

Expected: PASS.

### Task 4: Verify the regression coverage suite stays green

**Files:**
- Modify: `tests/integration/read_demo_wave.rs`
- Test: `tests/integration/read_demo_wave.rs`

**Step 1: Run the focused integration target**

Run: `cargo test --test read_demo_wave -v`

Expected: PASS for all waveform regression tests.

**Step 2: Run the neighboring coverage target**

Run: `cargo test --test read_trace_coverage -v`

Expected: PASS.

**Step 3: Run broader verification if the test file required non-trivial helper refactoring**

Run: `cargo test --all-targets -v`

Expected: PASS.
