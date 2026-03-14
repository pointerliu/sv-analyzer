pub mod blues;
pub mod static_slice;

use anyhow::{anyhow, Result};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::HashMap;

use crate::types::{
    BlockId, BlockJson as StableBlockJson, BlockNode, SignalId, StableSliceEdgeJson,
    StableSliceGraphJson, StableSliceNode, StableSliceNodeJson, Timestamp,
};

pub use blues::BluesSlicer;
pub use static_slice::StaticSlicer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRequest {
    pub signal: SignalId,
    pub time: Timestamp,
    pub min_time: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StaticBlockNode {
    pub block_id: crate::types::BlockId,
}

impl StableSliceNode for StaticBlockNode {
    fn block_id(&self) -> BlockId {
        self.block_id
    }

    fn time(&self) -> Option<Timestamp> {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceEdge<TNode> {
    pub from: TNode,
    pub to: TNode,
    pub signal: Option<SignalId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceBlock {
    pub id: crate::types::BlockId,
    pub scope: String,
    pub block_type: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SliceGraph<TNode> {
    pub nodes: Vec<TNode>,
    pub edges: Vec<SliceEdge<TNode>>,
    pub blocks: Vec<SliceBlock>,
}

impl<TNode> SliceGraph<TNode>
where
    TNode: StableSliceNode + Clone + PartialEq + Eq,
{
    pub fn stable_json_graph(&self) -> Result<StableSliceGraphJson> {
        let mut node_index_by_key = HashMap::with_capacity(self.nodes.len());
        for (index, node) in self.nodes.iter().enumerate() {
            let key = (node.block_id(), node.time());
            if node_index_by_key.insert(key, index).is_some() {
                let duplicate = match key.1 {
                    Some(time) => {
                        anyhow!(
                            "duplicate slice node for block_id={} time={}",
                            key.0 .0,
                            time.0
                        )
                    }
                    None => anyhow!("duplicate slice node for block_id={}", key.0 .0),
                };
                return Err(duplicate);
            }
        }

        Ok(StableSliceGraphJson {
            nodes: self
                .nodes
                .iter()
                .enumerate()
                .map(|(index, node)| StableSliceNodeJson {
                    id: index,
                    block_id: node.block_id(),
                    time: node.time(),
                })
                .collect(),
            edges: self
                .edges
                .iter()
                .map(|edge| StableSliceEdgeJson {
                    from: node_index_by_key[&(edge.from.block_id(), edge.from.time())],
                    to: node_index_by_key[&(edge.to.block_id(), edge.to.time())],
                    signal: edge.signal.clone(),
                })
                .collect(),
            blocks: self
                .blocks
                .iter()
                .map(|block| StableBlockJson {
                    id: block.id,
                    scope: block.scope.clone(),
                    block_type: block.block_type.clone(),
                })
                .collect(),
        })
    }
}

impl<TNode> Serialize for SliceGraph<TNode>
where
    TNode: StableSliceNode + Clone + PartialEq + Eq,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let stable = self
            .stable_json_graph()
            .map_err(serde::ser::Error::custom)?;
        let mut state = serializer.serialize_struct("SliceGraph", 3)?;
        state.serialize_field("nodes", &stable.nodes)?;
        state.serialize_field("edges", &stable.edges)?;
        state.serialize_field("blocks", &stable.blocks)?;
        state.end()
    }
}

pub type StaticBlockEdge = SliceEdge<StaticBlockNode>;

pub type StaticBlockJson = SliceBlock;

pub type StaticSliceGraph = SliceGraph<StaticBlockNode>;

pub type BlockEdgeJson = SliceEdge<BlockNode>;

pub type BlockJson = SliceBlock;

pub type InstructionExecutionPath = SliceGraph<BlockNode>;

pub trait Slicer {
    fn slice(&self, request: &SliceRequest) -> Result<InstructionExecutionPath>;
}
