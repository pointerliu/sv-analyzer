use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sva_core::ast::AstProvider;
use sva_core::ast::SvParserProvider;
use sva_core::block::DataflowBlockizer;
use sva_core::block::{BlockType, Blockizer, CircuitType};
use sva_core::coverage::CoverageTracker;
use sva_core::coverage::VcdCoverageTracker;
use sva_core::types::{SignalNode, Timestamp};
use sva_core::wave::WaveformReader;
use sva_core::wave::WellenReader;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn demo_design_path() -> PathBuf {
    workspace_root().join("demo/trace_coverage_demo/design.sv")
}

fn demo_vcd_path() -> PathBuf {
    let ws_root = workspace_root();
    ws_root.join("demo/trace_coverage_demo/logs/sim.vcd")
}

#[test]
fn demo_vcd_path_points_to_existing_fixture() {
    assert!(
        demo_vcd_path().is_file(),
        "expected demo VCD fixture at {}",
        demo_vcd_path().display()
    );
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

#[test]
fn fuzzy_searches_waveform_signal_names() {
    let path = write_fixture_vcd();
    let wave = WellenReader::open_metadata(&path).unwrap();

    let matches = wave.search_signal_names("dut state", 3);

    assert_eq!(matches, vec!["tb.dut.state".to_string()]);

    let _ = fs::remove_file(path);
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
        .signal_value_at(&SignalNode::named("TOP.tb.dut.tmp"), Timestamp(35))
        .unwrap()
        .unwrap();
    let normalized_value = wave
        .signal_value_at(&SignalNode::named("tb.dut.tmp"), Timestamp(35))
        .unwrap()
        .unwrap();

    assert_eq!(full_name_value.raw_bits, "000011110");
    assert_eq!(full_name_value.pretty_hex.as_deref(), Some("0x01e"));
    assert_eq!(normalized_value, full_name_value);
}

#[test]
fn state_register_block_marks_else_but_not_reset_branch_at_time_45() {
    let block = demo_trace_block(|block| {
        matches!(block.block_type(), BlockType::Always)
            && matches!(block.circuit_type(), CircuitType::Sequential)
            && block.line_start() == 23
            && block.line_end() == 28
            && block
                .output_signals()
                .iter()
                .any(|signal| signal.name == "state")
    });
    let tracker = VcdCoverageTracker::open(demo_vcd_path()).unwrap();
    let wave = WellenReader::open(demo_vcd_path()).unwrap();
    let time = Timestamp(45);

    assert!(tracker
        .is_line_covered_at(block_file_key(&block), block.line_start(), time)
        .unwrap());
    assert!(!tracker.is_line_covered_at("design", 25, time).unwrap());
    assert!(!tracker.is_line_covered_at("design", 27, time).unwrap());
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__24_if", time),
        0
    );
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__24_else", time),
        1
    );
}

#[test]
fn next_state_block_marks_idle_if_but_not_else_at_time_45() {
    let block = demo_trace_block(|block| {
        matches!(block.block_type(), BlockType::Always)
            && matches!(block.circuit_type(), CircuitType::Combinational)
            && block.line_start() == 31
            && block.line_end() == 50
            && block
                .output_signals()
                .iter()
                .any(|signal| signal.name == "next_state")
    });
    let tracker = VcdCoverageTracker::open(demo_vcd_path()).unwrap();
    let wave = WellenReader::open(demo_vcd_path()).unwrap();
    let time = Timestamp(45);

    assert_eq!(block.line_start(), 31);
    assert!(tracker.is_line_covered_at("design", 34, time).unwrap());
    assert!(tracker.is_line_covered_at("design", 35, time).unwrap());
    assert!(!tracker.is_line_covered_at("design", 38, time).unwrap());
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__34_case", time),
        1
    );
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__35_if", time),
        1
    );
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__35_else", time),
        0
    );
}

#[test]
fn alu_exec_case_is_not_covered_at_time_45() {
    let block = demo_trace_block(|block| {
        matches!(block.block_type(), BlockType::Always)
            && matches!(block.circuit_type(), CircuitType::Sequential)
            && block.line_start() == 53
            && block.line_end() == 79
            && block
                .output_signals()
                .iter()
                .any(|signal| signal.name == "result")
    });
    let tracker = VcdCoverageTracker::open(demo_vcd_path()).unwrap();
    let wave = WellenReader::open(demo_vcd_path()).unwrap();
    let time = Timestamp(45);

    assert!(tracker
        .is_line_covered_at(block_file_key(&block), block.line_start(), time)
        .unwrap());
    assert!(!tracker.is_line_covered_at("design", 54, time).unwrap());
    assert!(tracker.is_line_covered_at("design", 57, time).unwrap());
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__57_if", time),
        0
    );
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__57_else", time),
        1
    );
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__59_case", time),
        0
    );
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__64_case", time),
        0
    );
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__69_case", time),
        0
    );
    assert_eq!(
        counter_delta_at(&wave, "tb.dut.vlCoverageLineTrace_design__73_case", time),
        0
    );
}

#[test]
fn rejects_ambiguous_single_root_aliases() {
    let path = write_collision_fixture_vcd();
    let wave = WellenReader::open(&path).unwrap();

    let exact_a = wave
        .signal_value_at(&SignalNode::named("root_a.leaf.sig"), Timestamp(0))
        .unwrap()
        .unwrap();
    let exact_b = wave
        .signal_value_at(&SignalNode::named("root_b.leaf.sig"), Timestamp(0))
        .unwrap()
        .unwrap();
    let ambiguous = wave
        .signal_value_at(&SignalNode::named("leaf.sig"), Timestamp(0))
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
        .signal_value_at(&SignalNode::named("leaf.sig"), Timestamp(0))
        .unwrap()
        .unwrap();
    let alias_source = wave
        .signal_value_at(&SignalNode::named("root.leaf.sig"), Timestamp(0))
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
        .signal_value_at(&SignalNode::named("tb.wide"), Timestamp(0))
        .unwrap()
        .unwrap();
    let expected_hex = format!("0x{}", "f".repeat(33));

    assert_eq!(value.raw_bits, wide_bits);
    assert_eq!(value.pretty_hex.as_deref(), Some(expected_hex.as_str()));

    fs::remove_file(path).unwrap();
}

fn demo_trace_block(predicate: impl Fn(&sva_core::block::Block) -> bool) -> sva_core::block::Block {
    let parsed = SvParserProvider.parse_files(&[demo_design_path()]).unwrap();

    DataflowBlockizer
        .blockize(&parsed)
        .unwrap()
        .blocks()
        .iter()
        .find(|block| predicate(block))
        .cloned()
        .unwrap()
}

fn block_file_key(block: &sva_core::block::Block) -> &str {
    Path::new(block.source_file())
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap()
}

fn counter_delta_at(wave: &WellenReader, signal: &str, time: Timestamp) -> u64 {
    let current = counter_at(wave, signal, time);
    let previous = if time.0 == 0 {
        0
    } else {
        counter_at(wave, signal, Timestamp(time.0 - 1))
    };

    current.saturating_sub(previous)
}

fn counter_at(wave: &WellenReader, signal: &str, time: Timestamp) -> u64 {
    let value = wave
        .signal_value_at(&SignalNode::named(signal), time)
        .unwrap()
        .unwrap();

    u64::from_str_radix(&value.raw_bits, 2).unwrap()
}
