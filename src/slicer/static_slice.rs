use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;

use crate::block::{Block, BlockSet};
use crate::slicer::{
    SliceRequest, StaticBlockEdge, StaticBlockJson, StaticBlockNode, StaticSliceGraph,
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
        let mut visited = HashSet::new();
        let mut queued = HashSet::new();
        let mut work = VecDeque::from([request.signal.clone()]);
        let mut node_ids = HashSet::new();
        let mut edge_keys = HashSet::new();

        queued.insert(request.signal.clone());

        while let Some(signal) = work.pop_front() {
            if !visited.insert(signal.clone()) {
                continue;
            }

            for driver_id in self.block_set.drivers_for(&signal) {
                let Some(driver) = self.blocks_by_id.get(driver_id) else {
                    continue;
                };

                node_ids.insert(*driver_id);

                for input in inputs_for_output(driver, &signal) {
                    for upstream_id in self.block_set.drivers_for(&input) {
                        if *upstream_id != *driver_id && self.blocks_by_id.contains_key(upstream_id)
                        {
                            node_ids.insert(*upstream_id);
                            edge_keys.insert((upstream_id.0, driver_id.0, input.clone()));
                        }
                    }

                    if !visited.contains(&input) && queued.insert(input.clone()) {
                        work.push_back(input.clone());
                    }
                }
            }
        }

        let mut node_ids = node_ids.into_iter().collect::<Vec<_>>();
        node_ids.sort_by_key(|block_id| block_id.0);

        let mut edge_keys = edge_keys.into_iter().collect::<Vec<_>>();
        edge_keys.sort_by(|left, right| {
            (left.0, left.1, left.2.as_str()).cmp(&(right.0, right.1, right.2.as_str()))
        });

        Ok(StaticSliceGraph {
            nodes: node_ids
                .iter()
                .map(|block_id| StaticBlockNode {
                    block_id: *block_id,
                })
                .collect(),
            edges: edge_keys
                .into_iter()
                .map(|(from, to, signal)| StaticBlockEdge {
                    from: StaticBlockNode {
                        block_id: BlockId(from),
                    },
                    to: StaticBlockNode {
                        block_id: BlockId(to),
                    },
                    signal: Some(signal),
                })
                .collect(),
            blocks: node_ids
                .into_iter()
                .filter_map(|block_id| self.blocks_by_id.get(&block_id))
                .map(|block| StaticBlockJson {
                    id: block.id(),
                    scope: block.module_scope().to_string(),
                    block_type: format!("{:?}", block.block_type()),
                })
                .collect(),
        })
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
