# Trait-Based Dataflow Engine Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a Rust library + CLI for SystemVerilog blockization and Blues-style dataflow slicing, using fully swappable trait-based core systems, VCD trace-coverage, and waveform reading.

**Architecture:** The crate is a library-first design with thin CLI wrappers. Core systems are abstracted behind traits (`AstProvider`, `Blockizer`, `CoverageTracker`, `WaveformReader`, `Slicer`) so implementations can be replaced without changing the analysis pipeline. The first implementation uses `sv-parser` for AST parsing, `wellen` for waveform/coverage access, and `petgraph` for the instruction-execution-path graph.

**Tech Stack:** Rust 2021, `sv-parser`, `wellen`, `petgraph`, `serde`, `serde_json`, `clap`, `anyhow`, `thiserror`, `rayon` (optional), `insta` or JSON snapshot tests (optional)

---

## Ground Rules

- Do not modify anything under `sv-analysis/`; it is read-only reference material.
- Keep all core systems trait-based and injectable.
- Prefer small, test-first steps.
- Keep the public API library-first; the CLI should only orchestrate library calls.
- Use the demo assets in `demo/trace_coverage_demo/` as the first integration fixture.

### Task 1: Create the crate skeleton

**Files:**
- Create: `Cargo.toml`
- Create: `src/lib.rs`
- Create: `src/bin/main.rs`
- Create: `src/types.rs`
- Create: `src/ast/mod.rs`
- Create: `src/ast/sv_parser.rs`
- Create: `src/block/mod.rs`
- Create: `src/block/dataflow.rs`
- Create: `src/coverage/mod.rs`
- Create: `src/coverage/vcd.rs`
- Create: `src/wave/mod.rs`
- Create: `src/wave/wellen.rs`
- Create: `src/slicer/mod.rs`
- Create: `src/slicer/blues.rs`
- Create: `src/slicer/static_slice.rs`
- Create: `tests/integration/smoke_cli.rs`

**Step 1: Write the failing smoke test**

```rust
use std::process::Command;

#[test]
fn cli_shows_help() {
    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .arg("--help")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("blockize"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test smoke_cli -v`
Expected: fail because the crate does not exist yet.

**Step 3: Write minimal crate scaffolding**

Add a `Cargo.toml` with library + binary targets and the initial dependency list. Add a minimal `clap` CLI with subcommands `blockize`, `slice`, `coverage`, and `wave`. Export empty modules from `src/lib.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test smoke_cli -v`
Expected: pass and show the CLI help text.

**Step 5: Commit**

```bash
git add Cargo.toml src tests/integration/smoke_cli.rs
git commit -m "feat: scaffold trait-based dataflow engine crate"
```

### Task 2: Define shared core types and serialization

**Files:**
- Modify: `src/types.rs`
- Modify: `src/lib.rs`
- Create: `tests/integration/types_json.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::types::{BlockId, BlockNode, InstructionExecutionPath, Timestamp};

#[test]
fn block_node_serializes_as_expected() {
    let node = BlockNode {
        block_id: BlockId(7),
        time: Timestamp(19),
    };

    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("block_id"));
    assert!(json.contains("time"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test types_json -v`
Expected: fail because the shared types are not implemented.

**Step 3: Write minimal implementation**

Define:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignalId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockNode {
    pub block_id: BlockId,
    pub time: Timestamp,
}
```

Also define JSON-friendly graph DTOs for CLI output even if the internal graph uses `petgraph`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test types_json -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/types.rs src/lib.rs tests/integration/types_json.rs
git commit -m "feat: add shared analysis types and json models"
```

### Task 3: Define the trait interfaces

