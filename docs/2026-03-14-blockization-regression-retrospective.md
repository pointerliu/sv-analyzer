# Blockization Regression Retrospective

This note records mistakes made while fixing the blockization bugs from `buggy_blocks.json` and the rules to follow so the same mistakes are less likely to happen again.

## Mistakes Made

### 1. I trusted derived text length for block line ranges

- Mistake: I treated `syntax_tree.get_str(...)` output as a safe way to infer `line_end` for `always` blocks.
- Why it was wrong: `sv-parser` text extraction included trailing trivia, including comments that belonged to the following section.
- Symptom: block ranges like `23..23` instead of `23..28`, and risk of overshooting when comments followed the block.
- Better rule: derive line ranges from structural AST end tokens, not from snippet line counts.

### 2. I did not update every consumer after changing `DataflowEntry.output`

- Mistake: I changed `DataflowEntry.output` from scalar to `Vec<SignalId>` in core code, but one integration test helper still constructed the old scalar form.
- Why it was wrong: a model shape change must be propagated through all tests and all query code in one pass.
- Symptom: `tests/integration/blues_demo.rs` failed to compile with `expected Vec<SignalId>, found SignalId`.
- Better rule: after changing a core type, search globally for constructors, equality checks, serialization expectations, and helper functions before claiming the migration is done.

### 3. I added a JSON assertion that depended on unstable set ordering

- Mistake: I wrote a regression test that expected `inputs` in one exact serialized order.
- Why it was wrong: `inputs` come from a `HashSet`, so order is not stable.
- Symptom: the new regression test failed even though the underlying bug was fixed.
- Better rule: only require exact order when the production type guarantees order; otherwise compare sorted values or compare sets.

### 4. I initially verified behavior indirectly instead of at the serialized boundary

- Mistake: some tests checked the in-memory model but did not directly assert the JSON fields the user was complaining about.
- Why it was wrong: the reported bug was about exported block JSON, not only internal structures.
- Symptom: coverage existed, but it was weaker than the real user-facing failure mode.
- Better rule: when a bug report references CLI or JSON output, add at least one regression assertion at that exact boundary.

## What Solved The Problems

### For incorrect line numbers in `design.sv`

- Solution: use structural AST traversal to compute end lines for `AlwaysConstruct`, `SeqBlock`, `case`, conditional statements, and continuous assignments.
- Regression coverage: `tests/integration/blockize_demo_design.rs` asserts the `clk` input block is on line 3 and the `always_ff` state-register block spans lines 23 through 28.

### For multi-target assignment outputs

- Solution: change `DataflowEntry.output` to `Vec<SignalId>` and update extraction, slicers, merged assign handling, and tests.
- Regression coverage: `tests/integration/extract_assign_dataflow.rs` asserts `{a, b} = {c, d};` produces one dataflow entry with `output: ["a", "b"]`.

### For top-level `ModOutput` semantics

- Solution: represent top-level module outputs as consuming the internal signal with no external output signals.
- Regression coverage: `tests/integration/blockize_demo_design.rs` asserts the `result` `ModOutput` block has `input_signals = ["result"]` and empty `output_signals`.

### For `code_snippet` vs `ast_snippet`

- Solution: rename the serialized field to `code_snippet` and assert the real source text is present in block JSON.
- Regression coverage: `tests/integration/blockize_demo_design.rs` asserts `code_snippet` exists and `ast_snippet` does not.

## Rules To Follow Next Time

1. If a bug is about parser structure, inspect the AST shape first and prefer structural tokens over reconstructed text.
2. If a core type changes, do a global search for constructors, comparisons, helper builders, and JSON assertions before running the suite.
3. If a test uses `HashSet`-backed data, make the assertion order-insensitive unless ordering is a documented contract.
4. If the user reports a CLI or JSON problem, add regression coverage at the CLI or JSON layer, not only in internal unit tests.
5. Before saying a bug is fixed, run the targeted regression test and then run the broader verification command that covers the affected area.

## Fresh Verification Used For This Fix Set

- `cargo test --test blockize_demo_design --test extract_assign_dataflow -v`
- `cargo test --all-targets -v`
