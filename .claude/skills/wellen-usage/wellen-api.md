# Wellen API Reference

Detailed API documentation for the wellen crate.

## Simple Interface (`wellen::simple`)

### Functions

```rust
// Read a waveform file with default options
pub fn read<P: AsRef<std::path::Path>>(filename: P) -> Result<Waveform>

// Read with custom options
pub fn read_with_options<P: AsRef<std::path::Path>>(
    filename: P,
    options: &LoadOptions
) -> Result<Waveform>

// Read from any BufRead + Seek source
pub fn read_from_reader<R: BufRead + Seek + Send + Sync + 'static>(
    input: R
) -> Result<Waveform>
```

### Waveform Struct

```rust
pub struct Waveform {
    // Get the hierarchy containing all signal metadata
    pub fn hierarchy(&self) -> &Hierarchy
    
    // Get the time table (all simulation timestamps)
    pub fn time_table(&self) -> &[Time]
    
    // Load signals (single-threaded)
    pub fn load_signals(&mut self, ids: &[SignalRef])
    
    // Load signals (multi-threaded)
    pub fn load_signals_multi_threaded(&mut self, ids: &[SignalRef])
    
    // Unload signals to free memory
    pub fn unload_signals(&mut self, ids: &[SignalRef])
    
    // Get a loaded signal
    pub fn get_signal(&self, id: SignalRef) -> Option<&Signal>
    
    // Print backend statistics
    pub fn print_backend_statistics(&self)
}
```

## Viewers Interface (`wellen::viewers`)

### Key Types

```rust
pub struct HeaderResult<R: BufRead + Seek> {
    pub hierarchy: Hierarchy,
    pub file_format: FileFormat,
    pub body_len: u64,
    pub body: ReadBodyContinuation<R>,
}

pub struct BodyResult {
    pub source: SignalSource,
    pub time_table: TimeTable,
}
```

### Functions

```rust
// Read header from file (faster - doesn't load signal data)
pub fn read_header_from_file<P: AsRef<std::path::Path>>(
    filename: P,
    options: &LoadOptions
) -> Result<HeaderResult<...>>

// Read header from any source
pub fn read_header<R: BufRead + Seek>(
    input: R,
    options: &LoadOptions
) -> Result<HeaderResult<R>>

// Read body (signal data) - call after reading header
pub fn read_body<R: BufRead + Seek + Sync + Send + 'static>(
    body: ReadBodyContinuation<R>,
    hierarchy: &Hierarchy,
    progress: Option<ProgressCount>
) -> Result<BodyResult>

// Detect file format
pub fn detect_file_format(input: &mut (impl BufRead + Seek)) -> FileFormat
pub fn open_and_detect_file_format<P: AsRef<std::path::Path>>(filename: P) -> FileFormat
```

## Stream Interface (`wellen::stream`)

```rust
use wellen::stream::{self, StreamingWaveform, Filter};

fn process_streaming(filename: &str) -> Result<(), wellen::WellenError> {
    let options = LoadOptions::default();
    let mut waveform: StreamingWaveform<_> = stream::read_from_file(filename, &options)?;
    
    let hierarchy = waveform.hierarchy();
    
    // Process signals in a time range
    let filter = Filter::new(0, 1000, &[]); // time 0 to 1000
    let changes = waveform.read_filtered_changes(filter)?;
    
    for (signal_ref, change) in changes {
        println!("Signal {:?}: {:?} at time {}", signal_ref, change.value, change.time);
    }
    
    Ok(())
}
```

## Working with Hierarchy

### Accessing Scopes and Variables

```rust
use wellen::{Hierarchy, ScopeRef, VarRef, ScopeOrVarRef};

// Iterate top-level items
for item in hierarchy.items() {
    match item {
        ScopeOrVarRef::Scope(scope_ref) => {
            let scope = &hierarchy[scope_ref];
            println!("Scope: {}", scope.name(hierarchy));
        }
        ScopeOrVarRef::Var(var_ref) => {
            let var = &hierarchy[var_ref];
            println!("Variable: {}", var.name(hierarchy));
        }
    }
}

// Get all scopes (recursively)
for scope_ref in hierarchy.scopes() {
    let scope = &hierarchy[scope_ref];
    println!("Scope: {} (type: {:?})", scope.full_name(hierarchy), scope.scope_type());
}

// Get all variables (recursively)
for var_ref in hierarchy.vars() {
    let var = &hierarchy[var_ref];
    println!("Variable: {} (type: {:?})", var.full_name(hierarchy), var.var_type());
}
```

### Looking Up Variables

