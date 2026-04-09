# sv-analyzer

Block-level dataflow analysis engine for Verilog/SystemVerilog designs.

## Quick Start

```bash
cargo build
cargo test --all-targets -v
cargo run -p sva_cli -- --help
```

## Crates

| Crate | Purpose |
|-------|---------|
| `sva-core` | Core library: AST, block, coverage, slicer, types, wave |
| `sva-cli` | CLI crate (`sva_cli`), exposed as `dataflow-engine` in help/output |
| `sva-mcp` | MCP server |
| `sva-vscode` | VSCode extension backend |

## CLI Usage

```bash
cargo run -p sva_cli -- blockize --sv <file> [--sv <file> ...]
cargo run -p sva_cli -- blockize --project-path <dir> [--include-paths <dir1,dir2,...>]

cargo run -p sva_cli -- slice --static --sv <file> --signal <hierarchical-name>
cargo run -p sva_cli -- slice --static --project-path <dir> --signal <hierarchical-name> [--include-paths <dir1,dir2,...>]

cargo run -p sva_cli -- slice --sv <file> --vcd <file> --signal <hierarchical-name> --time <t> --min-time <t>
cargo run -p sva_cli -- coverage --sv <file> --vcd <file> --time <t>
cargo run -p sva_cli -- wave --vcd <file> --signal <hierarchical-name> --time <t>
```

### Source Selection

- Use `--sv` to pass one or more explicit SystemVerilog source files.
- Use `--project-path` to scan a larger source tree recursively.
- `--project-path` currently discovers `.sv` files only. It skips `.svh` and other non-source files when walking a directory.
- `--include-paths` is a comma-separated list of extra include directories passed to `sv_parser`.
- At least one source must be provided through `--sv` or `--project-path`.

### Notes

- Static and dynamic slice queries work best with hierarchical signal names in elaborated designs.
- If a signal name is ambiguous or missing, the CLI returns similar hierarchical names.
- The parser currently hardcodes the `RVFI` define for preprocessing.

### Real Project Example

Ibex blockize with external include roots:

```bash
cargo run -p sva_cli -- blockize \
  --project-path /home/lzz/exp_wkdir/ibex_test/ibex/rtl \
  --include-paths /home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/ip/prim/rtl,/home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/dv/sv/dv_utils
```

Ibex static slice with a fully qualified signal:

```bash
cargo run -p sva_cli -- slice --static \
  --project-path /home/lzz/exp_wkdir/ibex_test/ibex/rtl \
  --include-paths /home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/ip/prim/rtl,/home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/dv/sv/dv_utils \
  --signal TOP.ibex_top_tracing.u_ibex_top.u_ibex_core.ex_block_i.alu_i.adder_result_o
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

Run `cargo run -p sva_cli -- <subcommand> --help` to inspect the current CLI surface.