**Files:**
- Modify: `src/ast/mod.rs`
- Modify: `src/block/mod.rs`
- Modify: `src/coverage/mod.rs`
- Modify: `src/wave/mod.rs`
- Modify: `src/slicer/mod.rs`
- Create: `tests/integration/trait_object_compile.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::ast::AstProvider;
use dac26_mcp::block::Blockizer;
use dac26_mcp::coverage::CoverageTracker;
use dac26_mcp::slicer::Slicer;
use dac26_mcp::wave::WaveformReader;

fn accepts_traits(
    _ast: &dyn AstProvider,
    _blockizer: &dyn Blockizer,
    _coverage: &dyn CoverageTracker,
    _wave: &dyn WaveformReader,
    _slicer: &dyn Slicer,
) {
}

#[test]
fn trait_objects_compile() {
    assert!(true);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test trait_object_compile -v`
Expected: fail because the traits are not defined.

**Step 3: Write minimal implementation**

Define the public traits with small, focused APIs. Recommended signatures:

```rust
pub trait AstProvider {
    type ParsedFile;
    fn parse_files(&self, paths: &[std::path::PathBuf]) -> anyhow::Result<Vec<Self::ParsedFile>>;
}

pub trait Blockizer {
    type ParsedFile;
    fn blockize(&self, files: &[Self::ParsedFile]) -> anyhow::Result<BlockSet>;
}

pub trait CoverageTracker {
    fn is_line_covered_at(&self, file: &str, line: usize, time: Timestamp) -> anyhow::Result<bool>;
    fn hit_count_at(&self, file: &str, line: usize, time: Timestamp) -> anyhow::Result<u64>;
}

pub trait WaveformReader {
    fn signal_value_at(&self, signal: &SignalId, time: Timestamp) -> anyhow::Result<Option<SignalValue>>;
}

pub trait Slicer {
    fn slice(&self, request: &SliceRequest) -> anyhow::Result<InstructionExecutionPath>;
}
```

Keep object-safety in mind; avoid generic methods on trait objects.

**Step 4: Run test to verify it passes**

Run: `cargo test --test trait_object_compile -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/ast/mod.rs src/block/mod.rs src/coverage/mod.rs src/wave/mod.rs src/slicer/mod.rs tests/integration/trait_object_compile.rs
git commit -m "feat: define swappable core analysis traits"
```

### Task 4: Implement `sv-parser` AST provider

**Files:**
- Modify: `src/ast/sv_parser.rs`
- Modify: `src/ast/mod.rs`
- Create: `tests/integration/parse_demo_sv.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::ast::{AstProvider, SvParserProvider};
use std::path::PathBuf;

#[test]
fn parses_demo_design_files() {
    let provider = SvParserProvider::default();
    let files = provider
        .parse_files(&[
            PathBuf::from("demo/trace_coverage_demo/design.sv"),
            PathBuf::from("demo/trace_coverage_demo/tb.sv"),
        ])
        .unwrap();

    assert_eq!(files.len(), 2);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test parse_demo_sv -v`
Expected: fail because `SvParserProvider` is not implemented.

**Step 3: Write minimal implementation**

Wrap `sv-parser` in `SvParserProvider`. The returned parsed-file type should retain enough data for blockization: source text, path, and parsed syntax tree.

**Step 4: Run test to verify it passes**

Run: `cargo test --test parse_demo_sv -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/ast/mod.rs src/ast/sv_parser.rs tests/integration/parse_demo_sv.rs
git commit -m "feat: add sv-parser backed ast provider"
```

### Task 5: Define blocks and graph-ready block set

**Files:**
- Modify: `src/block/mod.rs`
- Create: `tests/integration/block_models.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::block::{Block, BlockSet, BlockType, CircuitType};
use dac26_mcp::types::{BlockId, SignalId};

#[test]
fn block_holds_input_and_output_signals() {
    let block = Block {
        id: BlockId(1),
        block_type: BlockType::Assign,
        circuit_type: CircuitType::Combinational,
        module_scope: "alu".into(),
        source_file: "design.sv".into(),
        line_start: 60,
        line_end: 62,
        input_signals: [SignalId("a".into()), SignalId("b".into())].into_iter().collect(),
        output_signals: [SignalId("tmp".into())].into_iter().collect(),
        dataflow: Default::default(),
        ast_snippet: "tmp = a + b;".into(),
    };

    assert_eq!(block.output_signals.len(), 1);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test block_models -v`
