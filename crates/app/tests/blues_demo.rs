use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;

use dac26_app::slicer::BluesSlicer;
use dac26_core::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use dac26_core::coverage::CoverageTracker;
use dac26_core::slicer::{SliceRequest, TimedSliceNode};
use dac26_core::types::{BlockId, SignalNode, SignalNodeKind, Timestamp};

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
            signal: SignalNode::named("result"),
            time: Timestamp(3),
            min_time: Timestamp(1),
        })
        .unwrap();

    assert_eq!(
        path.nodes
            .iter()
            .filter_map(|node| match node {
                TimedSliceNode::Block {
                    block_id,
                    time: Some(time),
                } => Some((block_id.0, time.0)),
                TimedSliceNode::Block { time: None, .. } => None,
                TimedSliceNode::Literal { .. } => None,
            })
            .collect::<Vec<_>>(),
        vec![(1, 1), (2, 2), (2, 3)]
    );
    assert_eq!(
        path.edges
            .iter()
            .map(|edge| (
                match &edge.from {
                    TimedSliceNode::Block {
                        block_id,
                        time: Some(time),
                    } => Some((block_id.0, time.0)),
                    TimedSliceNode::Block { time: None, .. } => None,
                    TimedSliceNode::Literal { .. } => None,
                },
                match &edge.to {
                    TimedSliceNode::Block {
                        block_id,
                        time: Some(time),
                    } => Some((block_id.0, time.0)),
                    TimedSliceNode::Block { time: None, .. } => None,
                    TimedSliceNode::Literal { .. } => None,
                },
                edge.signal.as_ref().map(|signal| signal.name.as_str()),
            ))
            .collect::<Vec<_>>(),
        vec![
            (Some((1, 1)), Some((2, 2)), Some("tmp")),
            (Some((2, 2)), Some((2, 3)), Some("result")),
        ]
    );
    assert!(path.nodes.iter().all(|node| match node {
        TimedSliceNode::Block {
            time: Some(time), ..
        }
        | TimedSliceNode::Literal {
            time: Some(time), ..
        } => time.0 >= 1,
        TimedSliceNode::Block { time: None, .. } | TimedSliceNode::Literal { time: None, .. } =>
            false,
    }));
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
            signal: SignalNode::named("result"),
            time: Timestamp(5),
            min_time: Timestamp(0),
        })
        .unwrap();

    assert!(path.nodes.iter().any(|node| matches!(
        node,
        TimedSliceNode::Block {
            block_id: BlockId(1),
            ..
        }
    )));
    assert!(path.nodes.iter().any(|node| matches!(
        node,
        TimedSliceNode::Block {
            block_id: BlockId(2),
            ..
        }
    )));
    assert!(path.edges.iter().any(|edge| {
        matches!(
            edge.from,
            TimedSliceNode::Block {
                block_id: BlockId(1),
                ..
            }
        ) && matches!(
            edge.to,
            TimedSliceNode::Block {
                block_id: BlockId(3),
                ..
            }
        ) && edge.signal.as_ref().map(|signal| signal.name.as_str()) == Some("left_src")
    }));
    assert!(path.edges.iter().any(|edge| {
        matches!(
            edge.from,
            TimedSliceNode::Block {
                block_id: BlockId(2),
                ..
            }
        ) && matches!(
            edge.to,
            TimedSliceNode::Block {
                block_id: BlockId(3),
                ..
            }
        ) && edge.signal.as_ref().map(|signal| signal.name.as_str()) == Some("right_src")
    }));
}

#[test]
fn blues_keeps_literals_as_terminal_nodes() {
    let block_set = BlockSet::new(vec![Block::new(
        BlockId(2),
        BlockType::Always,
        CircuitType::Sequential,
        "demo",
        "design.sv",
        53,
        55,
        vec![DataflowEntry {
            output: vec![SignalNode::named("result")],
            inputs: HashSet::from([SignalNode::named("rst_n"), SignalNode::literal("8'h0")]),
        }],
        "always_ff @(posedge clk or negedge rst_n) if (!rst_n) result <= 8'h0;",
    )
    .unwrap()])
    .unwrap();

    let path = BluesSlicer::new(
        block_set,
        Arc::new(FixtureCoverage::covered([("design.sv", 53, 2)])),
    )
    .slice(&SliceRequest {
        signal: SignalNode::named("result"),
        time: Timestamp(3),
        min_time: Timestamp(0),
    })
    .unwrap();

    assert!(path.nodes.iter().any(|node| match node {
        TimedSliceNode::Literal {
            signal,
            time: Some(time),
        } => {
            signal.kind == SignalNodeKind::Literal && signal.name == "8'h0" && time.0 == 2
        }
        _ => false,
    }));
    assert!(path.edges.iter().any(|edge| match (&edge.from, &edge.to) {
        (
            TimedSliceNode::Literal {
                signal,
                time: Some(time),
            },
            TimedSliceNode::Block {
                block_id,
                time: Some(sink_time),
            },
        ) => {
            signal.name == "8'h0"
                && time.0 == 2
                && block_id.0 == 2
                && sink_time.0 == 3
                && edge.signal.is_none()
        }
        _ => false,
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
            .collect::<HashSet<_>>(),
    }
}
