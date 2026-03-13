---
name: wellen
description: Use when reading, parsing, or processing VCD, FST, or GHW waveform files in Rust projects
---

# Wellen

High-performance Rust library for reading waveform files (VCD, FST, GHW) with lazy loading, multi-threading, and memory efficiency.

## When to Use

- Need to parse VCD, FST, or GHW waveform files in Rust
- Building verification tools, waveform viewers, or signal analysis
- Working with digital design verification data
- Need lazy loading (don't want to load entire file)
- Processing large waveforms with limited memory

**Not for:** Creating waveforms (wellen is read-only)

## Quick Reference

### Interfaces

| Interface | Use When |
|-----------|----------|
| `wellen::simple` | Batch processing, simple apps |
| `wellen::viewers` | GUI apps, two-phase loading (header then body) |
| `wellen::stream` | Large files, time-range filtering |

### Supported Formats

- **VCD** (.vcd) - Value Change Dump (IEEE 1364)
- **FST** (.fst) - Fast Signal Trace
- **GHW** (.ghw) - GHDL waveform

### Key Types

```rust
Hierarchy    // Signal metadata (scopes, variables)
Signal       // Time indices + data values
SignalRef    // Unique signal identifier
VarRef       // Variable reference in hierarchy
SignalValue  // Binary, FourValue, NineValue, String, Real
```

### Basic Usage

```rust
use wellen::simple::{self, Waveform};

// Read waveform
let mut waveform = simple::read("waveform.vcd")?;

// Access hierarchy
let hierarchy = waveform.hierarchy();

// Load specific signal
let signal_ids = vec![hierarchy.lookup_var(&[], "clk").unwrap()];
waveform.load_signals(&signal_ids);

// Read signal data
if let Some(signal) = waveform.get_signal(signal_ids[0]) {
    for (time_idx, value) in signal.iter_changes() {
        println!("Time {}: {}", time_idx, value);
    }
}
```

### Two-Phase Loading (Viewers)

```rust
use wellen::{viewers, LoadOptions};

// Phase 1: Header only (fast)
let header = viewers::read_header_from_file(filename, &options)?;

// Phase 2: Load signals on demand
let body = viewers::read_body(header.body, &hierarchy, None)?;
```

### Signal Value Types

```rust
SignalValue::Binary(..)    // 2-state (0, 1)
SignalValue::FourValue(..) // 4-state (0, 1, x, z)
SignalValue::NineValue(..)  // 9-state (0, 1, x, z, h, u, w, l, -)
SignalValue::String(..)    // String values
SignalValue::Real(..)      // Floating-point
```

### Compression

```rust
use wellen::compressed::{CompressedSignal, CompressedTimeTable};

// Compress signal for storage
let compressed = CompressedSignal::compress(signal);
let decompressed = compressed.uncompress();
```

## Installation

```toml
[dependencies]
wellen = "0.20"

# Optional features
wellen = { version = "0.20", features = ["serde1"] }
```

## Performance Tips

1. Load only needed signals (use viewers interface)
2. Enable multi-threaded parsing: `LoadOptions { multi_thread: true, .. }`
3. Unload unused signals: `waveform.unload_signals(&ids)`
4. Cache SignalRef values to avoid repeated lookups

## Detailed API

See [wellen-api.md](./wellen-api.md) for comprehensive API reference.
