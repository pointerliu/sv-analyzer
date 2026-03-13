# sv-parser: SystemVerilog Parser for Rust

`sv-parser` is a Rust library for parsing SystemVerilog source code into an Abstract Syntax Tree (AST). It provides both parsing and preprocessing capabilities, making it suitable for building linters, language servers, code analysis tools, and other development tools.

## Table of Contents

1. [Installation](#installation)
2. [Quick Start](#quick-start)
3. [Core Concepts](#core-concepts)
4. [API Reference](#api-reference)
   - [Parsing Functions](#parsing-functions)
   - [SyntaxTree](#syntaxtree)
   - [Preprocessing](#preprocessing)
   - [Macros](#macros)
5. [AST Traversal](#ast-traversal)
6. [Working with Defines](#working-with-defines)
7. [Error Handling](#error-handling)
8. [Examples from Real Projects](#examples-from-real-projects)
   - [Example: Module List](#example-module-list)
   - [Example: Building a Linter Rule](#example-building-a-linter-rule)
   - [Example: Language Server Integration](#example-language-server-integration)
9. [Complete Usage Example](#complete-usage-example)

---

## Installation

Add `sv-parser` to your `Cargo.toml`:

```toml
[dependencies]
sv-parser = "0.13"
```

---

## Quick Start

The simplest way to parse a SystemVerilog file:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use sv_parser::parse_sv;

let path = PathBuf::from("design.sv");
let defines = HashMap::new();  // No predefined macros
let includes: Vec<PathBuf> = Vec::new();

match parse_sv(&path, &defines, &includes, false, false) {
    Ok((syntax_tree, _defines)) => {
        println!("Parse successful!");
        // Process the AST...
    }
    Err(e) => {
        println!("Parse failed: {:?}", e);
    }
}
```

---

## Core Concepts

### SyntaxTree

The `SyntaxTree` is the main structure returned by the parser. It contains:

- **node**: The root AST node (`AnyNode`)
- **text**: The preprocessed source text (`PreprocessedText`)

```rust
pub struct SyntaxTree {
    node: AnyNode,
    text: PreprocessedText,
}
```

### RefNode

`RefNode` is an enum representing all possible AST node types in SystemVerilog. Some common variants include:

- **ModuleDeclarationAnsi**, **ModuleDeclarationNonansi**: Module declarations
- **AlwaysConstruct**: Always blocks
- **InitialConstruct**: Initial blocks
- **ContinuousAssignment**: Assign statements
- **ModuleInstantiation**: Module instantiations
- **InterfaceDeclaration**: Interface declarations
- **ClassDeclaration**: Class definitions
- And many more...

### NodeEvent

When traversing the AST, you receive `NodeEvent` values indicating when you enter or leave a node:

```rust
pub enum NodeEvent<'a> {
    Enter(RefNode<'a>),
    Leave(RefNode<'a>),
}
```

### Locate

`Locate` stores position information for source code elements:

```rust
pub struct Locate {
    pub offset: usize,  // Byte offset in source
    pub line: u32,      // Line number (1-based)
    pub len: usize,     // Length in bytes
}
```

---

## API Reference

### Parsing Functions

#### `parse_sv`

Parse a SystemVerilog file from a file path:

```rust
pub fn parse_sv<T: AsRef<Path>, U: AsRef<Path>, V: BuildHasher>(
    path: T,
    pre_defines: &Defines<V>,
    include_paths: &[U],
    ignore_include: bool,
    allow_incomplete: bool,
) -> Result<(SyntaxTree, Defines), Error>
```

**Parameters:**
- `path`: Path to the SystemVerilog source file
- `pre_defines`: Predefined macros (from command-line `-D` options)
- `include_paths`: Directories to search for `include files
- `ignore_include`: If true, ignore `include directives
- `allow_incomplete`: If true, allow parsing incomplete code (for IDE support)

**Returns:** `Result<(SyntaxTree, Defines), Error>`

#### `parse_sv_str`

Parse SystemVerilog from a string (useful for IDE/language servers):

```rust
pub fn parse_sv_str<T: AsRef<Path>, U: AsRef<Path>, V: BuildHasher>(
    s: &str,
    path: T,
    pre_defines: &Defines<V>,
    include_paths: &[U],
    ignore_include: bool,
    allow_incomplete: bool,
) -> Result<(SyntaxTree, Defines), Error>
```

#### `parse_lib` / `parse_lib_str`

Parse library mapping files (with `include` statements):

```rust
pub fn parse_lib<T: AsRef<Path>, U: AsRef<Path>, V: BuildHasher>(
    path: T,
    pre_defines: &Defines<V>,
    include_paths: &[U],
    ignore_include: bool,
    allow_incomplete: bool,
) -> Result<(SyntaxTree, Defines), Error>
```

#### `parse_sv_pp` / `parse_lib_pp`

Parse from pre-processed text:

```rust
pub fn parse_sv_pp(
    text: PreprocessedText,
    defines: Defines,
    allow_incomplete: bool,
) -> Result<(SyntaxTree, Defines), Error>
```

### SyntaxTree Methods

#### `get_str`

Get the original source text for a node:

```rust
pub fn get_str<'a, T: Into<RefNodes<'a>>>(&self, nodes: T) -> Option<&str>
```

Example:
```rust
if let Some(module_id) = unwrap_node!(x, ModuleIdentifier) {
    let name = syntax_tree.get_str(&module_id).unwrap();
    println!("Module: {}", name);
}
```

#### `get_str_trim`

Get source text without trailing whitespace:

```rust
pub fn get_str_trim<'a, T: Into<RefNodes<'a>>>(&self, nodes: T) -> Option<&str>
```

#### `get_origin`

Get the source file location of a `Locate`:

```rust
pub fn get_origin(&self, locate: &Locate) -> Option<(&PathBuf, usize)>
```

Returns: `Option<(&PathBuf, usize)>` - (file path, byte offset)

### Preprocessing

The preprocessor handles:
- `define` and `undef`
- `include` files
- `ifdef` / `ifndef` / `elsif` / `else` / `endif`
- `__FILE__`, `__LINE__`

#### `preprocess`

Run preprocessing only (without parsing):

```rust
pub fn preprocess<T: AsRef<Path>, U: AsRef<Path>, V: BuildHasher>(
    path: T,
    pre_defines: &Defines<V>,
    include_paths: &[U],
    strip_comments: bool,
    ignore_include: bool,
) -> Result<(PreprocessedText, Defines), Error>
```

#### `preprocess_str`

Preprocess a string:

```rust
pub fn preprocess_str<T: AsRef<Path>, U: AsRef<Path>, V: BuildHasher>(
    s: &str,
    path: T,
    pre_defines: &Defines<V>,
    include_paths: &[U],
    ignore_include: bool,
    strip_comments: bool,
    resolve_depth: usize,
    include_depth: usize,
) -> Result<(PreprocessedText, Defines), Error>
```

### Macros

#### `unwrap_node!`

Extract a specific node type from an iterator:

```rust
unwrap_node!(nodes, ModuleIdentifier)
unwrap_node!(nodes, SimpleIdentifier, EscapedIdentifier)  // Multiple types
```

Returns `Option<RefNode::<Type>>`.

#### `unwrap_locate!`

Extract a `Locate` from a node:

```rust
let locate = unwrap_locate!(node);
```

---

## AST Traversal

### Basic Traversal

Iterate over all nodes in the AST:

```rust
for node in &syntax_tree {
    match node {
        RefNode::ModuleDeclarationAnsi(x) => {
            // Handle ANSI module declaration
        }
        RefNode::ModuleDeclarationNonansi(x) => {
            // Handle non-ANSI module declaration
        }
        RefNode::AlwaysConstruct(x) => {
            // Handle always block
        }
        _ => {}
    }
}
```

### Event-Based Traversal

For more control, use event-based iteration:

```rust
for event in syntax_tree.into_iter().event() {
    match event {
        NodeEvent::Enter(node) => {
            println!("Entering: {:?}", node);
        }
        NodeEvent::Leave(node) => {
            println!("Leaving: {:?}", node);
        }
    }
}
```

### Finding Child Nodes

Use `unwrap_node!` to find specific children:

```rust
RefNode::ModuleDeclarationAnsi(x) => {
    // x contains the module declaration nodes
    // Find the module identifier
    if let Some(id) = unwrap_node!(&x, ModuleIdentifier) {
        let name = syntax_tree.get_str(&id).unwrap();
    }
    
    // Find all port declarations
    for child in x {
        match child {
            RefNode::PortDeclaration(_) => { /* ... */ }
            _ => {}
        }
    }
}
```

---

## Working with Defines

### Creating Defines

```rust
use sv_parser::{Define, DefineText};
use std::collections::HashMap;

// Simple define (no value)
let mut defines = HashMap::new();
let define = Define::new("DEBUG".to_string(), vec![], None);
defines.insert("DEBUG".to_string(), Some(define));

// Define with value
let text = DefineText::new("1".to_string(), None);
let define = Define::new("DATA_WIDTH".to_string(), vec![], Some(text));
defines.insert("DATA_WIDTH".to_string(), Some(define));

// Define with arguments (function-like macro)
let args = vec![("WIDTH".to_string(), None)];
let text = DefineText::new("WIDTH * 2".to_string(), None);
let define = Define::new("DOUBLE".to_string(), args, Some(text));
defines.insert("DOUBLE".to_string(), Some(define));
```

### Using Defines in Parsing

```rust
use std::collections::HashMap;
use sv_parser::parse_sv;

let mut defines = HashMap::new();
// Add defines as shown above

let includes = vec!["../common".into(), "./includes".into()];
let result = parse_sv("design.sv", &defines, &includes, false, false);
```

---

## Error Handling

### Parse Errors

```rust
use sv_parser::parse_sv;
use sv_parser_error::Error;

match parse_sv(&path, &defines, &includes, false, false) {
    Ok((syntax_tree, new_defines)) => {
        // Success
    }
    Err(Error::Parse(Some((origin_path, origin_pos)))) => {
        eprintln!("Parse error at {}:{}", 
            origin_path.display(), 
            origin_pos);
    }
    Err(Error::Parse(None)) => {
        eprintln!("Parse error (position unknown)");
    }
    Err(Error::File { source, path }) => {
        eprintln!("File error: {} - {}", path.display(), source);
    }
}
```

### Error Type Definition

```rust
pub enum Error {
    Parse(Option<(PathBuf, usize)>),  // (file path, byte offset)
    File {
        source: std::io::Error,
        path: PathBuf,
    },
}
```

---

## Examples from Real Projects

### Example: Module List

This example from `sv-parser` shows how to extract all module names from a SystemVerilog file:

```rust
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use sv_parser::{parse_sv, unwrap_node, Locate, RefNode};

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = PathBuf::from(&args[1]);
    let defines = HashMap::new();
    let includes: Vec<PathBuf> = Vec::new();

    let result = parse_sv(&path, &defines, &includes, false, false);

    if let Ok((syntax_tree, _)) = result {
        for node in &syntax_tree {
            match node {
                RefNode::ModuleDeclarationNonansi(x) => {
                    let id = unwrap_node!(x, ModuleIdentifier).unwrap();
                    let id = get_identifier(id).unwrap();
                    let id = syntax_tree.get_str(&id).unwrap();
                    println!("module: {}", id);
                }
                RefNode::ModuleDeclarationAnsi(x) => {
                    let id = unwrap_node!(x, ModuleIdentifier).unwrap();
                    let id = get_identifier(id).unwrap();
                    let id = syntax_tree.get_str(&id).unwrap();
                    println!("module: {}", id);
                }
                _ => (),
            }
        }
    }
}

fn get_identifier(node: RefNode) -> Option<Locate> {
    match unwrap_node!(node, SimpleIdentifier, EscapedIdentifier) {
        Some(RefNode::SimpleIdentifier(x)) => Some(x.nodes.0),
        Some(RefNode::EscapedIdentifier(x)) => Some(x.nodes.0),
        _ => None,
    }
}
```

### Example: Building a Linter Rule

This example from `svlint` shows how to create a syntax rule that checks for loop statements inside `always_ff` blocks:

```rust
use crate::config::ConfigOption;
use crate::linter::{SyntaxRule, SyntaxRuleResult};
use sv_parser::{unwrap_locate, unwrap_node, AlwaysKeyword, NodeEvent, RefNode, SyntaxTree};

pub struct LoopStatementInAlwaysFf;

impl SyntaxRule for LoopStatementInAlwaysFf {
    fn check(
        &mut self,
        _syntax_tree: &SyntaxTree,
        event: &NodeEvent,
        _option: &ConfigOption,
    ) -> SyntaxRuleResult {
        let node = match event {
            NodeEvent::Enter(x) => x,
            NodeEvent::Leave(_) => return SyntaxRuleResult::Pass,
        };

        match node {
            RefNode::AlwaysConstruct(x) => {
                let (t, x) = &x.nodes;
                match t {
                    AlwaysKeyword::AlwaysFf(_) => {
                        if let Some(x) = unwrap_node!(x, LoopStatement) {
                            let loc = unwrap_locate!(x.clone()).unwrap();
                            SyntaxRuleResult::FailLocate(*loc)
                        } else {
                            SyntaxRuleResult::Pass
                        }
                    }
                    _ => SyntaxRuleResult::Pass,
                }
            }
            _ => SyntaxRuleResult::Pass,
        }
    }

    fn name(&self) -> String {
        String::from("loop_statement_in_always_ff")
    }

    fn hint(&self, _option: &ConfigOption) -> String {
        String::from("Keywords `for` is forbidden within `always_ff`.")
    }

    fn reason(&self) -> String {
        String::from("Procedural loops within `always_ff` introduce sequential dependencies.")
    }
}
```

### Example: Language Server Integration

This example from `svls` (SystemVerilog Language Server) shows how to integrate parsing with diagnostics:

```rust
use std::collections::HashMap;
use sv_parser::{parse_sv_str, Define, DefineText};

fn lint(&self, s: &str, path: &Path) -> Vec<Diagnostic> {
    let mut ret = Vec::new();
    
    // Setup defines and include paths
    let mut defines = HashMap::new();
    for define_str in &config.verilog.defines {
        let mut parts = define_str.splitn(2, '=');
        let ident = parts.next().unwrap().to_string();
        let text = parts.next()
            .and_then(|x| enquote::unescape(x, None).ok())
            .map(|x| DefineText::new(x, None));
        let define = Define::new(ident.clone(), vec![], text);
        defines.insert(ident, Some(define));
    }

    // Parse the source
    let parsed = parse_sv_str(s, path, &defines, &include_paths, false, false);
    
    match parsed {
        Ok((syntax_tree, _)) => {
            // Check syntax rules for each node
            for event in syntax_tree.into_iter().event() {
                for failed in linter.syntaxrules_check(&syntax_tree, &event) {
                    // Convert to LSP diagnostic...
                }
            }
        }
        Err(e) => {
            // Handle parse error
            if let sv_parser::Error::Parse(Some((path, pos))) = e {
                // Create error diagnostic...
            }
        }
    }
    
    ret
}
```

---

## Complete Usage Example

Here's a complete example that demonstrates most features:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use sv_parser::{
    parse_sv_str, 
    Define, 
    DefineText, 
    unwrap_node, 
    unwrap_locate,
    NodeEvent, 
    RefNode, 
    SyntaxTree
};

fn main() {
    // Example SystemVerilog source
    let source = r#"
module FIFO #(
    parameter int DEPTH = 8,
    parameter int WIDTH = 32
) (
    input  logic clk,
    input  logic rst_n,
    input  logic [WIDTH-1:0] wr_data,
    input  logic wr_en,
    output logic [WIDTH-1:0] rd_data,
    input  logic rd_en,
    output logic full,
    output logic empty
);
    
    logic [WIDTH-1:0] mem [DEPTH];
    logic [$clog2(DEPTH)-1:0] wr_ptr;
    logic [$clog2(DEPTH)-1:0] rd_ptr;
    
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            wr_ptr <= '0;
            rd_ptr <= '0;
        end else begin
            if (wr_en && !full) begin
                mem[wr_ptr] <= wr_data;
                wr_ptr <= wr_ptr + 1'b1;
            end
            if (rd_en && !empty) begin
                rd_data <= mem[rd_ptr];
                rd_ptr <= rd_ptr + 1'b1;
            end
        end
    end
    
    assign full = (wr_ptr + 1'b1) == rd_ptr;
    assign empty = wr_ptr == rd_ptr;
    
endmodule
"#;

    // Create defines (like command-line -D options)
    let mut defines = HashMap::new();
    let text = DefineText::new("int".to_string(), None);
    let define = Define::new("logic".to_string(), vec![], Some(text));
    defines.insert("logic".to_string(), Some(define));

    // Parse from string
    let path = PathBuf::from("fifo.sv");
    let includes: Vec<PathBuf> = Vec::new();
    
    match parse_sv_str(source, &path, &defines, &includes, false, false) {
        Ok((syntax_tree, _defines)) => {
            println!("=== Parse Successful ===\n");
            
            // Print the AST (debug format)
            println!("AST Structure:");
            println!("{:?}", syntax_tree);
            
            // Find and print all module declarations
            println!("\n=== Modules Found ===");
            for node in &syntax_tree {
                match node {
                    RefNode::ModuleDeclarationAnsi(x) => {
                        if let Some(id) = unwrap_node!(x, ModuleIdentifier) {
                            // Get the identifier's location
                            let loc = unwrap_locate!(id).unwrap();
                            let name = syntax_tree.get_str(&id).unwrap();
                            println!("Module: {} (line {}, offset {})", 
                                name, loc.line, loc.offset);
                        }
                    }
                    RefNode::AlwaysConstruct(x) => {
                        let (keyword, _) = &x.nodes;
                        println!("Always block at line: {:?}", keyword);
                    }
                    RefNode::ContinuousAssignment(x) => {
                        println!("Continuous assignment found");
                        // Can traverse to get assign targets...
                    }
                    _ => {}
                }
            }
            
            // Event-based traversal example
            println!("\n=== Event Traversal ===");
            for event in syntax_tree.into_iter().event() {
                match event {
                    NodeEvent::Enter(RefNode::ModuleIdentifier(_)) => {
                        // Could process here
                    }
                    NodeEvent::Enter(RefNode::AlwaysConstruct(_)) => {
                        // Found an always block
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            println!("Parse failed: {:?}", e);
        }
    }
}
```

---

## Finding AST Node Types

The AST node types are defined in `sv-parser-syntaxtree/src/`. Here's the directory structure:

```
sv-parser-syntaxtree/src/
├── behavioral_statements/   # always, initial, procedures, assignments
│   ├── continuous_assignment_and_net_alias_statements.rs
│   ├── procedural_blocks_and_assignments.rs  # BlockingAssignment, NonblockingAssignment
│   ├── statements.rs
│   └── ...
├── declarations/            # types, parameters, ports, functions, tasks
├── expressions/             # expressions, operators, numbers, strings
├── instantiations/          # module, interface, program instantiation
├── source_text/             # modules, interfaces, packages, classes
├── general/                 # identifiers, comments, directives
└── ...
```

### Where to Find Specific Node Types

| What you want to find | File location |
|----------------------|---------------|
| **Assignments** | |
| `ContinuousAssign`, `NetAssignment`, `VariableAssignment` | `behavioral_statements/continuous_assignment_and_net_alias_statements.rs` |
| `BlockingAssignment`, `NonblockingAssignment` | `behavioral_statements/procedural_blocks_and_assignments.rs` |
| **Control flow** | |
| `IfElse`, `Case`, `CaseStatement` | `behavioral_statements/conditional_statements.rs` |
| `ForLoop`, `WhileLoop`, `DoWhileLoop`, `ForeachLoop` | `behavioral_statements/looping_statements.rs` |
| **Modules/Interfaces** | |
| `ModuleDeclarationAnsi`, `ModuleDeclarationNonansi` | `source_text/module_items.rs` |
| `InterfaceDeclaration` | `source_text/interface_items.rs` |
| **Expressions** | |
| `Expression`, `UnaryExpression`, `BinaryExpression` | `expressions/expressions.rs` |
| `Number`, `IntegralNumber`, `RealNumber` | `expressions/numbers.rs` |
| **Declarations** | |
| `Parameter`, `LocalParam` | `declarations/module_parameter_declarations.rs` |
| `FunctionDeclaration`, `TaskDeclaration` | `declarations/function_declarations.rs`, `task_declarations.rs` |

### Don't Know the AST Type? Print the AST!

If you're not sure what AST node type a SystemVerilog construct maps to, parse it and print the AST structure:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use sv_parser::{parse_sv_str, SyntaxTree};

fn print_ast(syntax_tree: &SyntaxTree) {
    // Using Display trait shows the tree structure
    println!("{}", syntax_tree);
}

fn main() {
    let source = r#"
module example;
    logic [7:0] data;
    logic clk;
    
    // This is an always_ff block
    always_ff @(posedge clk) begin
        data <= data + 1;
    end
    
    // This is a continuous assignment
    assign ready = valid;
endmodule
"#;

    let path = PathBuf::from("test.sv");
    let defines = HashMap::new();
    let includes: Vec<PathBuf> = Vec::new();

    match parse_sv_str(source, &path, &defines, &includes, false, false) {
        Ok((syntax_tree, _)) => {
            print_ast(&syntax_tree);
        }
        Err(e) => {
            println!("Parse error: {:?}", e);
        }
    }
}
```

**Sample Output:**
```
Token: 'module' @ line:2
 SystemVerilogSourceText
  SourceDescription
   SystemVerilogModule
    ModuleDeclarationAnsi
     Keyword @line:2: 'module'
     ModuleIdentifier
      SimpleIdentifier
       Token: 'example' @ line:2
     ...
     AlwaysConstruct
      ...
     ContinuousAssignment
      ...
```

The `{:?}` (Debug) format shows even more detail including whitespace and comments.

### Example: Finding Assignment Types

If you want to know what types of assignments exist in sv-parser:

```bash
# From sv-parser directory
grep -r "Assignment" sv-parser-syntaxtree/src/
```

You will find:
- `ContinuousAssign` - assign statements (outside always blocks)
- `BlockingAssignment` - `=` inside procedural blocks
- `NonblockingAssignment` - `<=` inside procedural blocks  
- `VariableAssignment` - variable assignments
- `NetAssignment` - net assignments

### Quick Reference: Common RefNode Variants

```rust
// Assignments
RefNode::ContinuousAssign(...)
RefNode::BlockingAssignment(...)
RefNode::NonblockingAssignment(...)

// Module/Interface
RefNode::ModuleDeclarationAnsi(...)
RefNode::ModuleDeclarationNonansi(...)
RefNode::InterfaceDeclaration(...)

// Control
RefNode::IfElse(...)
RefNode::CaseStatement(...)
RefNode::ForLoop(...)

// Always blocks
RefNode::AlwaysConstruct(x)  // x.nodes.0 is AlwaysKeyword (AlwaysFf, AlwaysComb, etc.)
RefNode::InitialConstruct(...)

// Declarations
RefNode::FunctionDeclaration(...)
RefNode::TaskDeclaration(...)
RefNode::ClassDeclaration(...)
```

---

## Additional Resources

- **svlint**: A SystemVerilog linter built with sv-parser - https://github.com/dalance/svlint
- **svls**: A SystemVerilog language server built with sv-parser - https://github.com/dalance/svls
- **sv-parser repository**: https://github.com/dalance/sv-parser

---

## Common Patterns

### Finding All Instances of a Node Type

```rust
// Collect all module names
let modules: Vec<String> = syntax_tree
    .into_iter()
    .filter_map(|node| {
        match node {
            RefNode::ModuleDeclarationAnsi(x) => {
                unwrap_node!(&x, ModuleIdentifier)
            }
            RefNode::ModuleDeclarationNonansi(x) => {
                unwrap_node!(&x, ModuleIdentifier)
            }
            _ => None,
        }
    })
    .filter_map(|id| {
        syntax_tree.get_str(&id).ok().map(|s| s.to_string())
    })
    .collect();
```

### Getting Line Numbers for Diagnostics

```rust
if let Some(loc) = unwrap_locate!(node.clone()) {
    if let Some((path, offset)) = syntax_tree.get_origin(&loc) {
        // Calculate line number from offset or use loc.line directly
        println!("Error at {}:{}", path.display(), loc.line);
    }
}
```

### Working with Nested Nodes

```rust
RefNode::AlwaysConstruct(x) => {
    let (keyword, body) = &x.nodes;
    
    // keyword is an AlwaysKeyword enum variant
    // body contains the statements inside the always block
    
    // Find blocking assignments in the always block
    for stmt in body {
        if let RefNode::BlockingAssignment(assign) = stmt {
            // Process blocking assignment...
        }
    }
}
```