Expected: fail because block models are incomplete.

**Step 3: Write minimal implementation**

Implement `BlockType`, `CircuitType`, `Block`, `DataflowEntry`, `BlockSet`, and indexes like `signal_to_drivers: HashMap<SignalId, Vec<BlockId>>` for efficient inter-block lookup.

**Step 4: Run test to verify it passes**

Run: `cargo test --test block_models -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/block/mod.rs tests/integration/block_models.rs
git commit -m "feat: add block models and block set indexes"
```

### Task 6: Implement basic statement-level dataflow extraction

**Files:**
- Modify: `src/block/dataflow.rs`
- Modify: `src/block/mod.rs`
- Create: `tests/integration/extract_assign_dataflow.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::ast::{AstProvider, SvParserProvider};
use dac26_mcp::block::{Blockizer, DataflowBlockizer};
use std::path::PathBuf;

#[test]
fn extracts_input_and_output_signals_from_assignments() {
    let provider = SvParserProvider::default();
    let parsed = provider
        .parse_files(&[PathBuf::from("demo/trace_coverage_demo/design.sv")])
        .unwrap();

    let blockizer = DataflowBlockizer::default();
    let blocks = blockizer.blockize(&parsed).unwrap();

    assert!(blocks.blocks.iter().any(|b| b.output_signals.iter().any(|s| s.0 == "result")));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test extract_assign_dataflow -v`
Expected: fail because the blockizer does not extract signals yet.

**Step 3: Write minimal implementation**

Start with these statement forms:
- continuous `assign`
- blocking assignment `=`
- non-blocking assignment `<=`
- `if` conditions contribute to input signals
- `case` selector contributes to input signals

Capture left-values as outputs and right-values plus conditions as inputs.

**Step 4: Run test to verify it passes**

Run: `cargo test --test extract_assign_dataflow -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/block/dataflow.rs src/block/mod.rs tests/integration/extract_assign_dataflow.rs
git commit -m "feat: extract statement-level systemverilog dataflow"
```

### Task 7: Implement paper-style blockization

**Files:**
- Modify: `src/block/dataflow.rs`
- Create: `tests/integration/blockize_demo_design.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::ast::{AstProvider, SvParserProvider};
use dac26_mcp::block::{BlockType, Blockizer, DataflowBlockizer};
use std::path::PathBuf;

#[test]
fn creates_assign_and_always_blocks_for_demo_design() {
    let provider = SvParserProvider::default();
    let parsed = provider
        .parse_files(&[PathBuf::from("demo/trace_coverage_demo/design.sv")])
        .unwrap();
    let blockizer = DataflowBlockizer::default();
    let block_set = blockizer.blockize(&parsed).unwrap();

    assert!(block_set.blocks.iter().any(|b| matches!(b.block_type, BlockType::Always)));
    assert!(block_set.blocks.iter().any(|b| matches!(b.block_type, BlockType::Assign | BlockType::ModInput | BlockType::ModOutput)));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test blockize_demo_design -v`
Expected: fail because block categories are incomplete.

**Step 3: Write minimal implementation**

Implement the four paper block types:
- `ModInput`
- `ModOutput`
- `Always`
- `Assign`

Also implement assign-block merging when one assign block's outputs feed another assign block's inputs.

**Step 4: Run test to verify it passes**

Run: `cargo test --test blockize_demo_design -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/block/dataflow.rs tests/integration/blockize_demo_design.rs
git commit -m "feat: implement blues-style dataflow blockization"
```

### Task 8: Implement waveform reader with `wellen`

