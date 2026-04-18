use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use anyhow::Result;

use crate::block::{Block, BlockSet, BlockType, CircuitType, DataflowEntry};
use crate::coverage::CoverageTracker;
use crate::slicer::{BlockEdgeJson, BlockJson, InstructionExecutionPath, SliceRequest, Slicer};
use crate::types::{BlockId, SignalNode, TimedSliceNode, Timestamp};

type TimedEdgeKey = (TimedSliceNode, TimedSliceNode, Option<SignalNode>);

struct SliceAccum<'a> {
    nodes: &'a mut HashSet<TimedSliceNode>,
    edges: &'a mut HashSet<TimedEdgeKey>,
    block_ids: &'a mut HashSet<BlockId>,
}

struct UpstreamEdgeParams<'a> {
    signal: &'a SignalNode,
    source_time: Timestamp,
    sink_block_id: BlockId,
    sink_time: Timestamp,
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
        let root_signal = self.block_set.resolve_signal_with_driver(&request.signal)?;

        let mut work = VecDeque::from([(root_signal.clone(), request.time)]);
        let mut queued = HashSet::from([(root_signal, request.time.0)]);
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

                if !is_elaborated(self.coverage.as_ref(), driver) {
                    continue;
                }

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
                        for input in
                            inputs_for_output_at(driver, &signal, self.coverage.as_ref(), time)?
                        {
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
                                self.coverage.as_ref(),
                                &mut SliceAccum {
                                    nodes: &mut nodes,
                                    edges: &mut edge_keys,
                                    block_ids: &mut block_ids,
                                },
                                UpstreamEdgeParams {
                                    signal: &input,
                                    source_time: time,
                                    sink_block_id: driver.id(),
                                    sink_time: time,
                                },
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

                        for input in inputs_for_output_at(
                            driver,
                            &signal,
                            self.coverage.as_ref(),
                            previous_time,
                        )? {
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
                                self.coverage.as_ref(),
                                &mut SliceAccum {
                                    nodes: &mut nodes,
                                    edges: &mut edge_keys,
                                    block_ids: &mut block_ids,
                                },
                                UpstreamEdgeParams {
                                    signal: &input,
                                    source_time: previous_time,
                                    sink_block_id: driver.id(),
                                    sink_time: time,
                                },
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
            target: request.signal.name.clone(),
            start_time: Some(request.time),
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
                    ast_line_start: block.ast_line_start(),
                    ast_line_end: block.ast_line_end(),
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
    coverage: &dyn CoverageTracker,
    accum: &mut SliceAccum<'_>,
    params: UpstreamEdgeParams<'_>,
) {
    for upstream_id in block_set.drivers_for(params.signal) {
        if *upstream_id == params.sink_block_id {
            continue;
        }
        let Some(upstream) = blocks_by_id.get(upstream_id) else {
            continue;
        };
        if !is_elaborated(coverage, upstream) {
            continue;
        }

        let upstream_node = TimedSliceNode::Block {
            block_id: *upstream_id,
            time: Some(params.source_time),
        };
        accum.nodes.insert(upstream_node.clone());
        accum.block_ids.insert(*upstream_id);
        accum.edges.insert((
            upstream_node,
            TimedSliceNode::Block {
                block_id: params.sink_block_id,
                time: Some(params.sink_time),
            },
            Some(params.signal.clone()),
        ));
    }
}

fn is_elaborated(coverage: &dyn CoverageTracker, block: &Block) -> bool {
    if matches!(
        block.block_type(),
        BlockType::ModInput | BlockType::ModOutput
    ) {
        return coverage.is_scope_elaborated(block.module_scope());
    }
    coverage.is_block_elaborated(block.source_file(), block.line_start(), block.line_end())
}

fn inputs_for_output_at(
    block: &Block,
    output: &SignalNode,
    coverage: &dyn CoverageTracker,
    time: Timestamp,
) -> Result<Vec<SignalNode>> {
    let entries = block
        .dataflow()
        .iter()
        .filter(|entry| entry.output.contains(output))
        .collect::<Vec<_>>();
    let covered_entries = covered_entries(block, coverage, time, &entries)?;
    let active_entries = if covered_entries.is_empty() {
        entries
    } else {
        covered_entries
    };

    Ok(active_entries
        .into_iter()
        .flat_map(|entry| entry.inputs.iter().cloned())
        .collect())
}

fn covered_entries<'a>(
    block: &Block,
    coverage: &dyn CoverageTracker,
    time: Timestamp,
    entries: &[&'a DataflowEntry],
) -> Result<Vec<&'a DataflowEntry>> {
    let mut covered = Vec::new();
    for entry in entries {
        for line in entry_output_lines(entry) {
            if coverage.is_scoped_line_covered_at(
                block.module_scope(),
                block.source_file(),
                line,
                time,
            )? {
                covered.push(*entry);
                break;
            }
        }
    }
    Ok(covered)
}

fn entry_output_lines(entry: &DataflowEntry) -> Vec<usize> {
    entry
        .output
        .iter()
        .filter_map(|signal| {
            let line = signal.locate.line;
            (line != 0).then_some(line)
        })
        .collect()
}
