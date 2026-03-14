use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn cli_blockize_outputs_json() {
    let fixture = write_fixture(
        "module demo(input logic a, input logic b, output logic y);\n  assign y = a & b;\nendmodule\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_main"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_main"))
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

    let output = Command::new(env!("CARGO_BIN_EXE_main"))
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

fn write_fixture(contents: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "dac26_task13_cli_blockize_{}_{}.sv",
        std::process::id(),
        unique
    ));

    fs::write(&path, contents).unwrap();
    path
}