**Files:**
- Modify: `src/wave/wellen.rs`
- Modify: `src/wave/mod.rs`
- Create: `tests/integration/read_demo_wave.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::types::{SignalId, Timestamp};
use dac26_mcp::wave::{WaveformReader, WellenReader};

#[test]
fn reads_signal_value_from_demo_vcd() {
    let wave = WellenReader::open("demo/trace_coverage_demo/logs/sim.vcd").unwrap();
    let value = wave
        .signal_value_at(&SignalId("tb.dut.state".into()), Timestamp(20))
        .unwrap();

    assert!(value.is_some());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test read_demo_wave -v`
Expected: fail because the waveform reader is not implemented.

**Step 3: Write minimal implementation**

Use `wellen` to load VCD/FST files and resolve signal handles by hierarchical name. Add a `SignalValue` type that can preserve raw bitstrings and optional pretty-printed hex.

**Step 4: Run test to verify it passes**

Run: `cargo test --test read_demo_wave -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/wave/mod.rs src/wave/wellen.rs tests/integration/read_demo_wave.rs
git commit -m "feat: add wellen-backed waveform reader"
```

### Task 9: Implement VCD trace-coverage tracker

**Files:**
- Modify: `src/coverage/vcd.rs`
- Modify: `src/coverage/mod.rs`
- Create: `tests/integration/read_trace_coverage.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::coverage::{CoverageTracker, VcdCoverageTracker};
use dac26_mcp::types::Timestamp;

#[test]
fn reads_verilator_trace_coverage_from_vcd() {
    let tracker = VcdCoverageTracker::open("demo/trace_coverage_demo/logs/sim.vcd").unwrap();
    let covered = tracker.is_line_covered_at("design", 35, Timestamp(30)).unwrap();
    assert!(matches!(covered, true | false));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test read_trace_coverage -v`
Expected: fail because coverage parsing is not implemented.

**Step 3: Write minimal implementation**

Parse VCD signals matching `vlCoverageLineTrace_<file>__<line>_<type>`. Store indexes for efficient lookup and implement `is_line_covered_at`, `hit_count_at`, and `delta_hits`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test read_trace_coverage -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/coverage/mod.rs src/coverage/vcd.rs tests/integration/read_trace_coverage.rs
git commit -m "feat: parse verilator trace coverage from vcd"
```

### Task 10: Implement static slicer

**Files:**
- Modify: `src/slicer/static_slice.rs`
- Modify: `src/slicer/mod.rs`
- Create: `tests/integration/static_slice_demo.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::slicer::{SliceRequest, Slicer, StaticSlicer};
use dac26_mcp::types::{SignalId, Timestamp};

