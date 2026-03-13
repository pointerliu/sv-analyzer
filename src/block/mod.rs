pub mod dataflow;

use std::collections::HashMap;

use anyhow::Result;

use crate::ast::ParsedFile;
use crate::types::{BlockId, SignalId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BlockSet {
    pub blocks: Vec<Block>,
    pub signal_to_drivers: HashMap<SignalId, Vec<BlockId>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub id: BlockId,
}

pub trait Blockizer {
    fn blockize(&self, files: &[ParsedFile]) -> Result<BlockSet>;
}
