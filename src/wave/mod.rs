pub mod wellen;

use anyhow::Result;

use crate::types::{SignalId, Timestamp};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalValue {
    pub raw_bits: String,
}

pub trait WaveformReader {
    fn signal_value_at(&self, signal: &SignalId, time: Timestamp) -> Result<Option<SignalValue>>;
}
