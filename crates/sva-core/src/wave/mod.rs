use anyhow::Result;

use crate::types::{SignalNode, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalValue {
    pub raw_bits: String,
    pub pretty_hex: Option<String>,
}

pub trait WaveformReader {
    fn signal_value_at(&self, signal: &SignalNode, time: Timestamp) -> Result<Option<SignalValue>>;
}

pub mod wellen;
pub use wellen::{apply_scope_remap_to_graph, WellenReader};
