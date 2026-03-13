use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use dac26_mcp::ast::{AstProvider, SvParserProvider};
use dac26_mcp::block::{BlockType, Blockizer, DataflowBlockizer};

#[test]
fn creates_paper_style_blocks_and_merges_chained_assigns() {
    let fixture = write_fixture(
        "module demo(\n  input logic clk,\n  input logic a,\n  input logic b,\n  output logic y,\n  output logic z\n);\n  logic tmp;\n\n  assign tmp = a & b;\n  assign y = tmp;\n\n  always_ff @(posedge clk) begin\n    z <= y;\n  end\nendmodule\n",
    );

    let provider = SvParserProvider::default();
    let parsed = provider.parse_files(&[fixture.clone()]).unwrap();
    let blockizer = DataflowBlockizer::default();
    let block_set = blockizer.blockize(&parsed).unwrap();

    let shapes = block_set
        .blocks()
        .iter()
        .map(|block| {
            (
                format!("{:?}", block.block_type()),
                sorted_signals(block.input_signals()),
                sorted_signals(block.output_signals()),
            )
        })
        .collect::<BTreeSet<_>>();

    assert!(
        shapes.contains(&("ModInput".to_string(), Vec::new(), vec!["a".to_string()],)),
        "missing a module-input block: {shapes:?}"
    );
    assert!(
        shapes.contains(&(
            "ModOutput".to_string(),
            vec!["y".to_string()],
            vec!["y".to_string()],
        )),
        "missing y module-output block: {shapes:?}"
    );
    assert!(
        shapes.contains(&(
            "Always".to_string(),
            vec!["y".to_string()],
            vec!["z".to_string()],
        )),
        "missing always block: {shapes:?}"
    );
    assert!(
        shapes.contains(&(
            "Assign".to_string(),
            vec!["a".to_string(), "b".to_string()],
            vec!["tmp".to_string(), "y".to_string()],
        )),
        "missing merged assign block: {shapes:?}"
    );
    assert_eq!(
        block_set
            .blocks()
            .iter()
            .filter(|block| matches!(block.block_type(), BlockType::Assign))
            .count(),
        1,
        "expected chained assigns to merge into one assign block: {shapes:?}"
    );

    let assign_entries = block_set
        .blocks()
        .iter()
        .find(|block| matches!(block.block_type(), BlockType::Assign))
        .unwrap()
        .dataflow()
        .iter()
        .map(|entry| (entry.output.0.clone(), sorted_signals(&entry.inputs)))
        .collect::<BTreeSet<_>>();

    assert!(
        assign_entries.contains(&("tmp".to_string(), vec!["a".to_string(), "b".to_string()])),
        "missing tmp merged entry dependencies: {assign_entries:?}"
    );
    assert!(
        assign_entries.contains(&("y".to_string(), vec!["a".to_string(), "b".to_string()])),
        "missing propagated y merged entry dependencies: {assign_entries:?}"
    );

    let _ = fs::remove_file(fixture);
}

#[test]
fn creates_port_blocks_for_non_ansi_modules_too() {
    let fixture = write_fixture(
        "module legacy(clk, a, y);\n  input logic clk;\n  input logic a;\n  output logic y;\n\n  assign y = a;\nendmodule\n",
    );

    let provider = SvParserProvider::default();
    let parsed = provider.parse_files(&[fixture.clone()]).unwrap();
    let blockizer = DataflowBlockizer::default();
    let block_set = blockizer.blockize(&parsed).unwrap();

    let shapes = block_set
        .blocks()
        .iter()
        .map(|block| {
            (
                format!("{:?}", block.block_type()),
                sorted_signals(block.input_signals()),
                sorted_signals(block.output_signals()),
            )
        })
        .collect::<BTreeSet<_>>();

    assert!(
        shapes.contains(&("ModInput".to_string(), Vec::new(), vec!["a".to_string()])),
        "missing non-ANSI input block: {shapes:?}"
    );
    assert!(
        shapes.contains(&("ModInput".to_string(), Vec::new(), vec!["clk".to_string()])),
        "missing non-ANSI clock input block: {shapes:?}"
    );
    assert!(
        shapes.contains(&(
            "ModOutput".to_string(),
            vec!["y".to_string()],
            vec!["y".to_string()],
        )),
        "missing non-ANSI output block: {shapes:?}"
    );

    let _ = fs::remove_file(fixture);
}

fn sorted_signals(signals: &std::collections::HashSet<dac26_mcp::types::SignalId>) -> Vec<String> {
    let mut values = signals
        .iter()
        .map(|signal| signal.0.clone())
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn write_fixture(contents: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "dac26_task7_blockize_demo_design_{}_{}.sv",
        std::process::id(),
        unique
    ));

    fs::write(&path, contents).unwrap();
    path
}
