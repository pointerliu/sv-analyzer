use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

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

const ASSIGNMENT_LINES: &[usize] = &[
    25, 27, 32, 36, 38, 41, 44, 47, 55, 56, 60, 61, 62, 65, 66, 67, 70, 71, 74, 75,
];

#[test]
fn cli_reports_assignment_statement_coverage_for_demo_wave() {
    let output_45 = Command::new(main_bin())
        .args([
            "coverage",
            "--sv",
            workspace_root()
                .join("demo/trace_coverage_demo/design.sv")
                .to_str()
                .unwrap(),
            "--vcd",
            workspace_root()
                .join("demo/trace_coverage_demo/logs/sim.vcd")
                .to_str()
                .unwrap(),
            "--time",
            "45",
        ])
        .output()
        .unwrap();

    assert!(
        output_45.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output_45.stdout),
        String::from_utf8_lossy(&output_45.stderr)
    );

    let output_65 = Command::new(main_bin())
        .args([
            "coverage",
            "--sv",
            workspace_root()
                .join("demo/trace_coverage_demo/design.sv")
                .to_str()
                .unwrap(),
            "--vcd",
            workspace_root()
                .join("demo/trace_coverage_demo/logs/sim.vcd")
                .to_str()
                .unwrap(),
            "--time",
            "65",
        ])
        .output()
        .unwrap();

    assert!(
        output_65.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output_65.stdout),
        String::from_utf8_lossy(&output_65.stderr)
    );

    let json_45: Value = serde_json::from_slice(&output_45.stdout).unwrap();
    let json_65: Value = serde_json::from_slice(&output_65.stdout).unwrap();

    assert_assignment_lines(&json_45, ASSIGNMENT_LINES);
    assert_assignment_lines(&json_65, ASSIGNMENT_LINES);
    assert_covered_lines(&json_45, &[27, 32, 36]);
    assert_uncovered_lines(
        &json_45,
        &[
            25, 38, 41, 44, 47, 55, 56, 60, 61, 62, 65, 66, 67, 70, 71, 74, 75,
        ],
    );
    assert_covered_lines(&json_65, &[27, 32, 44, 65, 66, 67]);
    assert_uncovered_lines(
        &json_65,
        &[25, 36, 38, 41, 47, 55, 56, 60, 61, 62, 70, 71, 74, 75],
    );
}

fn assert_assignment_lines(json: &Value, expected: &[usize]) {
    let mut lines = json["covered"]
        .as_array()
        .unwrap()
        .iter()
        .chain(json["uncovered"].as_array().unwrap().iter())
        .map(|entry| entry["line"].as_u64().unwrap() as usize)
        .collect::<Vec<_>>();
    lines.sort_unstable();

    assert_eq!(lines, expected);
}

fn assert_covered_lines(json: &Value, expected: &[usize]) {
    let mut lines = json["covered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["line"].as_u64().unwrap() as usize)
        .collect::<Vec<_>>();
    lines.sort_unstable();

    assert_eq!(lines, expected);
}

fn assert_uncovered_lines(json: &Value, expected: &[usize]) {
    let mut lines = json["uncovered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["line"].as_u64().unwrap() as usize)
        .collect::<Vec<_>>();
    lines.sort_unstable();

    assert_eq!(lines, expected);
}