```rust
// Simple lookup by name
let var_ref = hierarchy.lookup_var(&[], "clock_signal");

// Lookup in a specific scope
let var_ref = hierarchy.lookup_var(&["top", "cpu"], "data_out");

// Lookup with index (for arrays/buses)
use wellen::VarIndex;
let var_ref = hierarchy.lookup_var_with_index(
    &["top", "memory"], 
    "data", 
    &Some(VarIndex::new(7, 0))  // [7:0]
);
```

### Hierarchy Metadata

```rust
let hierarchy = waveform.hierarchy();

// Global metadata
println!("Timescale: {:?}", hierarchy.timescale());
println!("Date: {}", hierarchy.date());
println!("Version: {}", hierarchy.version());
println!("File format: {:?}", hierarchy.file_format());

// Memory usage
println!("Hierarchy size: {} bytes", hierarchy.size_in_memory());
```

## Working with Signals

### Loading Signals

```rust
// Get signal references from hierarchy
let signal_ids: Vec<SignalRef> = hierarchy.vars()
    .map(|v| hierarchy[v].signal_ref())
    .collect();

// Load signals (single-threaded)
waveform.load_signals(&signal_ids);

// Or multi-threaded (faster for many signals)
waveform.load_signals_multi_threaded(&signal_ids);

// Load specific signals
let specific_ids = vec![
    hierarchy.lookup_var(&[], "clock").unwrap(),
    hierarchy.lookup_var(&[], "reset").unwrap(),
];
waveform.load_signals(&specific_ids);
```

### Accessing Signal Data

```rust
if let Some(signal) = waveform.get_signal(signal_ref) {
    // Get signal reference
    let idx = signal.signal_ref();
    
    // Iterate through all value changes
    for (time_table_idx, value) in signal.iter_changes() {
        println!("At time index {}: {}", time_table_idx, value);
    }
    
    // Get time indices
    let indices = signal.time_indices();
    
    // Memory usage
    println!("Signal size: {} bytes", signal.size_in_memory());
}
```

### Getting Values at Specific Times

```rust
use wellen::{Signal, TimeTableIdx};

// Get value at a specific time table index
if let Some(offset) = signal.get_offset(time_table_idx) {
    let value = signal.get_value_at(&offset, 0);
    let actual_time_idx = signal.get_time_idx_at(&offset);
    println!("Value: {} at time index {}", value, actual_time_idx);
}

// Get first time index
if let Some(first_idx) = signal.get_first_time_idx() {
    println!("First change at time index: {}", first_idx);
}
```

## Signal Values

### SignalValue Types

```rust
use wellen::SignalValue;

for (_, value) in signal.iter_changes() {
    match value {
        // Binary (2-state) values
        SignalValue::Binary(data, bits) => {
            let bitstring = value.to_bit_string().unwrap();
            println!("Binary: {} ({} bits)", bitstring, bits);
        }
        
        // Four-state values (0, 1, x, z)
        SignalValue::FourValue(data, bits) => {
            println!("Four-state: {}", value);
        }
        
        // Nine-state values (0, 1, x, z, h, u, w, l, -)
        SignalValue::NineValue(data, bits) => {
            println!("Nine-state: {}", value);
        }
        
        // String values
        SignalValue::String(s) => {
            println!("String: {}", s);
        }
        
        // Real/floating-point values
        SignalValue::Real(r) => {
            println!("Real: {}", r);
        }
        
        // Event (no data)
        SignalValue::Event => {
            println!("Event");
        }
    }
    
    // Helper methods
    if value.is_event() { /* ... */ }
    if let Some(bits) = value.bits() { /* ... */ }
}
```

### Signal Encoding

```rust
use wellen::{SignalEncoding, VarType, VarDirection};

// Check encoding type
match var.signal_encoding() {
    SignalEncoding::BitVector(width) => println!("BitVector of width {}", width.get()),
    SignalEncoding::Real => println!("Real number"),
    SignalEncoding::String => println!("String"),
    SignalEncoding::Event => println!("Event"),
}

// Check variable properties
println!("Is real: {}", var.is_real());
println!("Is 1-bit: {}", var.is_1bit());
println!("Length: {:?}", var.length());
println!("Type: {:?}", var.var_type());
println!("Direction: {:?}", var.direction());

// For arrays
if let Some(index) = var.index() {
    println!("Array index: [{}:{}]", index.msb(), index.lsb());
}
```

## Compression

### Compressed Signals

