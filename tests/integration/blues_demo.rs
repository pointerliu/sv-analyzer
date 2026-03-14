use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;

use dac26_mcp::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use dac26_mcp::coverage::CoverageTracker;
use dac26_mcp::slicer::{BluesSlicer, SliceRequest};
use dac26_mcp::types::{BlockId, SignalId, Timestamp};

#[test]
fn blues_backtracks_sequential_state_until_coverage_hit_and_respects_min_time() {
    let block_set = BlockSet::new(vec![
        Block::new(
            BlockId(1),
            BlockType::Assign,
            CircuitType::Combinational,
            "demo",
            "design.sv",
            10,
            10,
            vec![entry("tmp", &["a", "b"])],
            "assign tmp = a & b;",
        )
        .unwrap(),
        Block::new(
            BlockId(2),
            BlockType::Always,
            CircuitType::Sequential,
            "demo",
            "design.sv",
            20,
            20,
            vec![entry("result", &["tmp"])],
            "always_ff @(posedge clk) result <= tmp;",
        )
        .unwrap(),
    ])
    .unwrap();

    let slicer = BluesSlicer::new(
        block_set,
        Arc::new(FixtureCoverage::covered([("design.sv", 20, 1)])),
    );

    let path = slicer
        .slice(&SliceRequest {
            signal: SignalId("result".into()),
            time: Timestamp(3),
            min_time: Timestamp(1),
        })
        .unwrap();

    assert_eq!(
        path.nodes
            .iter()
            .map(|node| (node.block_id.0, node.time.0))
            .collect::<Vec<_>>(),
        vec![(1, 1), (2, 2), (2, 3)]
    );
    assert_eq!(
        path.edges
            .iter()
            .map(|edge| (
                edge.from.block_id.0,
                edge.from.time.0,
                edge.to.block_id.0,
                edge.to.time.0,
                edge.signal.as_ref().map(|signal| signal.0.as_str()),
            ))
            .collect::<Vec<_>>(),
        vec![(1, 1, 2, 2, Some("tmp")), (2, 2, 2, 3, Some("result"))]
    );
    assert!(path.nodes.iter().all(|node| node.time.0 >= 1));
}

#[test]
fn blues_keeps_dependencies_from_distinct_outputs_of_same_block() {
    let block_set = BlockSet::new(vec![
        Block::new(
            BlockId(1),
            BlockType::Assign,
            CircuitType::Combinational,
            "demo",
            "design.sv",
            10,
            10,
            vec![entry("left_src", &["a"])],
            "assign left_src = a;",
        )
        .unwrap(),
        Block::new(
            BlockId(2),
            BlockType::Assign,
            CircuitType::Combinational,
            "demo",
            "design.sv",
            11,
            11,
            vec![entry("right_src", &["b"])],
            "assign right_src = b;",
        )
        .unwrap(),
        Block::new(
            BlockId(3),
            BlockType::Assign,
            CircuitType::Combinational,
            "demo",
            "design.sv",
            12,
            13,
            vec![entry("left", &["left_src"]), entry("right", &["right_src"])],
            "assign left = left_src; assign right = right_src;",
        )
        .unwrap(),
        Block::new(
            BlockId(4),
            BlockType::Assign,
            CircuitType::Combinational,
            "demo",
            "design.sv",
            14,
            14,
            vec![entry("result", &["left", "right"])],
            "assign result = left ^ right;",
        )
        .unwrap(),
    ])
    .unwrap();

    let slicer = BluesSlicer::new(block_set, Arc::new(FixtureCoverage::covered([])));

    let path = slicer
        .slice(&SliceRequest {
            signal: SignalId("result".into()),
            time: Timestamp(5),
            min_time: Timestamp(0),
        })
        .unwrap();

    assert!(path.nodes.iter().any(|node| node.block_id.0 == 1));
    assert!(path.nodes.iter().any(|node| node.block_id.0 == 2));
    assert!(path.edges.iter().any(|edge| {
        edge.from.block_id.0 == 1
            && edge.to.block_id.0 == 3
            && edge.signal.as_ref().map(|signal| signal.0.as_str()) == Some("left_src")
    }));
    assert!(path.edges.iter().any(|edge| {
        edge.from.block_id.0 == 2
            && edge.to.block_id.0 == 3
            && edge.signal.as_ref().map(|signal| signal.0.as_str()) == Some("right_src")
    }));
}

#[derive(Debug, Default)]
struct FixtureCoverage {
    covered_lines: HashSet<(String, usize, i64)>,
}

impl FixtureCoverage {
    fn covered<const N: usize>(entries: [(&str, usize, i64); N]) -> Self {
        Self {
            covered_lines: entries
                .into_iter()
                .map(|(file, line, time)| (file.to_string(), line, time))
                .collect(),
        }
    }
}

impl CoverageTracker for FixtureCoverage {
    fn is_line_covered_at(&self, file: &str, line: usize, time: Timestamp) -> Result<bool> {
        Ok(self
            .covered_lines
            .contains(&(file.to_string(), line, time.0)))
    }

    fn hit_count_at(&self, file: &str, line: usize, time: Timestamp) -> Result<u64> {
        Ok(u64::from(self.is_line_covered_at(file, line, time)?))
    }

    fn delta_hits(&self, file: &str, line: usize, time: Timestamp) -> Result<u64> {
        self.hit_count_at(file, line, time)
    }
}

fn entry(output: &str, inputs: &[&str]) -> DataflowEntry {
    DataflowEntry {
        output: SignalId(output.into()),
        inputs: inputs
            .iter()
            .map(|input| SignalId((*input).into()))
            .collect::<HashSet<_>>(),
    }
}
