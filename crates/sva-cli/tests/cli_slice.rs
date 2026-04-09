use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn main_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_sva_cli"))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn cli_slice_outputs_graph_json_with_blues_by_default() {
    let fixture = write_slice_fixture();

    let output = Command::new(main_bin())
        .args([
            "slice",
            "--sv",
            fixture.design.to_str().unwrap(),
            "--sv",
            fixture.testbench.to_str().unwrap(),
            "--vcd",
            fixture.vcd.to_str().unwrap(),
            "--signal",
            "TOP.tb.dut.result",
            "--time",
            "1",
            "--min-time",
            "0",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());
    assert!(json["blocks"].is_array());
    assert!(
        json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|node| node.get("time").is_some()),
        "dynamic slice should keep time annotations: {json:?}"
    );

    fixture.cleanup();
}

#[test]
fn cli_slice_dynamic_supports_scoped_signal_queries() {
    let fixture = write_slice_fixture();

    let output = Command::new(main_bin())
        .args([
            "slice",
            "--sv",
            fixture.design.to_str().unwrap(),
            "--sv",
            fixture.testbench.to_str().unwrap(),
            "--vcd",
            fixture.vcd.to_str().unwrap(),
            "--signal",
            "TOP.tb.result",
            "--time",
            "1",
            "--min-time",
            "0",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let blocks = json["blocks"].as_array().unwrap();
    let scopes = blocks
        .iter()
        .filter_map(|block| block["scope"].as_str())
        .collect::<Vec<_>>();

    assert!(
        scopes.contains(&"TOP.tb.dut"),
        "expected dynamic slice to traverse into the instantiated dut scope: {json:?}"
    );
    assert!(
        json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|node| node.get("time").is_some()),
        "dynamic slice should keep time annotations: {json:?}"
    );

    fixture.cleanup();
}

#[test]
fn cli_slice_fails_for_non_posedge_time() {
    let fixture = write_slice_fixture();

    let output = Command::new(main_bin())
        .args([
            "slice",
            "--sv",
            fixture.design.to_str().unwrap(),
            "--sv",
            fixture.testbench.to_str().unwrap(),
            "--vcd",
            fixture.vcd.to_str().unwrap(),
            "--signal",
            "TOP.tb.dut.result",
            "--time",
            "2",
            "--min-time",
            "0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail for non-posedge time");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("validation failed"),
        "error should mention validation failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not a valid posedge time"),
        "error should mention posedge time: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fixture.cleanup();
}

#[test]
fn cli_slice_fails_for_misaligned_backtrack_with_explicit_clock() {
    let fixture = write_slice_fixture();

    let output = Command::new(main_bin())
        .args([
            "slice",
            "--sv",
            fixture.design.to_str().unwrap(),
            "--sv",
            fixture.testbench.to_str().unwrap(),
            "--vcd",
            fixture.vcd.to_str().unwrap(),
            "--signal",
            "TOP.tb.dut.result",
            "--time",
            "2",
            "--min-time",
            "0",
            "--clock",
            "tb.dut.clk",
            "--clk-step",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "should fail for non-posedge time with explicit clock"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("validation failed"),
        "error should mention validation failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not a valid posedge time"),
        "error should mention posedge time: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    fixture.cleanup();
}

#[test]
fn cli_slice_supports_static_graph_output() {
    let fixture = write_slice_fixture();

    let output = Command::new(main_bin())
        .args([
            "slice",
            "--static",
            "--sv",
            fixture.design.to_str().unwrap(),
            "--sv",
            fixture.testbench.to_str().unwrap(),
            "--signal",
            "TOP.tb.dut.result",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(json["nodes"].is_array());
    assert!(json["edges"].is_array());
    assert!(json["blocks"].is_array());
    assert!(
        json["blocks"].as_array().unwrap().iter().all(|block| {
            block.get("source_file").is_some()
                && block.get("line_start").is_some()
                && block.get("line_end").is_some()
                && block.get("code_snippet").is_some()
        }),
        "slice block metadata should include source details for the viewer sidebar: {json:?}"
    );
    assert!(
        json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|node| node.get("time").is_none()),
        "static slice should omit time annotations: {json:?}"
    );

    fixture.cleanup();
}

#[test]
fn cli_slice_resolves_scoped_signal_through_instance_boundaries() {
    let fixture = write_slice_fixture();

    let output = Command::new(main_bin())
        .args([
            "slice",
            "--static",
            "--sv",
            fixture.design.to_str().unwrap(),
            "--sv",
            fixture.testbench.to_str().unwrap(),
            "--signal",
            "TOP.tb.result",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let blocks = json["blocks"].as_array().unwrap();
    let scopes = blocks
        .iter()
        .filter_map(|block| block["scope"].as_str())
        .collect::<Vec<_>>();

    assert!(
        scopes.contains(&"TOP.tb.dut"),
        "expected slice to traverse into the instantiated dut scope: {json:?}"
    );
    assert!(
        blocks
            .iter()
            .any(|block| block["scope"] == "TOP.tb.dut" && block["block_type"] == "ModOutput"),
        "expected the dut ModOutput block to stay in the child scope: {json:?}"
    );
    assert!(
        blocks
            .iter()
            .filter(|block| {
                matches!(
                    block["block_type"].as_str(),
                    Some("ModInput") | Some("ModOutput")
                )
            })
            .all(|block| block["scope"] == "TOP.tb.dut"),
        "expected all child port blocks to keep the child module scope: {json:?}"
    );
    assert!(
        blocks.iter().any(|block| {
            block["scope"] == "TOP.tb.dut"
                && block["block_type"] == "ModInput"
                && block["code_snippet"]
                    .as_str()
                    .is_some_and(|snippet| snippet.contains("input logic a") || snippet.contains("input logic clk"))
        }),
        "expected child ModInput block to keep port declaration snippet, not instance call: {json:?}"
    );
    assert!(
        blocks.iter().any(|block| {
            block["scope"] == "TOP.tb.dut"
                && block["block_type"] == "ModOutput"
                && block["code_snippet"]
                    .as_str()
                    .is_some_and(|snippet| snippet.contains("output logic result"))
        }),
        "expected child ModOutput block to keep port declaration snippet: {json:?}"
    );

    fixture.cleanup();
}

#[test]
fn cli_slice_static_traces_across_multi_submodule_demo() {
    let design = workspace_root().join("demo/multi_submodule_demo/design.sv");
    let testbench = workspace_root().join("demo/multi_submodule_demo/tb.sv");

    let output = Command::new(main_bin())
        .args([
            "slice",
            "--static",
            "--sv",
            design.to_str().unwrap(),
            "--sv",
            testbench.to_str().unwrap(),
            "--signal",
            "TOP.tb.result",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let blocks = json["blocks"].as_array().unwrap();
    let nodes = json["nodes"].as_array().unwrap();
    let edges = json["edges"].as_array().unwrap();
    let scopes = blocks
        .iter()
        .filter_map(|block| block["scope"].as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert!(
        scopes.contains("TOP.tb.u_top"),
        "expected top instance scope in slice: {json:?}"
    );
    assert!(
        scopes.contains("TOP.tb.u_top.u_sub1"),
        "expected submodule1 scope in slice: {json:?}"
    );
    assert!(
        scopes.contains("TOP.tb.u_top.u_sub2"),
        "expected submodule2 scope in slice: {json:?}"
    );

    assert!(
        blocks
            .iter()
            .any(|block| { block["scope"] == "TOP.tb.u_top" && block["block_type"] == "ModInput" }),
        "expected top input port block in top scope: {json:?}"
    );
    assert!(
        blocks.iter().any(|block| {
            block["scope"] == "TOP.tb.u_top" && block["block_type"] == "ModOutput"
        }),
        "expected top output port block in top scope: {json:?}"
    );
    assert!(
        blocks.iter().any(|block| {
            block["scope"] == "TOP.tb.u_top.u_sub1" && block["block_type"] == "Assign"
        }),
        "expected submodule1 logic block in slice: {json:?}"
    );
    assert!(
        blocks.iter().any(|block| {
            block["scope"] == "TOP.tb.u_top.u_sub1" && block["block_type"] == "Always"
        }),
        "expected submodule1 always_comb block in slice: {json:?}"
    );
    assert!(
        blocks.iter().any(|block| {
            block["scope"] == "TOP.tb.u_top.u_sub2" && block["block_type"] == "Assign"
        }),
        "expected submodule2 logic block in slice: {json:?}"
    );
    assert!(
        blocks.iter().any(|block| {
            block["scope"] == "TOP.tb.u_top.u_sub2" && block["block_type"] == "Always"
        }),
        "expected submodule2 sequential block in slice: {json:?}"
    );

    let block_by_node = nodes
        .iter()
        .filter_map(|node| Some((node["id"].as_u64()?, node["block_id"].as_u64()?)))
        .collect::<std::collections::HashMap<_, _>>();
    let block_meta = blocks
        .iter()
        .filter_map(|block| {
            Some((
                block["id"].as_u64()?,
                (
                    block["scope"].as_str()?.to_string(),
                    block["block_type"].as_str()?.to_string(),
                ),
            ))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let edge_chain = edges
        .iter()
        .filter_map(|edge| {
            let from_node = edge["from"].as_u64()?;
            let to_node = edge["to"].as_u64()?;
            let from_block = *block_by_node.get(&from_node)?;
            let to_block = *block_by_node.get(&to_node)?;
            Some((
                block_meta[&from_block].clone(),
                block_meta[&to_block].clone(),
                edge["signal"]["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            ))
        })
        .collect::<Vec<_>>();

    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top".to_string(), "ModInput".to_string()),
            ("TOP.tb.u_top.u_sub1".to_string(), "ModInput".to_string()),
            "TOP.tb.u_top.src".to_string(),
        )),
        "expected top input to feed submodule1 input: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top".to_string(), "ModInput".to_string()),
            ("TOP.tb.u_top.u_sub1".to_string(), "ModInput".to_string()),
            "TOP.tb.u_top.enable".to_string(),
        )),
        "expected top enable input to feed submodule1 input: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top.u_sub1".to_string(), "Assign".to_string()),
            ("TOP.tb.u_top.u_sub1".to_string(), "Always".to_string()),
            "TOP.tb.u_top.u_sub1.src_masked".to_string(),
        )),
        "expected submodule1 assign to feed its always block: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top.u_sub1".to_string(), "Assign".to_string()),
            ("TOP.tb.u_top.u_sub1".to_string(), "Always".to_string()),
            "TOP.tb.u_top.u_sub1.src_inverted".to_string(),
        )),
        "expected second assign output to feed submodule1 always block: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top.u_sub1".to_string(), "Always".to_string()),
            ("TOP.tb.u_top.u_sub1".to_string(), "ModOutput".to_string()),
            "TOP.tb.u_top.u_sub1.stage1".to_string(),
        )),
        "expected submodule1 always block to feed its output port: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top".to_string(), "ModInput".to_string()),
            ("TOP.tb.u_top.u_sub2".to_string(), "ModInput".to_string()),
            "TOP.tb.u_top.rst_n".to_string(),
        )),
        "expected top reset input to feed submodule2 input: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top.u_sub1".to_string(), "ModOutput".to_string()),
            ("TOP.tb.u_top.u_sub2".to_string(), "ModInput".to_string()),
            "TOP.tb.u_top.stage1".to_string(),
        )),
        "expected submodule1 output to feed submodule2 input: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top.u_sub2".to_string(), "Always".to_string()),
            ("TOP.tb.u_top.u_sub2".to_string(), "Assign".to_string()),
            "TOP.tb.u_top.u_sub2.stage2".to_string(),
        )),
        "expected submodule2 sequential block to feed assign result block: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top.u_sub2".to_string(), "Assign".to_string()),
            ("TOP.tb.u_top.u_sub2".to_string(), "ModOutput".to_string()),
            "TOP.tb.u_top.u_sub2.result".to_string(),
        )),
        "expected submodule2 assign to feed its output port: {edge_chain:?}"
    );
    assert!(
        edge_chain.contains(&(
            ("TOP.tb.u_top.u_sub2".to_string(), "ModOutput".to_string()),
            ("TOP.tb.u_top".to_string(), "ModOutput".to_string()),
            "TOP.tb.u_top.result".to_string(),
        )),
        "expected top output port to receive the traced result from submodule2: {edge_chain:?}"
    );
}

