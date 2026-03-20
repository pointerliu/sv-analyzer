use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use anyhow::Result;

use crate::block::{Block, BlockSet, CircuitType};
use crate::coverage::CoverageTracker;
use crate::slicer::{BlockEdgeJson, BlockJson, InstructionExecutionPath, SliceRequest, Slicer};
use crate::types::{BlockId, SignalNode, TimedSliceNode, Timestamp};

type TimedEdgeKey = (TimedSliceNode, TimedSliceNode, Option<SignalNode>);

struct SliceAccum<'a> {
    nodes: &'a mut HashSet<TimedSliceNode>,
    edges: &'a mut HashSet<TimedEdgeKey>,
    block_ids: &'a mut HashSet<BlockId>,
}

pub struct BluesSlicer {
    block_set: BlockSet,
    blocks_by_id: HashMap<BlockId, Block>,
    coverage: Arc<dyn CoverageTracker + Send + Sync>,
}

impl BluesSlicer {
    pub fn new(block_set: BlockSet, coverage: Arc<dyn CoverageTracker + Send + Sync>) -> Self {
        let blocks_by_id = block_set
            .blocks()
            .iter()
            .cloned()
            .map(|block| (block.id(), block))
            .collect();

        Self {
            block_set,
            blocks_by_id,
            coverage,
        }
    }

    pub fn slice(&self, request: &SliceRequest) -> Result<InstructionExecutionPath> {
        self.block_set.validate_signal_has_driver(&request.signal)?;

        let mut work = VecDeque::from([(request.signal.clone(), request.time)]);
        let mut queued = HashSet::from([(request.signal.clone(), request.time.0)]);
        let mut visited_signals = HashSet::new();
        let mut visited_driver_outputs = HashSet::new();
        let mut nodes = HashSet::new();
        let mut edge_keys: HashSet<TimedEdgeKey> = HashSet::new();
        let mut block_ids = HashSet::new();

        while let Some((signal, time)) = work.pop_front() {
            if signal.is_literal()
                || time.0 < request.min_time.0
                || !visited_signals.insert((signal.clone(), time.0))
            {
                continue;
            }

            for driver_id in self.block_set.drivers_for(&signal) {
                let Some(driver) = self.blocks_by_id.get(driver_id) else {
                    continue;
                };

                let driver_node = TimedSliceNode::Block {
                    block_id: driver.id(),
                    time: Some(time),
                };
                if !visited_driver_outputs.insert((driver.id(), time.0, signal.clone())) {
                    continue;
                }

                nodes.insert(driver_node.clone());
                block_ids.insert(driver.id());

                match driver.circuit_type() {
                    CircuitType::Combinational => {
                        for input in inputs_for_output(driver, &signal) {
                            if input.is_literal() {
                                nodes.insert(TimedSliceNode::Literal {
                                    signal: input.clone(),
                                    time: Some(time),
                                });
                                edge_keys.insert((
                                    TimedSliceNode::Literal {
                                        signal: input,
                                        time: Some(time),
                                    },
                                    driver_node.clone(),
                                    None,
                                ));
                                continue;
                            }

                            add_upstream_edges(
                                &self.block_set,
                                &self.blocks_by_id,
                                &mut SliceAccum {
                                    nodes: &mut nodes,
                                    edges: &mut edge_keys,
                                    block_ids: &mut block_ids,
                                },
                                &input,
                                time,
                                driver.id(),
                                time,
                            );

                            if queued.insert((input.clone(), time.0)) {
                                work.push_back((input, time));
                            }
                        }
                    }
                    CircuitType::Sequential => {
                        let clk_period = self.coverage.clock_period().unwrap_or(1);
                        let previous_time = Timestamp(time.0 - clk_period);
                        if previous_time.0 < request.min_time.0 {
                            continue;
                        }

                        if !self.coverage.is_line_covered_at(
                            driver.source_file(),
                            driver.line_start(),
                            previous_time,
                        )? {
                            let previous_node = TimedSliceNode::Block {
                                block_id: driver.id(),
                                time: Some(previous_time),
                            };
                            nodes.insert(previous_node.clone());
                            block_ids.insert(driver.id());
                            edge_keys.insert((
                                previous_node,
                                driver_node.clone(),
                                Some(signal.clone()),
                            ));

                            if queued.insert((signal.clone(), previous_time.0)) {
                                work.push_back((signal.clone(), previous_time));
                            }
                            continue;
                        }

                        for input in inputs_for_output(driver, &signal) {
                            if input.is_literal() {
                                let literal_node = TimedSliceNode::Literal {
                                    signal: input,
                                    time: Some(previous_time),
                                };
                                nodes.insert(literal_node.clone());
                                edge_keys.insert((literal_node, driver_node.clone(), None));
                                continue;
                            }

                            add_upstream_edges(
                                &self.block_set,
                                &self.blocks_by_id,
                                &mut SliceAccum {
                                    nodes: &mut nodes,
                                    edges: &mut edge_keys,
                                    block_ids: &mut block_ids,
                                },
                                &input,
                                previous_time,
                                driver.id(),
                                time,
                            );

                            if queued.insert((input.clone(), previous_time.0)) {
                                work.push_back((input, previous_time));
                            }
                        }
                    }
                }
            }
        }

        let mut nodes = nodes.into_iter().collect::<Vec<_>>();
        nodes.sort_by(|left, right| format!("{:?}", left).cmp(&format!("{:?}", right)));

        let mut edge_keys = edge_keys.into_iter().collect::<Vec<_>>();
        edge_keys.sort_by(|left, right| format!("{:?}", left).cmp(&format!("{:?}", right)));

        let mut block_ids = block_ids.into_iter().collect::<Vec<_>>();
        block_ids.sort_by_key(|block_id| block_id.0);

        Ok(InstructionExecutionPath {
            nodes,
            edges: edge_keys
                .into_iter()
                .map(|(from, to, signal)| BlockEdgeJson { from, to, signal })
                .collect(),
            blocks: block_ids
                .into_iter()
                .filter_map(|block_id| self.blocks_by_id.get(&block_id))
                .map(|block| BlockJson {
                    id: block.id(),
                    scope: block.module_scope().to_string(),
                    block_type: format!("{:?}", block.block_type()),
                    source_file: block.source_file().to_string(),
                    line_start: block.line_start(),
                    line_end: block.line_end(),
                    code_snippet: block.code_snippet().to_string(),
                })
                .collect(),
        })
    }
}

