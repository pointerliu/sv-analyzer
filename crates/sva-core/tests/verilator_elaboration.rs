use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use sva_core::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use sva_core::coverage::{CoverageTracker, ElaboratedCoverageTracker, VerilatorElaborationIndex};
use sva_core::slicer::{BluesSlicer, SliceRequest, TimedSliceNode};
use sva_core::types::{BlockId, SignalNode, Timestamp};

#[test]
fn parses_native_verilator_tree_json_locations() {
    let path = write_tree_json_with_meta(
        r#"{
  "type": "NETLIST",
  "loc": "<built-in>,0:0,0:0",
  "modulesp": [
    {
      "type": "MODULE",
      "loc": "/repo/rtl/live.sv,1:8,1:12",
      "stmtsp": [
        { "type": "VAR", "loc": "/repo/rtl/live.sv,50:3,50:8" },
        { "type": "ASSIGNW", "loc": "/repo/rtl/live.sv,10:3,10:14" },
        {
          "type": "ALWAYS",
          "loc": "../rtl/live.sv,20:1,25:4",
          "stmtsp": [
            { "type": "ASSIGNDLY", "loc": "../rtl/live.sv,22:9,22:18" }
          ]
        },
        { "type": "ASSIGN", "loc": "not-a-location" }
      ]
    },
    {
      "type": "MODULE",
      "loc": "f0,1:8,1:12",
      "stmtsp": [
        { "type": "ASSIGNW", "loc": "f0,30:3,30:14" }
      ]
    }
  ]
}"#,
        r#"{
  "files": {
    "f0": {
      "filename": "../rtl/file_id_only.sv",
      "realpath": "/repo/rtl/file_id_only.sv"
    }
  }
}"#,
    );

    let index = VerilatorElaborationIndex::from_tree_json_file(&path).unwrap();

    assert!(index.is_assign_like_elaborated("/other/path/live.sv", 10, 10));
    assert!(index.is_always_elaborated("live", 20, 25));
    assert!(index.is_assign_like_elaborated("live.sv", 22, 22));
    assert!(index.is_assign_like_elaborated("file_id_only.sv", 30, 30));
    assert!(!index.is_assign_like_elaborated("live.sv", 50, 50));
    assert!(!index.is_block_elaborated("missing.sv", 10, 10));

    cleanup_tree_fixture(path);
}

#[test]
fn blues_prunes_same_signal_driver_from_non_elaborated_generate_branch() {
    let block_set = BlockSet::new(vec![
        Block::builder()
            .id(BlockId(1))
            .block_type(BlockType::Assign)
            .circuit_type(CircuitType::Combinational)
            .module_scope("TOP.dut")
            .source_file("/repo/rtl/if_stage.sv")
            .lines(100, 100)
            .unwrap()
            .dataflow(vec![entry("TOP.dut.pc_id_o", &["TOP.dut.live_pc"])])
            .code_snippet("assign pc_id_o = live_pc;")
            .build()
            .unwrap(),
        Block::builder()
            .id(BlockId(2))
            .block_type(BlockType::Assign)
            .circuit_type(CircuitType::Combinational)
            .module_scope("TOP.dut")
            .source_file("/repo/rtl/if_stage.sv")
            .lines(200, 200)
            .unwrap()
            .dataflow(vec![entry("TOP.dut.pc_id_o", &["TOP.dut.dead_pc"])])
            .code_snippet("assign pc_id_o = dead_pc;")
            .build()
            .unwrap(),
    ])
    .unwrap();
    let path = write_tree_json(
        r#"{
  "type": "NETLIST",
  "stmtsp": [
    { "type": "ASSIGNW", "loc": "/repo/rtl/if_stage.sv,100:3,100:25" }
  ]
}"#,
    );
    let elaboration = VerilatorElaborationIndex::from_tree_json_file(&path).unwrap();
    let coverage = Arc::new(ElaboratedCoverageTracker::new(
        Arc::new(FixtureCoverage),
        elaboration,
    ));

    let path = BluesSlicer::new(block_set, coverage)
        .slice(&SliceRequest {
            signal: SignalNode::named("TOP.dut.pc_id_o"),
            time: Timestamp(5),
            min_time: Timestamp(0),
        })
        .unwrap();

    assert!(path.nodes.iter().any(|node| matches!(
        node,
        TimedSliceNode::Block {
            block_id: BlockId(1),
            time: Some(Timestamp(5)),
        }
    )));
    assert!(!path.nodes.iter().any(|node| matches!(
        node,
        TimedSliceNode::Block {
            block_id: BlockId(2),
            ..
        }
    )));
}

#[derive(Debug)]
struct FixtureCoverage;

impl CoverageTracker for FixtureCoverage {
    fn is_line_covered_at(&self, _file: &str, _line: usize, _time: Timestamp) -> Result<bool> {
        Ok(true)
    }

    fn hit_count_at(&self, _file: &str, _line: usize, _time: Timestamp) -> Result<u64> {
        Ok(1)
    }

    fn delta_hits(&self, _file: &str, _line: usize, _time: Timestamp) -> Result<u64> {
        Ok(1)
    }

    fn clock_period(&self) -> Option<i64> {
        Some(1)
    }

    fn is_posedge_time(&self, _time: i64) -> bool {
        true
    }
}

fn entry(output: &str, inputs: &[&str]) -> DataflowEntry {
    DataflowEntry {
        output: vec![SignalNode::named(output)],
        inputs: inputs
            .iter()
            .map(|input| SignalNode::named(*input))
            .collect(),
    }
}

fn write_tree_json(contents: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sva-verilator-tree-{unique}.json"));
    fs::write(&path, contents).unwrap();
    path
}

fn write_tree_json_with_meta(contents: &str, meta_contents: &str) -> std::path::PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("sva-verilator-tree-{unique}.tree.json"));
    let meta_path = path.with_file_name(
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .replace(".tree.json", ".tree.meta.json"),
    );
    fs::write(&path, contents).unwrap();
    fs::write(meta_path, meta_contents).unwrap();
    path
}

fn cleanup_tree_fixture(path: std::path::PathBuf) {
    let meta_path = path.with_file_name(
        path.file_name()
            .unwrap()
            .to_string_lossy()
            .replace(".tree.json", ".tree.meta.json"),
    );
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(meta_path);
}
