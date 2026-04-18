pub mod blues;
pub mod static_slice;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::types::{
    BlockJson as StableBlockJson, SignalNode, StableSliceEdgeJson, StableSliceGraphJson,
    StableSliceNode, StableSliceNodeKey, Timestamp,
};

pub use blues::BluesSlicer;
pub use static_slice::StaticSlicer;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRequest {
    pub signal: SignalNode,
    pub time: Timestamp,
    pub min_time: Timestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceEdge<TNode> {
    pub from: TNode,
    pub to: TNode,
    pub signal: Option<SignalNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceBlock {
    pub id: crate::types::BlockId,
    pub scope: String,
    pub block_type: String,
    pub source_file: String,
    pub line_start: usize,
    pub line_end: usize,
    pub ast_line_start: usize,
    pub ast_line_end: usize,
    pub code_snippet: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SliceGraph<TNode> {
    pub target: String,
    pub start_time: Option<Timestamp>,
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
            let key = node.stable_key();
            if node_index_by_key.insert(key, index).is_some() {
                let duplicate = match &node.stable_key() {
                    StableSliceNodeKey::Block {
                        block_id,
                        time: Some(time),
                    } => anyhow!(
                        "duplicate slice node for block_id={} time={}",
                        block_id.0,
                        time.0
                    ),
                    StableSliceNodeKey::Block {
                        block_id,
                        time: None,
                    } => anyhow!("duplicate slice node for block_id={}", block_id.0),
                    StableSliceNodeKey::Literal {
                        signal,
                        time: Some(time),
                    } => anyhow!(
                        "duplicate literal slice node for signal={} time={}",
                        signal.name,
                        time.0
                    ),
                    StableSliceNodeKey::Literal { signal, time: None } => {
                        anyhow!("duplicate literal slice node for signal={}", signal.name)
                    }
                };
                return Err(duplicate);
            }
        }

        Ok(StableSliceGraphJson {
            target: self.target.clone(),
            start_time: self.start_time,
            nodes: self
                .nodes
                .iter()
                .enumerate()
                .map(|(index, node)| node.stable_json(index))
                .collect(),
            edges: self
                .edges
                .iter()
                .map(|edge| StableSliceEdgeJson {
                    from: node_index_by_key[&edge.from.stable_key()],
                    to: node_index_by_key[&edge.to.stable_key()],
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
                    source_file: block.source_file.clone(),
                    line_start: block.line_start,
                    line_end: block.line_end,
                    ast_line_start: block.ast_line_start,
                    ast_line_end: block.ast_line_end,
                    code_snippet: block.code_snippet.clone(),
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
        S: serde::Serializer,
    {
        self.stable_json_graph()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

pub type StaticBlockNode = crate::types::TimedSliceNode;

pub type TimedSliceNode = crate::types::TimedSliceNode;

pub type StaticBlockEdge = SliceEdge<TimedSliceNode>;

pub type StaticBlockJson = SliceBlock;

pub type StaticSliceGraph = SliceGraph<TimedSliceNode>;

pub type BlockEdgeJson = SliceEdge<TimedSliceNode>;

pub type BlockJson = SliceBlock;

pub type InstructionExecutionPath = SliceGraph<TimedSliceNode>;

pub trait Slicer {
    fn slice(&self, request: &SliceRequest) -> Result<InstructionExecutionPath>;
}
