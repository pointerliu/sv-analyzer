use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use dac26_mcp::ast::{AstProvider, SvParserProvider};
use dac26_mcp::block::{BlockType, Blockizer, DataflowBlockizer};

#[test]
fn creates_paper_style_blocks_and_merges_chained_assigns() {
    let fixture = write_fixture(
        "module demo(\n  input logic clk,\n  input logic a,\n  input logic b,\n  output logic y,\n  output logic z\n);\n  logic tmp;\n\n  assign tmp = a & b;\n  assign y = tmp;\n\n  always_ff @(posedge clk) begin\n    z <= y;\n  end\nendmodule\n",
    );

    let provider = SvParserProvider;
    let parsed = provider
        .parse_files(std::slice::from_ref(&fixture))
        .unwrap();
    let blockizer = DataflowBlockizer;
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
        shapes.contains(&("ModOutput".to_string(), vec!["y".to_string()], Vec::new(),)),
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
        .map(|entry| (sorted_outputs(&entry.output), sorted_signals(&entry.inputs)))
        .collect::<BTreeSet<_>>();

    assert!(
        assign_entries.contains(&(
            vec!["tmp".to_string()],
            vec!["a".to_string(), "b".to_string()]
        )),
        "missing tmp merged entry dependencies: {assign_entries:?}"
    );
    assert!(
        assign_entries.contains(&(
            vec!["y".to_string()],
            vec!["a".to_string(), "b".to_string()]
        )),
        "missing propagated y merged entry dependencies: {assign_entries:?}"
    );

    let _ = fs::remove_file(fixture);
}

#[test]
fn creates_port_blocks_for_non_ansi_modules_too() {
    let fixture = write_fixture(
        "module legacy(clk, a, y);\n  input logic clk;\n  input logic a;\n  output logic y;\n\n  assign y = a;\nendmodule\n",
    );

    let provider = SvParserProvider;
    let parsed = provider
        .parse_files(std::slice::from_ref(&fixture))
        .unwrap();
    let blockizer = DataflowBlockizer;
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
        shapes.contains(&("ModOutput".to_string(), vec!["y".to_string()], Vec::new(),)),
        "missing non-ANSI output block: {shapes:?}"
    );

    let _ = fs::remove_file(fixture);
}

