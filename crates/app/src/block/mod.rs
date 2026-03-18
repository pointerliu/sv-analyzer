pub mod dataflow;

pub use dac26_core::block::{Block, BlockSet, BlockType, Blockizer, CircuitType, DataflowEntry};
pub use dataflow::{elaborate_block_set, DataflowBlockizer};
