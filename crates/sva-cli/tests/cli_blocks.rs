use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::json;

fn main_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_sva_cli"))
}

#[test]
fn cli_blocks_query_filters_by_block_id() {
    let fixture = write_blocks_fixture();

    let output = Command::new(main_bin())
        .args([
            "blocks",
            "query",
            "--input",
            fixture.to_str().unwrap(),
            "--block-id",
            "8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["match_count"], 1);
    assert_eq!(json["blocks"][0]["id"], 8);
    assert!(json["blocks"][0].get("dataflow").is_some());
    assert!(json["blocks"][0].get("code_snippet").is_some());

    let _ = fs::remove_file(fixture);
}

#[test]
fn cli_blocks_query_applies_all_requested_filters() {
    let fixture = write_blocks_fixture();

    let output = Command::new(main_bin())
        .args([
            "blocks",
            "query",
            "--input",
            fixture.to_str().unwrap(),
            "--output-signal",
            "TOP.a.b.out0",
            "--output-signal",
            "TOP.a.b.out1",
            "--input-signal",
            "TOP.a.b.in0",
            "--input-signal",
            "TOP.a.b.in1",
            "--scope",
            "TOP.a",
            "--block-type",
            "always",
            "--circuit-type",
            "sequential",
            "--source-file",
            "rtl/child.sv",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["match_count"], 1);
    assert_eq!(json["blocks"][0]["id"], 7);
    assert_eq!(json["blocks"][0]["module_scope"], "TOP.a.b");

    let _ = fs::remove_file(fixture);
}

fn write_blocks_fixture() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sva_cli_blocks_query_{}_{}.json",
        std::process::id(),
        unique
    ));

    let locate = json!({
        "offset": 0,
        "line": 0,
        "ast_line": 0,
        "len": 12
    });

    let json = json!({
        "blocks": [
            {
                "id": 7,
                "block_type": "Always",
                "circuit_type": "Sequential",
                "module_scope": "TOP.a.b",
                "source_file": "/tmp/project/rtl/child.sv",
                "line_start": 10,
                "line_end": 20,
                "ast_line_start": 10,
                "ast_line_end": 20,
                "input_signals": ["TOP.a.b.in0", "TOP.a.b.in1"],
                "output_signals": ["TOP.a.b.out0", "TOP.a.b.out1"],
                "dataflow": [
                    {
                        "output": [
                            {"kind": "variable", "name": "TOP.a.b.out0", "locate": locate},
                            {"kind": "variable", "name": "TOP.a.b.out1", "locate": locate}
                        ],
                        "inputs": [
                            {"kind": "variable", "name": "TOP.a.b.in0", "locate": locate},
                            {"kind": "variable", "name": "TOP.a.b.in1", "locate": locate}
                        ]
                    }
                ],
                "code_snippet": "always_ff @(posedge clk) begin out0 <= in0; out1 <= in1; end"
            },
            {
                "id": 8,
                "block_type": "Assign",
                "circuit_type": "Combinational",
                "module_scope": "TOP.a.b",
                "source_file": "/tmp/project/rtl/child.sv",
                "line_start": 22,
                "line_end": 22,
                "ast_line_start": 22,
                "ast_line_end": 22,
                "input_signals": ["TOP.a.b.in0"],
                "output_signals": ["TOP.a.b.out2"],
                "dataflow": [
                    {
                        "output": [
                            {"kind": "variable", "name": "TOP.a.b.out2", "locate": locate}
                        ],
                        "inputs": [
                            {"kind": "variable", "name": "TOP.a.b.in0", "locate": locate}
                        ]
                    }
                ],
                "code_snippet": "assign out2 = in0;"
            },
            {
                "id": 9,
                "block_type": "Always",
                "circuit_type": "Sequential",
                "module_scope": "TOP.c",
                "source_file": "/tmp/project/rtl/other.sv",
                "line_start": 30,
                "line_end": 40,
                "ast_line_start": 30,
                "ast_line_end": 40,
                "input_signals": ["TOP.c.in0", "TOP.c.in1"],
                "output_signals": ["TOP.c.out0", "TOP.c.out1"],
                "dataflow": [],
                "code_snippet": "always_ff @(posedge clk) begin out0 <= in0; out1 <= in1; end"
            }
        ],
        "signal_to_drivers": {
            "TOP.a.b.out0": [7],
            "TOP.a.b.out1": [7],
            "TOP.a.b.out2": [8],
            "TOP.c.out0": [9],
            "TOP.c.out1": [9]
        }
    });

    fs::write(&path, serde_json::to_vec_pretty(&json).unwrap()).unwrap();
    path
}
