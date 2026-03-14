use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use anyhow::Result;

use crate::block::{Block, BlockSet, CircuitType};
use crate::coverage::CoverageTracker;
use crate::slicer::{BlockEdgeJson, BlockJson, InstructionExecutionPath, SliceRequest, Slicer};
use crate::types::{BlockId, BlockNode, SignalNode, Timestamp};

type TimedBlockKey = (BlockId, i64);
type TimedEdgeKey = (TimedBlockKey, TimedBlockKey, Option<SignalNode>);

struct SliceAccum<'a> {
    node_keys: &'a mut HashSet<TimedBlockKey>,
    edge_keys: &'a mut HashSet<TimedEdgeKey>,
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
        let mut work = VecDeque::from([(request.signal.clone(), request.time)]);
        let mut queued = HashSet::from([(request.signal.clone(), request.time.0)]);
        let mut visited_signals = HashSet::new();
        let mut visited_driver_outputs = HashSet::new();
        let mut node_keys = HashSet::new();
        let mut edge_keys: HashSet<TimedEdgeKey> = HashSet::new();
        let mut block_ids = HashSet::new();

        while let Some((signal, time)) = work.pop_front() {
            if time.0 < request.min_time.0 || !visited_signals.insert((signal.clone(), time.0)) {
                continue;
            }

            for driver_id in self.block_set.drivers_for(&signal) {
                let Some(driver) = self.blocks_by_id.get(driver_id) else {
                    continue;
                };

                let driver_key = (driver.id(), time.0);
                if !visited_driver_outputs.insert((driver.id(), time.0, signal.clone())) {
                    continue;
                }

                node_keys.insert(driver_key);
                block_ids.insert(driver.id());

                match driver.circuit_type() {
                    CircuitType::Combinational => {
                        for input in inputs_for_output(driver, &signal) {
                            add_upstream_edges(
                                &self.block_set,
                                &self.blocks_by_id,
                                &mut SliceAccum {
                                    node_keys: &mut node_keys,
                                    edge_keys: &mut edge_keys,
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
                        let previous_time = Timestamp(time.0 - 1);
                        if previous_time.0 < request.min_time.0 {
                            continue;
                        }

                        if !self.coverage.is_line_covered_at(
                            driver.source_file(),
                            driver.line_start(),
                            previous_time,
                        )? {
                            let previous_node = (driver.id(), previous_time.0);
                            node_keys.insert(previous_node);
                            block_ids.insert(driver.id());
                            edge_keys.insert((previous_node, driver_key, Some(signal.clone())));

                            if queued.insert((signal.clone(), previous_time.0)) {
                                work.push_back((signal.clone(), previous_time));
                            }
                            continue;
                        }

                        for input in inputs_for_output(driver, &signal) {
                            add_upstream_edges(
                                &self.block_set,
                                &self.blocks_by_id,
                                &mut SliceAccum {
                                    node_keys: &mut node_keys,
                                    edge_keys: &mut edge_keys,
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

        let mut node_keys = node_keys.into_iter().collect::<Vec<_>>();
        node_keys.sort_by_key(|(block_id, time)| (time.to_owned(), block_id.0));

        let mut edge_keys = edge_keys.into_iter().collect::<Vec<_>>();
        edge_keys.sort_by(|left, right| {
            (
                left.0 .1,
                left.0 .0 .0,
                left.1 .1,
                left.1 .0 .0,
                left.2.as_ref().map(|signal| signal.as_str()),
            )
                .cmp(&(
                    right.0 .1,
                    right.0 .0 .0,
                    right.1 .1,
                    right.1 .0 .0,
                    right.2.as_ref().map(|signal| signal.as_str()),
                ))
        });

        let mut block_ids = block_ids.into_iter().collect::<Vec<_>>();
        block_ids.sort_by_key(|block_id| block_id.0);

        Ok(InstructionExecutionPath {
            nodes: node_keys
                .iter()
                .map(|(block_id, time)| BlockNode {
                    block_id: *block_id,
                    time: Timestamp(*time),
                })
                .collect(),
            edges: edge_keys
                .into_iter()
                .map(|(from, to, signal)| BlockEdgeJson {
                    from: BlockNode {
                        block_id: from.0,
                        time: Timestamp(from.1),
                    },
                    to: BlockNode {
                        block_id: to.0,
                        time: Timestamp(to.1),
                    },
                    signal,
                })
                .collect(),
            blocks: block_ids
                .into_iter()
                .filter_map(|block_id| self.blocks_by_id.get(&block_id))
                .map(|block| BlockJson {
                    id: block.id(),
                    scope: block.module_scope().to_string(),
                    block_type: format!("{:?}", block.block_type()),
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

        let upstream_key = (*upstream_id, source_time.0);
        accum.node_keys.insert(upstream_key);
        accum.block_ids.insert(*upstream_id);
        accum.edge_keys.insert((
            upstream_key,
            (sink_block_id, sink_time.0),
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
