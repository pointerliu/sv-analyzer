use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timestamp(pub i64);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SignalId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockNode {
    pub block_id: BlockId,
    pub time: Timestamp,
}

pub trait StableSliceNode {
    fn block_id(&self) -> BlockId;
    fn time(&self) -> Option<Timestamp>;
}

impl StableSliceNode for BlockNode {
    fn block_id(&self) -> BlockId {
        self.block_id
    }

    fn time(&self) -> Option<Timestamp> {
        Some(self.time)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableSliceNodeJson {
    pub id: usize,
    pub block_id: BlockId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Timestamp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableSliceEdgeJson {
    pub from: usize,
    pub to: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<SignalId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StableSliceGraphJson {
    pub nodes: Vec<StableSliceNodeJson>,
    pub edges: Vec<StableSliceEdgeJson>,
    pub blocks: Vec<BlockJson>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockEdgeJson {
    pub from: BlockNode,
    pub to: BlockNode,
    pub signal: Option<SignalId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockJson {
    pub id: BlockId,
    pub scope: String,
    pub block_type: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceGraphJson {
    pub nodes: Vec<BlockNode>,
    pub edges: Vec<BlockEdgeJson>,
    pub blocks: Vec<BlockJson>,
}
