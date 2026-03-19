use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use sva_core::ast::AstProvider;
use sva_core::ast::SvParserProvider;
use sva_core::block::DataflowBlockizer;
use sva_core::block::{BlockType, Blockizer};

#[test]
fn extracts_statement_level_dataflow_from_assignments_and_conditions() {
    let fixture = write_fixture(
        "module helper(\n  input logic x,\n  output logic z\n);\n  assign z = x;\nendmodule\n\nmodule sample(\n  input logic clk,\n  input logic a,\n  input logic b,\n  input logic c,\n  input logic d,\n  input logic e,\n  input logic sel,\n  input logic [1:0] op,\n  input logic alt_sel,\n  input logic alt_sel2,\n  output logic y,\n  output logic q,\n  output logic r,\n  output logic s,\n  output logic h,\n  output logic t,\n  output logic latch_q,\n  output logic p\n);\n  logic next_q;\n\n  function automatic logic pick(input logic value);\n    pick = value;\n  endfunction\n\n  helper helper_inst(.x(a), .z());\n\n  assign y = a & b;\n\n  always_comb begin\n    logic temp_init = b;\n\n    if (op matches (tagged JmpC '{cc:.alt_sel2, addr:.d})) begin\n      p = a;\n    end\n\n    case (op)\n      alt_sel: r = next_q;\n      alt_sel2: r = d;\n      default: r = a;\n    endcase\n\n    h = helper_inst.z;\n    t = pick(a);\n    r += a;\n  end\n\n  always_ff @(posedge clk) begin\n    if (c) begin\n      q <= q + 1;\n    end else begin\n      q <= next_q;\n    end\n  end\n\n  always @(posedge clk) begin\n    s <= q;\n  end\n\n  always_latch begin\n    if (sel) begin\n      latch_q <= a;\n    end\n  end\nendmodule\n",
    );

    let provider = SvParserProvider;
    let parsed = provider
        .parse_files(std::slice::from_ref(&fixture))
        .unwrap();

    let blockizer = DataflowBlockizer;
    let blocks = blockizer.blockize(&parsed).unwrap();

    let actual = collect_entries(&blocks);
    let expected = BTreeSet::from([
        entry("helper", "Assign", "Combinational", &["z"], &["x"]),
        entry("sample", "Assign", "Combinational", &["y"], &["a", "b"]),
        entry(
            "sample",
            "Always",
            "Combinational",
            &["r"],
            &["alt_sel", "next_q", "op"],
        ),
        entry(
            "sample",
            "Always",
            "Combinational",
            &["r"],
            &["alt_sel2", "d", "op"],
        ),
        entry("sample", "Always", "Combinational", &["r"], &["a", "op"]),
        entry(
            "sample",
            "Always",
            "Combinational",
            &["h"],
            &["helper_inst.z"],
        ),
        entry("sample", "Always", "Combinational", &["p"], &["a", "op"]),
        entry("sample", "Always", "Combinational", &["t"], &["a"]),
        entry("sample", "Always", "Combinational", &["temp_init"], &["b"]),
        entry("sample", "Always", "Sequential", &["q"], &["1", "c", "q"]),
        entry("sample", "Always", "Sequential", &["q"], &["c", "next_q"]),
        entry("sample", "Always", "Sequential", &["s"], &["q"]),
        entry(
            "sample",
            "Always",
            "Sequential",
            &["latch_q"],
            &["a", "sel"],
        ),
    ]);

    assert_eq!(actual, expected, "unexpected extracted dataflow set");

    let _ = fs::remove_file(fixture);
}

#[test]
fn keeps_multi_target_assignments_as_one_dataflow_entry_with_list_output() {
    let fixture = write_fixture(
        "module sample(input logic c, input logic d, output logic a, output logic b);\n  always_comb begin\n    {a, b} = {c, d};\n  end\nendmodule\n",
    );

    let parsed = SvParserProvider
        .parse_files(std::slice::from_ref(&fixture))
        .unwrap();
    let blocks = DataflowBlockizer.blockize(&parsed).unwrap();

    let multi_output_entries = blocks
        .blocks()
        .iter()
        .filter(|block| matches!(block.block_type(), BlockType::Always))
        .flat_map(|block| block.dataflow().iter())
        .map(|entry| (sorted_outputs(&entry.output), sorted_inputs(&entry.inputs)))
        .collect::<Vec<_>>();

    assert_eq!(
        multi_output_entries,
        vec![(
            vec!["a".to_string(), "b".to_string()],
            vec!["c".to_string(), "d".to_string()],
        )]
    );

    let json = serde_json::to_value(&blocks).unwrap();
    let always_json = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|block| block["block_type"] == "Always")
        .unwrap();
    assert_eq!(
        sorted_json_signal_names(&always_json["dataflow"][0]["output"]),
        vec!["a".to_string(), "b".to_string()]
    );
    assert_eq!(
        sorted_json_signal_names(&always_json["dataflow"][0]["inputs"]),
        vec!["c".to_string(), "d".to_string()]
    );

    let _ = fs::remove_file(fixture);
}

fn collect_entries(
    blocks: &sva_core::block::BlockSet,
) -> BTreeSet<(String, String, String, Vec<String>, Vec<String>)> {
    blocks
        .blocks()
        .iter()
        .filter(|block| matches!(block.block_type(), BlockType::Assign | BlockType::Always))
        .flat_map(|block| {
            block.dataflow().iter().map(|dataflow| {
                let mut inputs = dataflow
                    .inputs
                    .iter()
                    .map(|signal| signal.name.clone())
                    .collect::<Vec<_>>();
                inputs.sort();

                (
                    block.module_scope().to_string(),
                    format!("{:?}", block.block_type()),
                    format!("{:?}", block.circuit_type()),
                    sorted_outputs(&dataflow.output),
                    inputs,
                )
            })
        })
        .collect()
}

fn entry(
    module_scope: &str,
    block_type: &str,
    circuit_type: &str,
    output: &[&str],
    inputs: &[&str],
) -> (String, String, String, Vec<String>, Vec<String>) {
    let mut output = output
        .iter()
        .map(|signal| (*signal).to_string())
        .collect::<Vec<_>>();
    output.sort();
    let mut inputs = inputs
        .iter()
        .map(|signal| (*signal).to_string())
        .collect::<Vec<_>>();
    inputs.sort();

    (
        module_scope.to_string(),
        block_type.to_string(),
        circuit_type.to_string(),
        output,
        inputs,
    )
}

fn sorted_outputs(signals: &[sva_core::types::SignalNode]) -> Vec<String> {
    let mut outputs = signals
        .iter()
        .map(|signal| signal.name.clone())
        .collect::<Vec<_>>();
    outputs.sort();
    outputs
}

fn sorted_inputs(signals: &std::collections::HashSet<sva_core::types::SignalNode>) -> Vec<String> {
    let mut inputs = signals
        .iter()
        .map(|signal| signal.name.clone())
        .collect::<Vec<_>>();
    inputs.sort();
    inputs
}

fn write_fixture(contents: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sva_task6_extract_assign_dataflow_{}_{}.sv",
        std::process::id(),
        unique
    ));

    fs::write(&path, contents).unwrap();
    path
}

fn sorted_json_signal_names(value: &serde_json::Value) -> Vec<String> {
    let mut values = value
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["name"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();
    values.sort();
    values
}
