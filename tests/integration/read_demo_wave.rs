use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use dac26_mcp::types::{SignalId, Timestamp};
use dac26_mcp::wave::{WaveformReader, WellenReader};

fn demo_vcd_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("demo/trace_coverage_demo/logs/sim.vcd")
}

fn write_fixture_vcd() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dac26-task8-{unique}.vcd"));

    fs::write(
        &path,
        "$date\n    today\n$end\n\
$version\n    dac26 task8\n$end\n\
$timescale 1ns $end\n\
$scope module tb $end\n\
$scope module dut $end\n\
$var wire 4 ! state [3:0] $end\n\
$var wire 1 \" valid $end\n\
$upscope $end\n\
$upscope $end\n\
$enddefinitions $end\n\
#0\n\
b1010 !\n\
1\"\n\
#10\n\
b0011 !\n\
0\"\n",
    )
    .unwrap();

    path
}

fn write_collision_fixture_vcd() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dac26-task8-collision-{unique}.vcd"));

    fs::write(
        &path,
        "$date\n    today\n$end\n\
$version\n    dac26 task8 collision\n$end\n\
$timescale 1ns $end\n\
$scope module root_a $end\n\
$scope module leaf $end\n\
$var wire 1 ! sig $end\n\
$upscope $end\n\
$upscope $end\n\
$scope module root_b $end\n\
$scope module leaf $end\n\
$var wire 1 \" sig $end\n\
$upscope $end\n\
$upscope $end\n\
$enddefinitions $end\n\
#0\n\
1!\n\
0\"\n",
    )
    .unwrap();

    path
}

fn write_exact_vs_alias_fixture_vcd() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dac26-task8-exact-alias-{unique}.vcd"));

    fs::write(
        &path,
        "$date\n    today\n$end\n\
$version\n    dac26 task8 exact alias\n$end\n\
$timescale 1ns $end\n\
$scope module root $end\n\
$scope module leaf $end\n\
$var wire 1 ! sig $end\n\
$upscope $end\n\
$upscope $end\n\
$scope module leaf $end\n\
$var wire 1 \" sig $end\n\
$upscope $end\n\
$enddefinitions $end\n\
#0\n\
1!\n\
0\"\n",
    )
    .unwrap();

    path
}

#[test]
fn reads_signal_value_from_demo_vcd_by_hierarchical_name() {
    let wave = WellenReader::open(demo_vcd_path()).unwrap();

    let full_name_value = wave
        .signal_value_at(&SignalId("TOP.tb.dut.tmp".into()), Timestamp(35))
        .unwrap()
        .unwrap();
    let normalized_value = wave
        .signal_value_at(&SignalId("tb.dut.tmp".into()), Timestamp(35))
        .unwrap()
        .unwrap();

    assert_eq!(full_name_value.raw_bits, "000011110");
    assert_eq!(full_name_value.pretty_hex.as_deref(), Some("0x01e"));
    assert_eq!(normalized_value, full_name_value);
}

#[test]
fn rejects_ambiguous_single_root_aliases() {
    let path = write_collision_fixture_vcd();
    let wave = WellenReader::open(&path).unwrap();

    let exact_a = wave
        .signal_value_at(&SignalId("root_a.leaf.sig".into()), Timestamp(0))
        .unwrap()
        .unwrap();
    let exact_b = wave
        .signal_value_at(&SignalId("root_b.leaf.sig".into()), Timestamp(0))
        .unwrap()
        .unwrap();
    let ambiguous = wave
        .signal_value_at(&SignalId("leaf.sig".into()), Timestamp(0))
        .unwrap();

    assert_eq!(exact_a.raw_bits, "1");
    assert_eq!(exact_b.raw_bits, "0");
    assert_eq!(ambiguous, None);

    fs::remove_file(path).unwrap();
}

#[test]
fn exact_full_name_wins_over_conflicting_alias() {
    let path = write_exact_vs_alias_fixture_vcd();
    let wave = WellenReader::open(&path).unwrap();

    let exact = wave
        .signal_value_at(&SignalId("leaf.sig".into()), Timestamp(0))
        .unwrap()
        .unwrap();
    let alias_source = wave
        .signal_value_at(&SignalId("root.leaf.sig".into()), Timestamp(0))
        .unwrap()
        .unwrap();

    assert_eq!(exact.raw_bits, "0");
    assert_eq!(alias_source.raw_bits, "1");

    fs::remove_file(path).unwrap();
}

#[test]
fn pretty_hex_supports_arbitrary_width_clean_bitvectors() {
    let path = write_fixture_vcd();
    let wide_bits = "1111".repeat(33);
    let vcd = format!(
        "$date\n    today\n$end\n\
$version\n    dac26 task8\n$end\n\
$timescale 1ns $end\n\
$scope module tb $end\n\
$var wire 132 ! wide [131:0] $end\n\
$upscope $end\n\
$enddefinitions $end\n\
#0\n\
b{wide_bits} !\n"
    );

    fs::write(&path, vcd).unwrap();

    let wave = WellenReader::open(&path).unwrap();

    let value = wave
        .signal_value_at(&SignalId("tb.wide".into()), Timestamp(0))
        .unwrap()
        .unwrap();
    let expected_hex = format!("0x{}", "f".repeat(33));

    assert_eq!(value.raw_bits, wide_bits);
    assert_eq!(value.pretty_hex.as_deref(), Some(expected_hex.as_str()));

    fs::remove_file(path).unwrap();
}