#[test]
fn captures_demo_design_line_ranges_code_snippets_and_top_level_outputs() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("demo/trace_coverage_demo/design.sv");

    let parsed = SvParserProvider
        .parse_files(std::slice::from_ref(&fixture))
        .unwrap();
    let block_set = DataflowBlockizer.blockize(&parsed).unwrap();

    let clk_input = block_set
        .blocks()
        .iter()
        .find(|block| {
            matches!(block.block_type(), BlockType::ModInput)
                && sorted_signals(block.output_signals()) == vec!["clk".to_string()]
        })
        .unwrap();
    assert_eq!(clk_input.line_start(), 3);
    assert_eq!(clk_input.line_end(), 3);
    assert!(clk_input.code_snippet().contains("input  logic        clk"));

    let state_register = block_set
        .blocks()
        .iter()
        .find(|block| {
            matches!(block.block_type(), BlockType::Always)
                && matches!(
                    block.circuit_type(),
                    dac26_mcp::block::CircuitType::Sequential
                )
                && block.line_start() == 23
                && sorted_signals(block.output_signals()) == vec!["state".to_string()]
        })
        .unwrap();
    assert_eq!(state_register.line_end(), 28);
    assert!(state_register
        .code_snippet()
        .contains("always_ff @(posedge clk or negedge rst_n) begin"));
    assert!(state_register
        .code_snippet()
        .contains("state <= next_state;"));
    assert_eq!(state_register.dataflow().len(), 2);

    let state_output_names = state_register
        .dataflow()
        .iter()
        .flat_map(|entry| entry.output.iter())
        .map(|signal| signal.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(state_output_names, vec!["state", "state"]);

    let state_output_offsets = state_register
        .dataflow()
        .iter()
        .flat_map(|entry| entry.output.iter())
        .map(|signal| signal.locate.offset)
        .collect::<Vec<_>>();
    assert_eq!(state_output_offsets.len(), 2);
    assert_ne!(state_output_offsets[0], state_output_offsets[1]);

    let result_output = block_set
        .blocks()
        .iter()
        .find(|block| {
            matches!(block.block_type(), BlockType::ModOutput)
                && sorted_signals(block.input_signals()) == vec!["result".to_string()]
        })
        .unwrap();
    assert!(result_output.output_signals().is_empty());

    let json = serde_json::to_value(&block_set).unwrap();
    let clk_input_json = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| {
            block["block_type"] == "ModInput"
                && block["output_signals"] == serde_json::json!(["clk"])
        })
        .unwrap();
    assert_eq!(clk_input_json["line_start"], 3);
    assert_eq!(clk_input_json["line_end"], 3);
    assert_eq!(
        clk_input_json["code_snippet"],
        serde_json::json!("input  logic        clk,")
    );

    let state_register_json = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| {
            block["block_type"] == "Always"
                && block["line_start"] == 23
                && block["output_signals"] == serde_json::json!(["state"])
        })
        .unwrap();
    assert_eq!(state_register_json["line_end"], 28);
    assert!(state_register_json["code_snippet"]
        .as_str()
        .unwrap()
        .contains("state <= next_state;"));
    assert_eq!(state_register_json["dataflow"].as_array().unwrap().len(), 2);
    assert_eq!(
        state_register_json["dataflow"][0]["output"][0]["name"],
        "state"
    );
    assert_eq!(
        state_register_json["dataflow"][1]["output"][0]["name"],
        "state"
    );
    assert_ne!(
        state_register_json["dataflow"][0]["output"][0]["locate"]["offset"],
        state_register_json["dataflow"][1]["output"][0]["locate"]["offset"]
    );

    let next_state_logic = block_set
        .blocks()
        .iter()
        .find(|block| {
            matches!(block.block_type(), BlockType::Always)
                && matches!(
                    block.circuit_type(),
                    dac26_mcp::block::CircuitType::Combinational
                )
                && block.line_start() == 31
                && block.line_end() == 50
        })
        .unwrap();
    assert_eq!(next_state_logic.dataflow().len(), 6);

    let next_state_outputs = next_state_logic
        .dataflow()
        .iter()
        .map(|entry| {
            assert_eq!(entry.output.len(), 1);
            &entry.output[0]
        })
        .collect::<Vec<_>>();
    assert!(next_state_outputs
        .iter()
        .all(|signal| signal.name == "next_state"));

    let next_state_output_offsets = next_state_outputs
        .iter()
        .map(|signal| signal.locate.offset)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(next_state_output_offsets.len(), 6);

    let next_state_exec_entry = next_state_logic
        .dataflow()
        .iter()
        .find(|entry| {
            entry.output.len() == 1
                && entry.output[0].name == "next_state"
                && entry.output[0].locate.line == 36
        })
        .unwrap();
    let next_state_exec_inputs = next_state_exec_entry
        .inputs
        .iter()
        .map(|signal| {
            (
                signal.name.clone(),
                signal.locate.line,
                signal.locate.offset,
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(next_state_exec_inputs.contains(&("ST_EXEC".to_string(), 36, 1079)));
    assert!(next_state_exec_inputs.contains(&("ST_IDLE".to_string(), 34, 919)));
    assert!(next_state_exec_inputs.contains(&("state".to_string(), 33, 848)));
    assert!(next_state_exec_inputs.contains(&("op".to_string(), 35, 995)));

    let next_state_logic_json = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["line_start"] == 31 && block["line_end"] == 50)
        .unwrap();
    assert_eq!(
        next_state_logic_json["dataflow"].as_array().unwrap().len(),
        6
    );

    let next_state_names = next_state_logic_json["dataflow"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["output"][0]["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    assert_eq!(next_state_names, vec!["next_state"; 6]);

    let next_state_json_offsets = next_state_logic_json["dataflow"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["output"][0]["locate"]["offset"].as_u64().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(next_state_json_offsets.len(), 6);

    let next_state_exec_json = next_state_logic_json["dataflow"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["output"][0]["locate"]["line"] == 36)
        .unwrap();
    let next_state_exec_json_inputs = next_state_exec_json["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|signal| {
            (
                signal["name"].as_str().unwrap().to_string(),
                signal["locate"]["line"].as_u64().unwrap(),
                signal["locate"]["offset"].as_u64().unwrap(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(next_state_exec_json_inputs.contains(&("ST_EXEC".to_string(), 36, 1079)));
    assert!(next_state_exec_json_inputs.contains(&("ST_IDLE".to_string(), 34, 919)));
    assert!(next_state_exec_json_inputs.contains(&("state".to_string(), 33, 848)));
    assert!(next_state_exec_json_inputs.contains(&("op".to_string(), 35, 995)));

    let reset_result_entry = block_set
        .blocks()
        .iter()
        .find(|block| {
            matches!(block.block_type(), BlockType::Always)
                && matches!(
                    block.circuit_type(),
                    dac26_mcp::block::CircuitType::Sequential
                )
                && block.line_start() == 53
                && block.line_end() == 79
        })
        .unwrap()
        .dataflow()
        .iter()
        .find(|entry| {
            entry.output.len() == 1
                && entry.output[0].name == "result"
                && entry.output[0].locate.line == 55
        })
        .unwrap();
    let reset_result_inputs = reset_result_entry
        .inputs
        .iter()
        .map(|signal| {
            (
                signal.kind.as_str().to_string(),
                signal.name.clone(),
                signal.locate.line,
                signal.locate.offset,
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(reset_result_inputs.contains(&("variable".to_string(), "rst_n".to_string(), 54, 1676,)));
    assert!(reset_result_inputs.contains(&("literal".to_string(), "8'h0".to_string(), 55, 1713,)));

    let result_output_json = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| {
            block["block_type"] == "ModOutput"
                && block["input_signals"] == serde_json::json!(["result"])
        })
        .unwrap();
    assert_eq!(result_output_json["output_signals"], serde_json::json!([]));

    let reset_result_json = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["line_start"] == 53 && block["line_end"] == 79)
        .unwrap()["dataflow"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            entry["output"][0]["name"] == "result" && entry["output"][0]["locate"]["line"] == 55
        })
        .unwrap();
    let reset_result_json_inputs = reset_result_json["inputs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|signal| {
            (
                signal["kind"].as_str().unwrap().to_string(),
                signal["name"].as_str().unwrap().to_string(),
                signal["locate"]["line"].as_u64().unwrap(),
                signal["locate"]["offset"].as_u64().unwrap(),
            )
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert!(reset_result_json_inputs.contains(&(
        "variable".to_string(),
        "rst_n".to_string(),
        54,
        1676,
    )));
    assert!(reset_result_json_inputs.contains(&(
        "literal".to_string(),
        "8'h0".to_string(),
        55,
        1713,
    )));
    assert!(result_output_json.get("code_snippet").is_some());
    assert!(result_output_json.get("ast_snippet").is_none());
}

#[test]
fn blockize_assigns_stable_ids_for_identical_inputs() {
    let design = Path::new(env!("CARGO_MANIFEST_DIR")).join("demo/multi_submodule_demo/design.sv");
    let testbench = Path::new(env!("CARGO_MANIFEST_DIR")).join("demo/multi_submodule_demo/tb.sv");

    let parsed_once = SvParserProvider
        .parse_files(&[design.clone(), testbench.clone()])
        .unwrap();
    let parsed_twice = SvParserProvider.parse_files(&[design, testbench]).unwrap();

    let first = DataflowBlockizer.blockize(&parsed_once).unwrap();
    let second = DataflowBlockizer.blockize(&parsed_twice).unwrap();

    let first_by_snippet = first
        .blocks()
        .iter()
        .map(|block| {
            (
                (
                    block.module_scope().to_string(),
                    block.line_start(),
                    block.line_end(),
                    block.code_snippet().to_string(),
                ),
                block.id().0,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let second_by_snippet = second
        .blocks()
        .iter()
        .map(|block| {
            (
                (
                    block.module_scope().to_string(),
                    block.line_start(),
                    block.line_end(),
                    block.code_snippet().to_string(),
                ),
                block.id().0,
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();

    assert_eq!(
        first_by_snippet, second_by_snippet,
        "expected stable block IDs for identical inputs"
    );
}

fn sorted_outputs(signals: &[dac26_mcp::types::SignalNode]) -> Vec<String> {
    let mut values = signals
        .iter()
        .map(|signal| signal.name.clone())
        .collect::<Vec<_>>();
    values.sort();
    values
}

fn sorted_signals(
    signals: &std::collections::HashSet<dac26_mcp::types::SignalNode>,
) -> Vec<String> {
    let mut values = signals
        .iter()
        .map(|signal| signal.name.clone())
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