```rust
use wellen::compressed::{CompressedSignal, Compression};

let signal = waveform.get_signal(signal_ref).unwrap();

// Compress a signal for storage/transmission
let compressed = CompressedSignal::compress(signal);

// Decompress when needed
let decompressed = compressed.uncompress();
```

### Compressed Time Table

```rust
use wellen::compressed::CompressedTimeTable;

// Compress time table
let compressed = CompressedTimeTable::compress(&waveform.time_table());

// Decompress
let decompressed = compressed.decompress();
```

## Load Options

```rust
use wellen::LoadOptions;

let options = LoadOptions {
    // Enable multi-threaded parsing (default: true)
    // Useful for VCD files; FST is always single-threaded for header
    multi_thread: true,
    
    // Remove scopes with empty names (default: false)
    remove_scopes_with_empty_name: false,
};
```

## Error Handling

### WellenError Enum

```rust
use wellen::WellenError;

match error {
    WellenError::FailedToLoad(format, message) => {
        eprintln!("Failed to load {:?} file: {}", format, message);
    }
    WellenError::UnknownFileFormat => {
        eprintln!("Unknown waveform file format");
    }
    WellenError::Io(io_error) => {
        eprintln!("IO error: {}", io_error);
    }
}
```

## Additional Types

### Timescale

```rust
use wellen::{Timescale, TimescaleUnit};

// Create timescale
let ts = Timescale::new(1, TimescaleUnit::NanoSeconds);

// Access components
println!("Factor: {}", ts.factor);
println!("Unit: {:?}", ts.unit);

// Get exponent for calculations
if let Some(exp) = ts.unit.to_exponent() {
    println!("Exponent: {}", exp);  // -9 for nanoseconds
}
```

### Scope Types

```rust
use wellen::ScopeType;

// Various scope types
ScopeType::Module
ScopeType::Task
ScopeType::Function
ScopeType::Begin
ScopeType::Fork
ScopeType::Generate
ScopeType::Struct
ScopeType::Union
ScopeType::Class
ScopeType::Interface
ScopeType::Package
ScopeType::VhdlArchitecture
ScopeType::VhdlProcess
// ... and more
```

### Variable Types

```rust
use wellen::VarType;

// VCD types
VarType::Reg
VarType::Wire
VarType::Integer
VarType::Real
VarType::Parameter
VarType::String
// ... and more

// Check if parameter
var.var_type().is_parameter()  // checks for Parameter or RealParameter
```

## Real-world Example (Surfer)

The [Surfer](https://github.com/ekiwi/surfer) waveform viewer is a real-world application that uses wellen:

```rust
use wellen::{
    FileFormat, Hierarchy, LoadOptions, ScopeType, Signal, SignalEncoding, 
    SignalRef, SignalSource, Time, TimeTable, TimeTableIdx, Timescale, 
    TimescaleUnit, Var, VarRef, VarType, viewers
};

pub struct WellenContainer {
    hierarchy: Arc<Hierarchy>,
    signals: HashMap<SignalRef, Arc<Signal>>,
    time_table: Arc<TimeTable>,
    source: Option<SignalSource>,
}

impl WellenContainer {
    pub fn from_file(filename: &str) -> Result<Self> {
        let options = LoadOptions::default();
        
        // Read header
        let header = viewers::read_header_from_file(filename, &options)?;
        let hierarchy = Arc::new(header.hierarchy);
        
        // Read body
        let body = viewers::read_body(header.body, &hierarchy, None)?;
        
        Ok(WellenContainer {
            hierarchy,
            signals: HashMap::new(),
            time_table: Arc::new(body.time_table),
            source: Some(body.source),
        })
    }
    
    pub fn load_variables(&mut self, var_refs: &[VarRef]) -> Result<()> {
        let signal_refs: Vec<SignalRef> = var_refs
            .iter()
            .map(|r| self.hierarchy[*r].signal_ref())
            .collect();
        
        if let Some(source) = self.source.take() {
            let loaded = source.load_signals(&signal_refs, &self.hierarchy, false);
            for (id, signal) in loaded {
                self.signals.insert(id, Arc::new(signal));
            }
        }
        Ok(())
    }
    
    pub fn query_variable(&self, var_ref: &VarRef, time: u64) -> Result<Option<SignalValue>> {
        let signal_ref = self.hierarchy[*var_ref].signal_ref();
        let signal = self.signals.get(&signal_ref)?;
        
        // Binary search for time index
        let idx = binary_search(&self.time_table, time);
        
        if let Some(offset) = signal.get_offset(idx) {
            return Ok(Some(signal.get_value_at(&offset, 0)));
        }
        
        Ok(None)
    }
}
```
