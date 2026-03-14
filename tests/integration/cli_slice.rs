use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn cli_slice_outputs_graph_json_with_blues_by_default() {
    let fixture = write_slice_fixture();

    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .args([
            "slice",
            "--sv",
            fixture.design.to_str().unwrap(),
            "--sv",
            fixture.testbench.to_str().unwrap(),
            "--vcd",
            fixture.vcd.to_str().unwrap(),
            "--signal",
            "result",
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

    let output = Command::new(env!("CARGO_BIN_EXE_main"))
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
        scopes.contains(&"TOP.tb"),
        "expected dynamic slice to include the parent tb scope: {json:?}"
    );
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
fn cli_slice_supports_static_graph_output() {
    let fixture = write_slice_fixture();

    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .args([
            "slice",
            "--static",
            "--sv",
            fixture.design.to_str().unwrap(),
            "--sv",
            fixture.testbench.to_str().unwrap(),
            "--signal",
            "result",
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
            .all(|node| node.get("time").is_none()),
        "static slice should omit time annotations: {json:?}"
    );

    fixture.cleanup();
}

#[test]
fn cli_slice_resolves_scoped_signal_through_instance_boundaries() {
    let fixture = write_slice_fixture();

    let output = Command::new(env!("CARGO_BIN_EXE_main"))
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
        scopes.contains(&"TOP.tb"),
        "expected slice to include the parent tb scope: {json:?}"
    );
    assert!(
        scopes.contains(&"TOP.tb.dut"),
        "expected slice to traverse into the instantiated dut scope: {json:?}"
    );
    assert!(
        blocks
            .iter()
            .any(|block| block["scope"] == "TOP.tb" && block["block_type"] == "ModOutput"),
        "expected a boundary ModOutput block for the dut instance: {json:?}"
    );

    fixture.cleanup();
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
        "dac26_task15_cli_slice_{}_{}",
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
1!\n\
1\"\n\
1%\n\
b1 &\n\
1$\n",
    )
    .unwrap();

    SliceFixture {
        design,
        testbench,
        vcd,
    }
}
