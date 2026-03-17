use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;

use crate::block::{Block, BlockSet};
use crate::slicer::{
    InstructionExecutionPath, SliceRequest, Slicer, StaticBlockEdge, StaticBlockJson,
    StaticBlockNode, StaticSliceGraph,
};
use crate::types::{BlockId, SignalNode};

#[derive(Debug, Clone)]
pub struct StaticSlicer {
    block_set: BlockSet,
    blocks_by_id: HashMap<BlockId, Block>,
}

impl StaticSlicer {
    pub fn new(block_set: BlockSet) -> Self {
        let blocks_by_id = block_set
            .blocks()
            .iter()
            .cloned()
            .map(|block| (block.id(), block))
            .collect();

        Self {
            block_set,
            blocks_by_id,
        }
    }

    pub fn slice(&self, request: &SliceRequest) -> Result<StaticSliceGraph> {
        // Verify the signal exists in the block set
        if self.block_set.drivers_for(&request.signal).is_empty() {
            let signal_name = request.signal.as_str();
            anyhow::bail!(
                "signal '{}' not found in block set. Provide hierarchical name (e.g., 'TOP.module.signal')",
                signal_name
            );
        }

        let mut visited = HashSet::new();
        let mut queued = HashSet::new();
        let mut work = VecDeque::from([request.signal.clone()]);
        let mut nodes = HashSet::new();
        let mut block_ids = HashSet::new();
        let mut edge_keys = HashSet::new();

        queued.insert(request.signal.clone());

        while let Some(signal) = work.pop_front() {
            if signal.is_literal() || !visited.insert(signal.clone()) {
                continue;
            }

            for driver_id in self.block_set.drivers_for(&signal) {
                let Some(driver) = self.blocks_by_id.get(driver_id) else {
                    continue;
                };

                let driver_node = StaticBlockNode::Block {
                    block_id: *driver_id,
                    time: None,
                };
                nodes.insert(driver_node.clone());
                block_ids.insert(*driver_id);

                for input in inputs_for_output(driver, &signal) {
                    if input.is_literal() {
                        let literal_node = StaticBlockNode::Literal {
                            signal: input.clone(),
                            time: None,
                        };
                        nodes.insert(literal_node.clone());
                        edge_keys.insert((literal_node, driver_node.clone(), None));
                        continue;
                    }

                    for upstream_id in self.block_set.drivers_for(&input) {
                        if *upstream_id != *driver_id && self.blocks_by_id.contains_key(upstream_id)
                        {
                            let upstream_node = StaticBlockNode::Block {
                                block_id: *upstream_id,
                                time: None,
                            };
                            nodes.insert(upstream_node.clone());
                            block_ids.insert(*upstream_id);
                            edge_keys.insert((
                                upstream_node,
                                driver_node.clone(),
                                Some(input.clone()),
                            ));
                        }
                    }

                    if queued.insert(input.clone()) {
                        work.push_back(input);
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

        Ok(StaticSliceGraph {
            nodes,
            edges: edge_keys
                .into_iter()
                .map(|(from, to, signal)| StaticBlockEdge { from, to, signal })
                .collect(),
            blocks: block_ids
                .into_iter()
                .filter_map(|block_id| self.blocks_by_id.get(&block_id))
                .map(|block| StaticBlockJson {
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

impl Slicer for StaticSlicer {
    fn slice(&self, request: &SliceRequest) -> Result<InstructionExecutionPath> {
        StaticSlicer::slice(self, request)
    }
}

fn inputs_for_output(block: &Block, output: &SignalNode) -> Vec<SignalNode> {
    block
        .dataflow()
        .iter()
        .filter(move |entry| entry.output.contains(output))
        .flat_map(|entry| entry.inputs.iter().cloned())
        .collect()
}
