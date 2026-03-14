# 🧠 Dataflow Engine

A block-level dataflow analysis engine on Verilog/SystemVerilog designs.

It uses:

- 🌳 `sv-parser` to parse Verilog/SystemVerilog into ASTs
- 🌊 `wellen` to read VCD waveforms
- 🔎 static slicing for timeless dependency graphs
- ⏱️ Block-Level Instruction-Oriented Slicing (Blues algorithm)
- 📈 Verilator `--trace-coverage` counters inside VCDs to get coverage information

The repository contains both a reusable Rust library and a CLI for common workflows.

## ✨ What this repo does

This project turns HDL source code plus optional waveform/coverage information into graph-shaped dataflow results.

At a high level, it can:

- parse one or more `.v` / `.sv` / `.svh` source files
- extract statement-level input/output dependencies
- group statements into analysis blocks
- build a static slice graph for a target signal
- build a dynamic slice graph (Blues) for a target signal at a specific time
- query trace coverage counters from a VCD
- query signal values from a waveform
- export slice results as stable JSON

## 🏗️ Architecture in one minute

The core design goal is swappability.

Main subsystems are trait-based:

- `AstProvider` - parse HDL source files
- `Blockizer` - convert parsed source into block-level dataflow units
- `CoverageTracker` - answer coverage questions at annotation times
- `WaveformReader` - answer signal-value questions from waveforms
- `Slicer` - compute backward slices

Current built-in implementations are:

- `SvParserProvider`
- `DataflowBlockizer`
- `VcdCoverageTracker`
- `WellenReader`
- `StaticSlicer`
- `BluesSlicer`

This keeps the repo easy to extend if you later want to plug in a different parser, waveform backend, or slicing strategy.

## 🚀 Getting started

### Requirements

- Rust toolchain with `cargo`
- Verilator
- VCD files for `wave`, `coverage`, and dynamic `slice`

### Build

```bash
cargo build
```

### Run the test suite

```bash
cargo test --all-targets -v
```

### Show CLI help

```bash
cargo run -- --help
```

## 🧰 CLI overview

The binary name is `dataflow-engine`.

When using `cargo`, invoke it like this:

```bash
cargo run -- <subcommand> ...
```

Available subcommands:

- `blockize` - parse HDL and emit blockized dataflow units as JSON
- `slice` - compute a static or dynamic slice graph
- `coverage` - query Verilator trace-coverage counters from a VCD
- `wave` - query a signal value from a waveform

## 🌳 `blockize`

Turn HDL source files into block-level dataflow units.

### Help

```bash
cargo run -- blockize --help
```

### Usage

```bash
cargo run -- blockize --sv path/to/design.sv
```

Multiple source files are supported by repeating `--sv`:

```bash
cargo run -- blockize \
  --sv path/to/design.sv \
  --sv path/to/tb.sv
```

### Output shape

`blockize` prints JSON with two top-level fields:

- `blocks` - array of discovered blocks
- `signal_to_drivers` - map from signal name to block ids

Each block includes information such as:

- block id
- block type (`ModInput`, `ModOutput`, `Assign`, `Always`)
- circuit type (`Combinational`, `Sequential`)
- source file and line range
- input signals and output signals
- statement-level dataflow entries

Dataflow entries use typed signal objects:

- `kind: "variable"` for HDL identifiers that can be traced backward
- `kind: "literal"` for constants such as `8'h0`, `1'b0`, or `1`
- `locate` to preserve the original source offset, line, and token length

## 🔎 `slice`

Compute backward slices rooted at a target signal.

There are two modes:

- 🧱 static slice: timeless dependency graph
- ⏱️ dynamic slice: Blues-style time-annotated graph

### Help

```bash
cargo run -- slice --help
```

### Static slice

Static slicing does not need waveform input.

```bash
cargo run -- slice \
  --static \
  --sv demo/trace_coverage_demo/design.sv \
  --sv demo/trace_coverage_demo/tb.sv \
  --signal result
```

Static slice output:

