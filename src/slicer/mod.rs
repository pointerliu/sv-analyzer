pub mod blues;
pub mod static_slice;

use anyhow::Result;

use crate::types::{SignalId, Timestamp, TraceGraphJson};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SliceRequest {
    pub signal: SignalId,
    pub time: Timestamp,
    pub min_time: Timestamp,
}

pub type InstructionExecutionPath = TraceGraphJson;

pub trait Slicer {
    fn slice(&self, request: &SliceRequest) -> Result<InstructionExecutionPath>;
}
