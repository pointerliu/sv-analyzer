pub mod blues;
pub mod static_slice;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::types::{BlockNode, SignalId, Timestamp};

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

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SliceGraph<TNode> {
    pub nodes: Vec<TNode>,
    pub edges: Vec<SliceEdge<TNode>>,
    pub blocks: Vec<SliceBlock>,
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