- uses the shared stable graph JSON format
- block nodes do **not** include `time`
- literal leaf nodes may appear without a `block_id`
- edges represent dataflow dependencies between blocks/statements and may terminate at literals

### Dynamic slice

Dynamic slicing requires a VCD plus a target time window.

```bash
cargo run -- slice \
  --sv demo/trace_coverage_demo/design.sv \
  --sv demo/trace_coverage_demo/tb.sv \
  --vcd demo/trace_coverage_demo/logs/sim.vcd \
  --signal result \
  --time 40 \
  --min-time 0
```

Dynamic slice output:

- uses the same stable JSON graph format as static slicing
- block and literal nodes include `time`
- edges may carry the propagating signal name
- literal leaf edges omit `signal` and terminate traversal

### Stable slice JSON format

Both slicers emit a stable graph object with:

```json
{
  "nodes": [
    { "kind": "block", "id": 0, "block_id": 5, "time": 10 },
    {
      "kind": "literal",
      "id": 1,
      "signal": {
        "kind": "literal",
        "name": "8'h0",
        "locate": { "offset": 1713, "line": 55, "len": 1 }
      },
      "time": 9
    }
  ],
  "edges": [
    {
      "from": 0,
      "to": 1,
      "signal": {
        "kind": "variable",
        "name": "tmp",
        "locate": { "offset": 0, "line": 0, "len": 3 }
      }
    },
    { "from": 1, "to": 0 }
  ],
  "blocks": [
    { "id": 5, "scope": "dut", "block_type": "Always" }
  ]
}
```

Notes:

- block nodes carry `block_id`; literal nodes carry a `signal` object instead
- `nodes[*].time` is omitted for static slices
- `edges[*].from` and `edges[*].to` reference node ids, not raw block ids
- `edges[*].signal` is omitted for terminal literal edges
- `blocks` gives metadata for the block ids used in the graph

## 📈 `coverage`

Query Verilator `--trace-coverage` counters embedded in a VCD.

### Help

```bash
cargo run -- coverage --help
```

### Basic usage

```bash
cargo run -- coverage \
  --vcd demo/trace_coverage_demo/logs/sim.vcd \
  --file design \
  --line 35 \
  --time 12
```

Example output:

```json
{
  "file": "design",
  "line": 35,
  "time": 12,
  "hit_count": 1,
  "delta_hits": 1,
  "is_covered": true
}
```

### Clock-aware annotation mode

For dynamic analysis, this repo supports a clock-aware annotation timeline.

Use:

- `--clock-signal <name>`
- `--clk-step <n>`

Example:

```bash
cargo run -- coverage \
  --vcd demo/trace_coverage_demo/logs/sim.vcd \
  --file design \
  --line 35 \
  --time 100 \
  --clock-signal tb.clk \
  --clk-step 100
```

Important semantics:

- the analysis timeline is annotation-based, not raw-VCD-time-based
- annotation points advance by `clk_step` on each detected positive clock edge
- coverage at time `t` is determined from counter **delta**, not absolute counter value
- if a line has count `k` at `t` and still `k` at `t + clk_step`, then it is treated as **not covered** at the later annotation

## 🌊 `wave`

Query the value of a signal from a VCD.

### Help

```bash
cargo run -- wave --help
```

### Usage

```bash
cargo run -- wave \
  --vcd demo/trace_coverage_demo/logs/sim.vcd \
  --signal tb.dut.state \
  --time 10
```

Example output:

```json
{
  "signal": "tb.dut.state",
  "time": 10,
  "value": {
    "raw_bits": "0011",
    "pretty_hex": "0x3"
  }
}
```

Signal lookup behavior:

- full hierarchical names are supported
- suffix aliases may also resolve if they are unambiguous
- ambiguous aliases intentionally return `null` / no value
- exact full-name matches win over suffix aliases

## 🧪 Demo assets

The repo includes `demo/trace_coverage_demo/`, which is useful for experimentation.

You will find:

- HDL source files
- a testbench
- helper scripts
- generated logs including `logs/sim.vcd`

