---
name: sv-parser-usage
description: Use when parsing SystemVerilog source code in Rust, building linters, language servers, or code analysis tools for SystemVerilog
---

# sv-parser: SystemVerilog Parser for Rust

## Quick Reference

| Need | Solution |
|------|----------|
| Parse file | `parse_sv(path, &defines, &includes, ignore_include, allow_incomplete)` |
| Parse string | `parse_sv_str(source, path, &defines, &includes, ...)` |
| Get node text | `syntax_tree.get_str(&node)` |
| Find children | `unwrap_node!(nodes, ChildType)` |
| Get location | `unwrap_locate!(node)` |
| Traverse AST | `for node in &syntax_tree { ... }` |

## Installation

```toml
[dependencies]
sv-parser = "0.13"
```

## Core Types

```rust
// Main parse result
SyntaxTree { node: AnyNode, text: PreprocessedText }

// AST node reference (enum with 200+ variants)
RefNode::ModuleDeclarationAnsi(..)
RefNode::AlwaysConstruct(..)
RefNode::ContinuousAssignment(..)
RefNode::BlockingAssignment(..)
RefNode::NonblockingAssignment(..)

// Position info
Locate { offset: usize, line: u32, len: usize }
```

## Parsing

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use sv_parser::parse_sv;

let path = PathBuf::from("design.sv");
let defines = HashMap::new();
let includes: Vec<PathBuf> = Vec::new();

match parse_sv(&path, &defines, &includes, false, false) {
    Ok((syntax_tree, _defines)) => { /* success */ }
    Err(e) => { /* handle error */ }
}
```

From string (for IDE/language servers):
```rust
parse_sv_str(source, path, &defines, &includes, false, false)
```

## AST Traversal

### Simple iteration:
```rust
for node in &syntax_tree {
    match node {
        RefNode::ModuleDeclarationAnsi(x) => { /* ... */ }
        RefNode::AlwaysConstruct(x) => { /* ... */ }
        RefNode::ContinuousAssignment(x) => { /* ... */ }
        _ => {}
    }
}
```

### Event-based (enter/leave):
```rust
for event in syntax_tree.into_iter().event() {
    match event {
        NodeEvent::Enter(node) => { /* entering */ }
        NodeEvent::Leave(node) => { /* leaving */ }
    }
}
```

## Extracting Node Data

### Get identifier name:
```rust
if let Some(id) = unwrap_node!(x, ModuleIdentifier) {
    let name = syntax_tree.get_str(&id).unwrap();
}
```

### Find child nodes:
```rust
RefNode::AlwaysConstruct(x) => {
    let (keyword, body) = &x.nodes;
    // keyword is AlwaysKeyword (AlwaysFf, AlwaysComb, etc.)
    for stmt in body {
        if let RefNode::BlockingAssignment(assign) = stmt { ... }
    }
}
```

### Get line number:
```rust
if let Some(loc) = unwrap_locate!(node.clone()) {
    println!("Line {}", loc.line);
}
```

## Working with Defines

```rust
use sv_parser::{Define, DefineText};

// Simple define
let define = Define::new("DEBUG".to_string(), vec![], None);

// Define with value
let text = DefineText::new("1".to_string(), None);
let define = Define::new("DATA_WIDTH".to_string(), vec![], Some(text));

// Function-like macro
let args = vec![("WIDTH".to_string(), None)];
let text = DefineText::new("WIDTH * 2".to_string(), None);
let define = Define::new("DOUBLE".to_string(), args, Some(text));

defines.insert("DEBUG".to_string(), Some(define));
```

## Error Handling

```rust
match parse_sv(&path, &defines, &includes, false, false) {
    Ok((syntax_tree, _)) => { /* success */ }
    Err(Error::Parse(Some((path, pos)))) => {
        eprintln!("Parse error at {}:{}", path.display(), pos);
    }
    Err(Error::File { source, path }) => {
        eprintln!("File error: {} - {}", path.display(), source);
    }
}
```

## Common RefNode Variants

```rust
// Declarations
RefNode::ModuleDeclarationAnsi(..)
RefNode::ModuleDeclarationNonansi(..)
RefNode::InterfaceDeclaration(..)
RefNode::ClassDeclaration(..)
RefNode::FunctionDeclaration(..)
RefNode::TaskDeclaration(..)

// Statements
RefNode::AlwaysConstruct(..)
RefNode::InitialConstruct(..)
RefNode::ContinuousAssignment(..)
RefNode::BlockingAssignment(..)
RefNode::NonblockingAssignment(..)

// Control flow
RefNode::IfElse(..)
RefNode::CaseStatement(..)
RefNode::ForLoop(..)
RefNode::WhileLoop(..)
```

## Finding AST Node Types

AST types are in `sv-parser-syntaxtree/src/`:

| Category | Types | Location |
|----------|-------|----------|
| Assignments | ContinuousAssign, BlockingAssignment, NonblockingAssignment | `behavioral_statements/` |
| Control flow | IfElse, CaseStatement, ForLoop, WhileLoop | `behavioral_statements/` |
| Modules | ModuleDeclarationAnsi, ModuleDeclarationNonansi | `source_text/` |
| Expressions | Expression, Number, IntegralNumber | `expressions/` |

## Debugging: Print AST

Not sure what node type? Print the tree:

```rust
println!("{}", syntax_tree);  // Shows structure
println!("{:?}", syntax_tree); // Shows detailed debug
```

## Example: Extract All Module Names

```rust
use sv_parser::{parse_sv, unwrap_node, RefNode};

let result = parse_sv(&path, &HashMap::new(), &Vec::new(), false, false);
if let Ok((syntax_tree, _)) = result {
    for node in &syntax_tree {
        match node {
            RefNode::ModuleDeclarationAnsi(x) | RefNode::ModuleDeclarationNonansi(x) => {
                if let Some(id) = unwrap_node!(x, ModuleIdentifier) {
                    println!("Module: {}", syntax_tree.get_str(&id).unwrap());
                }
            }
            _ => {}
        }
    }
}
```

## Example: Complete Usage

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use sv_parser::{parse_sv_str, Define, DefineText, unwrap_node, unwrap_locate, RefNode};

let source = r#"
module FIFO #(parameter int DEPTH = 8) (input clk, input rst_n);
    always_ff @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin end
    end
endmodule
"#;

let mut defines = HashMap::new();
let text = DefineText::new("int".to_string(), None);
defines.insert("logic".to_string(), Some(Define::new("logic".to_string(), vec![], Some(text))));

match parse_sv_str(source, &PathBuf::from("test.sv"), &defines, &Vec::new(), false, false) {
    Ok((syntax_tree, _)) => {
        for node in &syntax_tree {
            if let RefNode::ModuleDeclarationAnsi(x) = node {
                if let Some(id) = unwrap_node!(x, ModuleIdentifier) {
                    let loc = unwrap_locate!(id).unwrap();
                    println!("Module: {} at line {}", syntax_tree.get_str(&id).unwrap(), loc.line);
                }
            }
        }
    }
    Err(e) => println!("Parse error: {:?}", e),
}
```

## Additional Resources

- **svlint**: SystemVerilog linter - https://github.com/dalance/svlint
- **svls**: Language server - https://github.com/dalance/svls
- **sv-parser**: https://github.com/dalance/sv-parser
