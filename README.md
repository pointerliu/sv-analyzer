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

cargo run -p sva_cli -- blocks query --input <blockize-json> [--block-id <id>] [--output-signal <name> ...] [--input-signal <name> ...] [--scope <prefix>] [--block-type <kind>] [--circuit-type <kind>] [--source-file <suffix>]

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

### Block Queries

- `blocks query` reads a saved `blockize` JSON file and returns full matching block JSON.
- All query properties are combined with logical AND.
- Repeated `--output-signal` and `--input-signal` flags require the block to contain every requested signal.
- `--scope` matches the exact `module_scope` or any hierarchical descendant.
- `--source-file` uses suffix matching, so `rtl/ibex_if_stage.sv` matches an absolute path ending in that suffix.

Example:

```bash
cargo run -p sva_cli -- blockize \
  --project-path /home/lzz/exp_wkdir/ibex_test/ibex/rtl \
  --sv /home/lzz/exp_wkdir/ibex_test/ibex/examples/simple_system/rtl/ibex_simple_system.sv \
  --include-paths /home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/ip/prim/rtl,/home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/dv/sv/dv_utils \
  > ibex_blocks.json

cargo run -p sva_cli -- blocks query \
  --input ibex_blocks.json \
  --scope TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i \
  --output-signal TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o \
  --source-file rtl/ibex_if_stage.sv
```

### Real Project Example

Ibex blockize with external include roots:

```bash
cargo run -p sva_cli -- blockize \
  --project-path /home/lzz/exp_wkdir/ibex_test/ibex/rtl \
  --include-paths /home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/ip/prim/rtl,/home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/dv/sv/dv_utils
```

Ibex static slice for the simple-system top-level:

```bash
cargo run -p sva_cli -- slice --static \
  --project-path /home/lzz/exp_wkdir/ibex_test/ibex/rtl \
  --sv /home/lzz/exp_wkdir/ibex_test/ibex/examples/simple_system/rtl/ibex_simple_system.sv \
  --include-paths /home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/ip/prim/rtl,/home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/dv/sv/dv_utils \
  --signal TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o \
  > ibex_static_slice.json
```

Ibex dynamic (blues) slice for the simple-system top-level:

```bash
cargo run -p sva_cli -- slice \
  --project-path /home/lzz/exp_wkdir/ibex_test/ibex/rtl \
  --sv /home/lzz/exp_wkdir/ibex_test/ibex/examples/simple_system/rtl/ibex_simple_system.sv \
  --include-paths /home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/ip/prim/rtl,/home/lzz/exp_wkdir/ibex_test/ibex/vendor/lowrisc_ip/dv/sv/dv_utils \
  --vcd /home/lzz/exp_wkdir/ibex_test/ibex/build/lowrisc_ibex_ibex_simple_system_cosim_0/sim-verilator/sim.fst \
  --signal TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o \
  --clock TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.clk_i \
  --clk-step 2 \
  --time 19 \
  --min-time 11 \
  > ibex_blues.json
```

The Ibex examples above use `--project-path` for `ibex/rtl`, and add
`examples/simple_system/rtl/ibex_simple_system.sv` explicitly with `--sv`
because that top-level wrapper is outside the scanned `rtl/` tree.

### Elaborated AST Dump (dead generate branches removed)

For tools that need the post-elaboration AST (with parameter-gated
`generate` branches pruned), run Verilator with `--json-only` against the
same file list used by the sim build. This stops Verilator after
`V3Begin`, so `GENIF` / `PARSEREF` nodes are gone and only the live body
of each `generate-if/else` survives:

```bash
# Starting from a fusesoc-generated Ibex build tree:
IBEX_BUILD=/home/lzz/exp_wkdir/ibex_test/ibex/build/lowrisc_ibex_ibex_simple_system_cosim_0/sim-verilator

# Reuse the VC file fusesoc produced, but strip C++-emission flags and
# append --json-only. The grep just drops --cc / --Mdir / --exe /
# CFLAGS / LDFLAGS / source-file lines so Verilator stops after elab.
grep -v '^--cc$\|^--Mdir\|^--exe\|^-CFLAGS\|^-LDFLAGS\|\.cc$\|\.cpp$\|\.c$\|\.o$' \
  $IBEX_BUILD/lowrisc_ibex_ibex_simple_system_cosim_0.vc > /tmp/ibex_jsononly.vc
cat <<'EOF' >> /tmp/ibex_jsononly.vc
--json-only
--no-json-ids
--Mdir /tmp/ibex_jsonout
--json-only-output /tmp/ibex_elab.tree.json
EOF

mkdir -p /tmp/ibex_jsonout
cd $IBEX_BUILD && verilator -f /tmp/ibex_jsononly.vc
```

Runs in well under a second on Ibex and produces two files:

- `/tmp/ibex_elab.tree.json` — live AST (`ASSIGNW`, `ASSIGNDLY`,
  `ALWAYS`, `INITIAL`, …). Every node carries a
  `loc:"<file_id>,<l1>:<c1>,<l2>:<c2>"` field; dead generate-if branches
  and non-instantiated parameter specializations of modules are absent.
- `/tmp/ibex_jsonout/Vibex_simple_system.tree.meta.json` — `files` map
  keyed by the two-letter `<file_id>` used in `loc`, with `filename` and
  `realpath` entries.

If you prefer to emit the dump alongside the normal `--cc` build rather
than as a separate step, add `--dump-tree-json --dumpi-tree-json 3
--no-json-ids` to the `verilator_options` block of
`dv/verilator/simple_system_cosim/ibex_simple_system_cosim.core` under
the `sim` target. The build then writes one `*.tree.json` per Verilator
pass; the file you want is the one produced after `V3Begin` (dead
branches removed).

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