Example files:

- `demo/trace_coverage_demo/design.sv`
- `demo/trace_coverage_demo/tb.sv`
- `demo/trace_coverage_demo/logs/sim.vcd`
- `demo/trace_coverage_demo/coverage_at_time.py`

## 🧠 Core concepts

### Blocks

The analysis first extracts statement-level dependencies, then groups them into analysis blocks.

Current block types are:

- `ModInput`
- `ModOutput`
- `Assign`
- `Always`

Each block records:

- module scope
- source location
- input signals
- output signals
- per-output dataflow entries

Signal objects are typed:

- variable signals can be traced through `signal_to_drivers`
- literals are preserved in dataflow and slice graphs as terminal leaves
- literals intentionally do not participate in backward driver lookup

### Static vs dynamic slicing

Static slicing answers:

> “Which blocks can affect this signal in the design?”

Dynamic slicing answers:

> “Which blocks affected this signal at this specific analysis time?”

Static slices are timeless.

Dynamic slices are time-annotated and use waveform / coverage information to decide which sequential dependencies are active.

### Coverage timeline semantics

When using clock-aware coverage:

- raw VCD timestamps are sampled at positive clock edges
- user-facing annotation times advance by `clk_step`
- coverage is interpreted through counter deltas between annotations

This makes the slicing logic independent of arbitrary raw timestamp spacing in the VCD.

## 🧩 Library usage

You can also use the crate as a library.

Main exported modules:

- `dac26_mcp::ast`
- `dac26_mcp::block`
- `dac26_mcp::coverage`
- `dac26_mcp::slicer`
- `dac26_mcp::types`
- `dac26_mcp::wave`

Typical flow in Rust:

```rust
use std::sync::Arc;

use dac26_mcp::ast::{AstProvider, SvParserProvider};
use dac26_mcp::block::{Blockizer, DataflowBlockizer};
use dac26_mcp::coverage::VcdCoverageTracker;
use dac26_mcp::slicer::{BluesSlicer, SliceRequest, StaticSlicer};
use dac26_mcp::types::{SignalNode, Timestamp};

fn main() -> anyhow::Result<()> {
    let parsed = SvParserProvider.parse_files(&vec!["design.sv".into(), "tb.sv".into()])?;
    let block_set = DataflowBlockizer.blockize(&parsed)?;

    let static_graph = StaticSlicer::new(block_set.clone())
        .slice(&SliceRequest {
            signal: SignalNode::named("result"),
            time: Timestamp(0),
            min_time: Timestamp(0),
        })?
        .stable_json_graph()?;

    let coverage = Arc::new(VcdCoverageTracker::open("sim.vcd")?);
    let dynamic_graph = BluesSlicer::new(block_set, coverage)
        .slice(&SliceRequest {
            signal: SignalNode::named("result"),
            time: Timestamp(40),
            min_time: Timestamp(0),
        })?
        .stable_json_graph()?;

    println!("static nodes: {}", static_graph.nodes.len());
    println!("dynamic nodes: {}", dynamic_graph.nodes.len());
    Ok(())
}
```

## 🔬 Development notes

Useful commands:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets -v
```

## ⚠️ Current limitations

- the CLI help text is still minimal and scaffold-like
- the project focuses on block-level graph results, not LLM-related pieces from earlier prototypes
- waveform and coverage handling currently targets VCD-backed workflows
- many examples rely on the included demo assets or temporary fixtures created in tests

## 📚 Related files

- `SPEC.md` - original project intent and requirements
- `docs/plans/2026-03-13-dataflow-engine-implementation.md` - implementation plan and task breakdown
- `demo/trace_coverage_demo/` - demo design and generated waveform assets

## 🤝 Contributing

If you extend the repo, try to preserve the current design direction:

- keep core systems trait-based and swappable
- prefer stable JSON output for graph results
- keep static and dynamic slicing on the same conceptual graph model
- verify changes with formatting, clippy, and integration tests

## 📜 License

No license file is present yet. Add one before distributing the project outside your current environment. 📝