#[test]
fn static_slice_builds_non_empty_graph() {
    let slicer = StaticSlicer::new_for_test();
    let path = slicer.slice(&SliceRequest {
        signal: SignalId("result".into()),
        time: Timestamp(20),
        min_time: Timestamp(-5),
    }).unwrap();

    assert!(!path.nodes.is_empty());
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test static_slice_demo -v`
Expected: fail because the static slicer is not implemented.

**Step 3: Write minimal implementation**

Implement a graph builder that walks backwards through `BlockSet.signal_to_drivers`, ignoring coverage and keeping timestamps according to `CircuitType`.

**Step 4: Run test to verify it passes**

Run: `cargo test --test static_slice_demo -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/slicer/mod.rs src/slicer/static_slice.rs tests/integration/static_slice_demo.rs
git commit -m "feat: add static block-level slicer"
```

### Task 11: Implement Blues dynamic slicer

**Files:**
- Modify: `src/slicer/blues.rs`
- Modify: `src/slicer/mod.rs`
- Create: `tests/integration/blues_demo.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::slicer::{BluesSlicer, SliceRequest, Slicer};
use dac26_mcp::types::{SignalId, Timestamp};

#[test]
fn blues_respects_custom_min_time_bound() {
    let slicer = BluesSlicer::new_for_test();
    let path = slicer.slice(&SliceRequest {
        signal: SignalId("result".into()),
        time: Timestamp(20),
        min_time: Timestamp(18),
    }).unwrap();

    assert!(path.nodes.iter().all(|n| n.time.0 >= 18));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test blues_demo -v`
Expected: fail because `BluesSlicer` is not implemented.

**Step 3: Write minimal implementation**

Implement queue-based backward slicing:
- queue starts with `(signal, time)`
- stop when `time < min_time`
- for sequential blocks, inspect assignment coverage at `t - 1`
- if uncovered, propagate self at `t - 1`
- use a visited set on `(block_id, time)` or `(signal, time, block_id)` to prevent loops

**Step 4: Run test to verify it passes**

Run: `cargo test --test blues_demo -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/slicer/mod.rs src/slicer/blues.rs tests/integration/blues_demo.rs
git commit -m "feat: implement blues dynamic slicer"
```

### Task 12: Add graph export and stable JSON output

**Files:**
- Modify: `src/slicer/mod.rs`
- Modify: `src/types.rs`
- Create: `tests/integration/slice_json.rs`

**Step 1: Write the failing test**

```rust
use dac26_mcp::slicer::InstructionExecutionPath;

#[test]
fn path_serializes_with_nodes_and_edges() {
    let path = InstructionExecutionPath::default();
    let json = serde_json::to_string(&path).unwrap();
    assert!(json.contains("nodes"));
    assert!(json.contains("edges"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test slice_json -v`
Expected: fail because the DTO/export type is missing.

**Step 3: Write minimal implementation**

Expose a stable JSON representation:

```json
{
  "nodes": [{"id": 0, "block_id": 17, "time": 19}],
  "edges": [{"from": 1, "to": 0}],
  "blocks": [{"id": 17, "scope": "tb.dut", "type": "Always"}]
}
```

Keep the internal `petgraph` structure private if needed.

**Step 4: Run test to verify it passes**

Run: `cargo test --test slice_json -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/slicer/mod.rs src/types.rs tests/integration/slice_json.rs
git commit -m "feat: export slice graphs as stable json"
```

### Task 13: Implement CLI `blockize` command

**Files:**
- Modify: `src/bin/main.rs`
- Create: `tests/integration/cli_blockize.rs`

**Step 1: Write the failing test**

```rust
use std::process::Command;

#[test]
fn cli_blockize_outputs_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .args(["blockize", "--sv", "demo/trace_coverage_demo/design.sv"])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("blocks"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test cli_blockize -v`
Expected: fail because the command is not wired.

**Step 3: Write minimal implementation**

Connect CLI parsing to `SvParserProvider` + `DataflowBlockizer` and print JSON to stdout.

**Step 4: Run test to verify it passes**

Run: `cargo test --test cli_blockize -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/bin/main.rs tests/integration/cli_blockize.rs
git commit -m "feat: add blockize cli command"
```

### Task 14: Implement CLI `coverage` and `wave` commands

**Files:**
- Modify: `src/bin/main.rs`
- Create: `tests/integration/cli_coverage.rs`
- Create: `tests/integration/cli_wave.rs`

**Step 1: Write the failing tests**

```rust
use std::process::Command;

#[test]
fn cli_coverage_reports_hit_info() {
    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .args([
            "coverage", "--vcd", "demo/trace_coverage_demo/logs/sim.vcd",
            "--file", "design", "--line", "35", "--time", "30"
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
}

#[test]
fn cli_wave_reports_signal_value() {
    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .args([
            "wave", "--vcd", "demo/trace_coverage_demo/logs/sim.vcd",
            "--signal", "tb.dut.state", "--time", "20"
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test cli_coverage --test cli_wave -v`
Expected: fail because the commands are not wired.

**Step 3: Write minimal implementation**

Wire the commands to `VcdCoverageTracker` and `WellenReader`. Print JSON by default.

**Step 4: Run tests to verify they pass**

Run: `cargo test --test cli_coverage --test cli_wave -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/bin/main.rs tests/integration/cli_coverage.rs tests/integration/cli_wave.rs
git commit -m "feat: add coverage and wave cli commands"
```

### Task 15: Implement CLI `slice` command

**Files:**
- Modify: `src/bin/main.rs`
- Create: `tests/integration/cli_slice.rs`

**Step 1: Write the failing test**

```rust
use std::process::Command;

#[test]
fn cli_slice_outputs_graph_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .args([
            "slice",
            "--sv", "demo/trace_coverage_demo/design.sv",
            "--sv", "demo/trace_coverage_demo/tb.sv",
            "--vcd", "demo/trace_coverage_demo/logs/sim.vcd",
            "--signal", "result",
            "--time", "30",
            "--min-time", "0"
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("nodes"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test cli_slice -v`
Expected: fail because the command is not wired.

**Step 3: Write minimal implementation**

Wire the end-to-end pipeline:
- parse SV files
- blockize them
- open the VCD for waveform + coverage
- run `BluesSlicer` by default or `StaticSlicer` with `--static`
- print stable JSON output

**Step 4: Run test to verify it passes**

Run: `cargo test --test cli_slice -v`
Expected: pass.

**Step 5: Commit**

```bash
git add src/bin/main.rs tests/integration/cli_slice.rs
git commit -m "feat: add end-to-end slice cli command"
```

### Task 16: Add demo-focused regression tests

**Files:**
- Create: `tests/integration/demo_regression.rs`

**Step 1: Write the failing test**

```rust
use std::process::Command;

#[test]
fn end_to_end_demo_pipeline_completes() {
    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .args([
            "slice",
            "--sv", "demo/trace_coverage_demo/design.sv",
            "--sv", "demo/trace_coverage_demo/tb.sv",
            "--vcd", "demo/trace_coverage_demo/logs/sim.vcd",
            "--signal", "tb.dut.result",
            "--time", "40",
            "--min-time", "0"
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test demo_regression -v`
Expected: fail until the entire pipeline is stable.

**Step 3: Write minimal implementation**

Fix issues discovered by the end-to-end test. Avoid expanding scope; only make the minimal changes needed to stabilize the pipeline.

**Step 4: Run test to verify it passes**

Run: `cargo test --test demo_regression -v`
Expected: pass.

**Step 5: Commit**

```bash
git add tests/integration/demo_regression.rs src
git commit -m "test: add demo regression coverage for end-to-end slicing"
```

### Task 17: Verify the full crate

**Files:**
- Modify: `README.md` if needed later

**Step 1: Run formatting**

Run: `cargo fmt --all`
Expected: success.

**Step 2: Run linting**

Run: `cargo clippy --all-targets --all-features -- -D warnings`
Expected: success.

**Step 3: Run tests**

Run: `cargo test --all-targets -v`
Expected: success.

**Step 4: Run a manual CLI sanity check**

Run:

```bash
cargo run -- slice \
  --sv demo/trace_coverage_demo/design.sv \
  --sv demo/trace_coverage_demo/tb.sv \
  --vcd demo/trace_coverage_demo/logs/sim.vcd \
  --signal tb.dut.result \
  --time 40 \
  --min-time 0
```

Expected: JSON graph with `nodes`, `edges`, and referenced blocks.

**Step 5: Commit**

```bash
git add src tests Cargo.toml
git commit -m "chore: verify trait-based dataflow engine"
```

## Notes for the Implementer

- Prefer small helper structs over giant modules.
- If `sv-parser` AST traversal becomes unwieldy, isolate syntax-specific walkers in `src/ast/` and keep `src/block/dataflow.rs` focused on semantic extraction.
- Keep `wellen`-specific code confined to `src/wave/wellen.rs` and `src/coverage/vcd.rs`.
- Use fully qualified or normalized signal names early; inconsistent naming will break inter-block analysis.
- The custom minimum time bound is a required feature; do not hardcode `0`.
- If internal `petgraph` types are awkward to serialize, maintain a dedicated export DTO instead of exposing raw graph internals.
- Do not implement LLM integration in this phase.
