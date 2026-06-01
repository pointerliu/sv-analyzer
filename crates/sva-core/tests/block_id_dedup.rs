use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use sva_core::ast::{AstProvider, SvParserProvider};
use sva_core::block::{elaborate_block_set, Blockizer, DataflowBlockizer};

/// Two instances of the same submodule should produce the same block_id
/// for blocks with identical source location (file + lines).
#[test]
fn same_source_code_same_block_id_across_instances() -> Result<()> {
    let tmp = PathBuf::from("/tmp/sva_block_id_dedup_test");
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp)?;

    // sub.v: a submodule with one assign block
    fs::write(
        tmp.join("sub.v"),
        "module sub(input logic a, input logic b, output logic y);\n  assign y = a + b;\nendmodule\n",
    )?;

    // top.v: instantiates sub twice
    fs::write(
        tmp.join("top.v"),
        "module top(input logic a1, input logic b1, input logic a2, input logic b2, output logic y1, output logic y2);\n  sub u1(.a(a1), .b(b1), .y(y1));\n  sub u2(.a(a2), .b(b2), .y(y2));\nendmodule\n",
    )?;

    let sv_files = vec![tmp.join("sub.v"), tmp.join("top.v")];
    let parsed = SvParserProvider.parse_files(&sv_files)?;
    let template = DataflowBlockizer.blockize(&parsed, None)?;
    let block_set = elaborate_block_set(&parsed, &template)?;

    // Find blocks for the "assign y = a + b" line (line 2 in sub.v)
    let assign_blocks: Vec<_> = block_set
        .blocks()
        .iter()
        .filter(|b| b.source_file().contains("sub.v") && b.line_start() == 2)
        .collect();

    assert!(
        assign_blocks.len() >= 2,
        "expected at least 2 blocks for line 2 of sub.v (one per instance), got {}",
        assign_blocks.len()
    );

    // All blocks at the same source location must have the same block_id
    let first_id = assign_blocks[0].id();
    for block in &assign_blocks[1..] {
        assert_eq!(
            block.id(),
            first_id,
            "block at same source location has different id: scope={} id={} vs scope={} id={}",
            block.module_scope(),
            block.id().0,
            assign_blocks[0].module_scope(),
            first_id.0,
        );
    }

    // Verify scopes differ (they are different instances)
    let scopes: Vec<&str> = assign_blocks.iter().map(|b| b.module_scope()).collect();
    let unique_scopes: std::collections::HashSet<_> = scopes.iter().copied().collect();
    assert!(
        unique_scopes.len() >= 2,
        "expected different scopes for the two instances, got: {:?}",
        scopes
    );

    Ok(())
}