impl Slicer for BluesSlicer {
    fn slice(&self, request: &SliceRequest) -> Result<InstructionExecutionPath> {
        BluesSlicer::slice(self, request)
    }
}

fn add_upstream_edges(
    block_set: &BlockSet,
    blocks_by_id: &HashMap<BlockId, Block>,
    accum: &mut SliceAccum<'_>,
    signal: &SignalNode,
    source_time: Timestamp,
    sink_block_id: BlockId,
    sink_time: Timestamp,
) {
    for upstream_id in block_set.drivers_for(signal) {
        if *upstream_id == sink_block_id || !blocks_by_id.contains_key(upstream_id) {
            continue;
        }

        let upstream_node = TimedSliceNode::Block {
            block_id: *upstream_id,
            time: Some(source_time),
        };
        accum.nodes.insert(upstream_node.clone());
        accum.block_ids.insert(*upstream_id);
        accum.edges.insert((
            upstream_node,
            TimedSliceNode::Block {
                block_id: sink_block_id,
                time: Some(sink_time),
            },
            Some(signal.clone()),
        ));
    }
}

fn inputs_for_output(block: &Block, output: &SignalNode) -> Vec<SignalNode> {
    block
        .dataflow()
        .iter()
        .filter(|entry| entry.output.contains(output))
        .flat_map(|entry| entry.inputs.iter().cloned())
        .collect()
}
