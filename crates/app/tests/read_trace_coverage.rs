use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use dac26_app::coverage::VcdCoverageTracker;
use dac26_core::coverage::CoverageTracker;
use dac26_core::types::Timestamp;

fn write_trace_coverage_vcd() -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("dac26-task9-{unique}.vcd"));

    fs::write(
        &path,
        "$date\n    today\n$end\n\
$version\n    dac26 task9\n$end\n\
$timescale 1ns $end\n\
$scope module tb $end\n\
$var wire 1 ! clk $end\n\
$var wire 32 \" vlCoverageLineTrace_design__35_stmt [31:0] $end\n\
$var wire 32 # vlCoverageLineTrace_design__40_stmt [31:0] $end\n\
$upscope $end\n\
$enddefinitions $end\n\
#0\n\
0!\n\
b0 \"\n\
b0 #\n\
#5\n\
1!\n\
#10\n\
0!\n\
#12\n\
b1 \"\n\
#15\n\
1!\n\
#20\n\
0!\n\
#25\n\
1!\n\
b10 #\n",
    )
    .unwrap();

    path
}

#[test]
fn interprets_trace_coverage_by_annotation_delta_not_absolute_count() {
    let path = write_trace_coverage_vcd();
    let clk_step = 100;
    let tracker = VcdCoverageTracker::open_with_clock(&path, "tb.clk", clk_step).unwrap();

    assert_eq!(tracker.hit_count_at("design", 35, Timestamp(0)).unwrap(), 0);
    assert_eq!(tracker.delta_hits("design", 35, Timestamp(0)).unwrap(), 0);
    assert!(!tracker
        .is_line_covered_at("design", 35, Timestamp(0))
        .unwrap());

    assert_eq!(
        tracker
            .hit_count_at("design", 35, Timestamp(clk_step))
            .unwrap(),
        1
    );
    assert_eq!(
        tracker
            .delta_hits("design", 35, Timestamp(clk_step))
            .unwrap(),
        1
    );
    assert!(tracker
        .is_line_covered_at("design", 35, Timestamp(clk_step))
        .unwrap());

    assert_eq!(
        tracker
            .hit_count_at("design", 35, Timestamp(2 * clk_step))
            .unwrap(),
        1
    );
    assert_eq!(
        tracker
            .delta_hits("design", 35, Timestamp(2 * clk_step))
            .unwrap(),
        0
    );
    assert!(!tracker
        .is_line_covered_at("design", 35, Timestamp(2 * clk_step))
        .unwrap());

    assert_eq!(
        tracker
            .hit_count_at("design", 40, Timestamp(clk_step))
            .unwrap(),
        0
    );
    assert_eq!(
        tracker
            .delta_hits("design", 40, Timestamp(clk_step))
            .unwrap(),
        0
    );
    assert!(!tracker
        .is_line_covered_at("design", 40, Timestamp(clk_step))
        .unwrap());

    assert_eq!(
        tracker
            .hit_count_at("design", 40, Timestamp(2 * clk_step))
            .unwrap(),
        2
    );
    assert_eq!(
        tracker
            .delta_hits("design", 40, Timestamp(2 * clk_step))
            .unwrap(),
        2
    );
    assert!(tracker
        .is_line_covered_at("design", 40, Timestamp(2 * clk_step))
        .unwrap());

    fs::remove_file(path).unwrap();
}
