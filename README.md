# sv-analyzer

Block-level dataflow analysis engine for Verilog/SystemVerilog designs.

## Quick Start

```bash
cargo build
cargo test --all-targets -v
cargo run --bin sva_cli -- --help          # CLI help
```

## Crates

| Crate | Purpose |
|-------|---------|
| `sva-core` | Core library: AST, block, coverage, slicer, types, wave |
| `sva-cli` | CLI binary (`sva_cli`) |
| `sva-mcp` | MCP server |
| `sva-vscode` | VSCode extension backend |

## CLI Subcommands

```bash
cargo run --bin sva_cli -- blockize --sv <file>      # Parse HDL → block dataflow
cargo run --bin sva_cli -- slice --sv <file> --static --signal <name>   # Static slice
cargo run --bin sva_cli -- slice --sv <file> --vcd <file> --signal <name> --time <t> --min-time <t>  # Dynamic slice
cargo run --bin sva_cli -- coverage --vcd <file> --file <name> --line <n> --time <t>
cargo run --bin sva_cli -- wave --vcd <file> --signal <name> --time <t>
```

## Library Usage

```rust
use sva_core::ast::{AstProvider, SvParserProvider};
use sva_core::block::{Blockizer, DataflowBlockizer};
use sva_core::coverage::VcdCoverageTracker;
use sva_core::slicer::{BluesSlicer, SliceRequest, StaticSlicer};
use sva_core::types::{SignalNode, Timestamp};

let parsed = SvParserProvider.parse_files(&[...])?;
let blocks = DataflowBlockizer.blockize(&parsed)?;
let graph = StaticSlicer::new(blocks).slice(&SliceRequest { ... })?;
```

## Architecture

Trait-based core: `AstProvider`, `Blockizer`, `CoverageTracker`, `WaveformReader`, `Slicer`.

Implementations: `SvParserProvider`, `DataflowBlockizer`, `VcdCoverageTracker`, `WellenReader`, `StaticSlicer`, `BluesSlicer`.

## Dev Commands

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets -v
```

## ⚠️ Commands may be out of date

Run `cargo run --bin sva_cli -- <subcommand> --help` to see current usage.
