use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

fn main_bin() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_BIN_EXE_sva_cli"))
}

#[test]
fn cli_wave_outputs_json_signal_value() {
    let fixture = write_wave_vcd();

    let output = Command::new(main_bin())
        .args([
            "wave",
            "--vcd",
            fixture.to_str().unwrap(),
            "--signal",
            "tb.dut.state",
            "--time",
            "10",
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
    assert_eq!(json["signal"], "tb.dut.state");
    assert_eq!(json["time"], 10);
    assert_eq!(json["value"]["raw_bits"], "0011");
    assert_eq!(json["value"]["pretty_hex"], "0x3");

    let _ = fs::remove_file(fixture);
}

fn write_wave_vcd() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "sva_task14_cli_wave_{}_{}.vcd",
        std::process::id(),
        unique
    ));

    fs::write(
        &path,
        "$date\n    today\n$end\n\
$version\n    dac26 task14 wave\n$end\n\
$timescale 1ns $end\n\
$scope module tb $end\n\
$scope module dut $end\n\
$var wire 4 ! state [3:0] $end\n\
$upscope $end\n\
$upscope $end\n\
$enddefinitions $end\n\
#0\n\
b1010 !\n\
#10\n\
b0011 !\n",
    )
    .unwrap();

    path
}