struct SliceFixture {
    design: PathBuf,
    testbench: PathBuf,
    vcd: PathBuf,
}

impl SliceFixture {
    fn cleanup(self) {
        let _ = fs::remove_file(self.design);
        let _ = fs::remove_file(self.testbench);
        let _ = fs::remove_file(self.vcd);
    }
}

fn write_slice_fixture() -> SliceFixture {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let base = std::env::temp_dir().join(format!(
        "sva_task15_cli_slice_{}_{}",
        std::process::id(),
        unique
    ));

    let design = base.with_extension("design.sv");
    let testbench = base.with_extension("tb.sv");
    let vcd = base.with_extension("vcd");

    fs::write(
        &design,
        "module dut(input logic a, input logic b, input logic clk, output logic result);\n  logic tmp;\n  assign tmp = a & b;\n  always_ff @(posedge clk) result <= tmp;\nendmodule\n",
    )
    .unwrap();
    fs::write(
        &testbench,
        "module tb;\n  logic a;\n  logic b;\n  logic clk;\n  logic result;\n  dut dut(.a(a), .b(b), .clk(clk), .result(result));\nendmodule\n",
    )
    .unwrap();
    fs::write(
        &vcd,
        "$date\n    today\n$end\n\
$version\n    dac26 task15 slice\n$end\n\
$timescale 1ns $end\n\
$scope module tb $end\n\
$scope module dut $end\n\
$var wire 1 ! a $end\n\
$var wire 1 \" b $end\n\
$var wire 1 # clk $end\n\
$var wire 1 $ result $end\n\
$var wire 1 % tmp $end\n\
$var wire 32 & vlCoverageLineTrace_design__4_stmt [31:0] $end\n\
$upscope $end\n\
$upscope $end\n\
$enddefinitions $end\n\
#0\n\
0!\n\
0\"\n\
0#\n\
0$\n\
0%\n\
b0 &\n\
#1\n\
1#\n\
1!\n\
1\"\n\
1%\n\
b1 &\n\
#2\n\
1$\n\
0#\n\
#3\n\
1#\n\
#4\n\
0#\n\
#5\n\
1#\n\
",
    )
    .unwrap();

    SliceFixture {
        design,
        testbench,
        vcd,
    }
}
