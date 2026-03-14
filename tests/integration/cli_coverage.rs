use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

#[test]
fn cli_coverage_outputs_json_hit_info() {
    let fixture = write_trace_coverage_vcd();

    let output = Command::new(env!("CARGO_BIN_EXE_main"))
        .args([
            "coverage",
            "--vcd",
            fixture.to_str().unwrap(),
            "--file",
            "design",
            "--line",
            "35",
            "--time",
            "12",
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
    assert_eq!(json["file"], "design");
    assert_eq!(json["line"], 35);
    assert_eq!(json["time"], 12);
    assert_eq!(json["hit_count"], 1);
    assert_eq!(json["delta_hits"], 1);
    assert_eq!(json["is_covered"], true);

    let _ = fs::remove_file(fixture);
}

fn write_trace_coverage_vcd() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "dac26_task14_cli_coverage_{}_{}.vcd",
        std::process::id(),
        unique
    ));

    fs::write(
        &path,
        "$date\n    today\n$end\n\
$version\n    dac26 task14 coverage\n$end\n\
$timescale 1ns $end\n\
$scope module tb $end\n\
$var wire 32 ! vlCoverageLineTrace_design__35_stmt [31:0] $end\n\
$upscope $end\n\
$enddefinitions $end\n\
#0\n\
b0 !\n\
#12\n\
b1 !\n",
    )
    .unwrap();

    path
}
