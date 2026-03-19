use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn main_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("target")
        .join("debug")
        .join("dataflow-engine")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn cli_blockize_outputs_json() {
    let fixture = write_fixture(
        "module demo(input logic a, input logic b, output logic y);\n  assign y = a & b;\nendmodule\n",
    );

    let output = Command::new(main_bin())
        .args(["blockize", "--sv", fixture.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).unwrap();
    let object = json.as_object().unwrap();

    assert_eq!(object.len(), 2, "unexpected top-level shape: {object:?}");
    assert!(object.contains_key("blocks"));
    assert!(object.contains_key("signal_to_drivers"));
    assert!(json["blocks"].is_array());
    assert!(json["signal_to_drivers"].is_object());

    let _ = fs::remove_file(fixture);
}

#[test]
fn cli_blockize_requires_sv_argument() {
    let output = Command::new(main_bin())
        .args(["blockize"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--sv <SV_FILES>"));
}

#[test]
fn cli_blockize_accepts_repeated_sv_arguments() {
    let fixture_a = write_fixture(
        "module demo_a(input logic a, output logic y);\n  assign y = a;\nendmodule\n",
    );
    let fixture_b = write_fixture(
        "module demo_b(input logic b, output logic z);\n  assign z = b;\nendmodule\n",
    );

    let output = Command::new(main_bin())
        .args([
            "blockize",
            "--sv",
            fixture_a.to_str().unwrap(),
            "--sv",
            fixture_b.to_str().unwrap(),
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
    assert!(
        blocks.len() >= 4,
        "expected combined blocks from both files: {blocks:?}"
    );

    let _ = fs::remove_file(fixture_a);
    let _ = fs::remove_file(fixture_b);
}

#[test]
fn cli_blockize_processes_multi_submodule_demo_modules() {
    let design = workspace_root().join("demo/multi_submodule_demo/design.sv");
    let testbench = workspace_root().join("demo/multi_submodule_demo/tb.sv");

    let output = Command::new(main_bin())
        .args([
            "blockize",
            "--sv",
            design.to_str().unwrap(),
            "--sv",
            testbench.to_str().unwrap(),
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
    let scopes = json["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|block| block["module_scope"].as_str())
        .collect::<std::collections::BTreeSet<_>>();

    assert!(scopes.contains("top"), "missing top blocks: {json:?}");
    assert!(
        scopes.contains("submodule1"),
        "missing submodule1 blocks: {json:?}"
    );
    assert!(
        scopes.contains("submodule2"),
        "missing submodule2 blocks: {json:?}"
    );

    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"] == "submodule1" && block["block_type"] == "Assign"
        }),
        "expected assign logic in submodule1: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"] == "submodule1" && block["block_type"] == "Always"
        }),
        "expected always/always_comb logic in submodule1: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"] == "submodule2" && block["block_type"] == "Always"
        }),
        "expected sequential always logic in submodule2: {json:?}"
    );
    assert!(
        json["blocks"].as_array().unwrap().iter().any(|block| {
            block["module_scope"] == "submodule2" && block["block_type"] == "Assign"
        }),
        "expected assign logic in submodule2: {json:?}"
    );
}

fn write_fixture(contents: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sva_task13_cli_blockize_{}_{}.sv",
        std::process::id(),
        unique
    ));

    fs::write(&path, contents).unwrap();
    path
}
