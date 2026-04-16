use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;

use sva_core::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use sva_core::coverage::CoverageTracker;
use sva_core::slicer::BluesSlicer;
use sva_core::slicer::{SliceRequest, TimedSliceNode};
use sva_core::types::{BlockId, SignalNode, SignalNodeKind, Timestamp};

#[test]
fn blues_backtracks_sequential_state_until_coverage_hit_and_respects_min_time() {
    let block_set = BlockSet::new(vec![
        Block::builder()
            .id(BlockId(1))
            .block_type(BlockType::Assign)
            .circuit_type(CircuitType::Combinational)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(10, 10)
            .unwrap()
            .dataflow(vec![entry("tmp", &["a", "b"])])
            .code_snippet("assign tmp = a & b;")
            .build()
            .unwrap(),
        Block::builder()
            .id(BlockId(2))
            .block_type(BlockType::Always)
            .circuit_type(CircuitType::Sequential)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(20, 20)
            .unwrap()
            .dataflow(vec![entry("result", &["tmp"])])
            .code_snippet("always_ff @(posedge clk) result <= tmp;")
            .build()
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
        Block::builder()
            .id(BlockId(1))
            .block_type(BlockType::Assign)
            .circuit_type(CircuitType::Combinational)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(10, 10)
            .unwrap()
            .dataflow(vec![entry("left_src", &["a"])])
            .code_snippet("assign left_src = a;")
            .build()
            .unwrap(),
        Block::builder()
            .id(BlockId(2))
            .block_type(BlockType::Assign)
            .circuit_type(CircuitType::Combinational)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(11, 11)
            .unwrap()
            .dataflow(vec![entry("right_src", &["b"])])
            .code_snippet("assign right_src = b;")
            .build()
            .unwrap(),
        Block::builder()
            .id(BlockId(3))
            .block_type(BlockType::Assign)
            .circuit_type(CircuitType::Combinational)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(12, 13)
            .unwrap()
            .dataflow(vec![
                entry("left", &["left_src"]),
                entry("right", &["right_src"]),
            ])
            .code_snippet("assign left = left_src; assign right = right_src;")
            .build()
            .unwrap(),
        Block::builder()
            .id(BlockId(4))
            .block_type(BlockType::Assign)
            .circuit_type(CircuitType::Combinational)
            .module_scope("demo")
            .source_file("design.sv")
            .lines(14, 14)
            .unwrap()
            .dataflow(vec![entry("result", &["left", "right"])])
            .code_snippet("assign result = left ^ right;")
            .build()
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
    let block_set = BlockSet::new(vec![Block::builder()
        .id(BlockId(2))
        .block_type(BlockType::Always)
        .circuit_type(CircuitType::Sequential)
        .module_scope("demo")
        .source_file("design.sv")
        .lines(53, 55)
        .unwrap()
        .dataflow(vec![DataflowEntry {
            output: vec![SignalNode::named("result")],
            inputs: HashSet::from([SignalNode::named("rst_n"), SignalNode::literal("8'h0")]),
        }])
        .code_snippet("always_ff @(posedge clk or negedge rst_n) if (!rst_n) result <= 8'h0;")
        .build()
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

#[test]
fn blues_resolves_signal_with_omitted_intermediate_instance() {
    let canonical_signal = "TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o";
    let query_signal = "TOP.ibex_simple_system.u_ibex_top.u_ibex_core.if_stage_i.pc_id_o";

    let block_set = BlockSet::new(vec![Block::builder()
        .id(BlockId(77))
        .block_type(BlockType::Assign)
        .circuit_type(CircuitType::Combinational)
        .module_scope("ibex_if_stage")
        .source_file("ibex_if_stage.sv")
        .lines(501, 501)
        .unwrap()
        .dataflow(vec![DataflowEntry {
            output: vec![SignalNode::named(canonical_signal)],
            inputs: HashSet::from([SignalNode::named(
                "TOP.ibex_simple_system.u_top.u_ibex_top.u_ibex_core.if_stage_i.pc_if_o",
            )]),
        }])
        .code_snippet("assign pc_id_o = pc_if_o;")
        .build()
        .unwrap()])
    .unwrap();

    let path = BluesSlicer::new(block_set, Arc::new(FixtureCoverage::covered([])))
        .slice(&SliceRequest {
            signal: SignalNode::named(query_signal),
            time: Timestamp(3),
            min_time: Timestamp(0),
        })
        .unwrap();

    assert!(path.nodes.iter().any(|node| matches!(
        node,
        TimedSliceNode::Block {
            block_id: BlockId(77),
            time: Some(Timestamp(3)),
        }
    )));
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
