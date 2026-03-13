pub mod wellen;

use anyhow::Result;

use crate::types::{SignalId, Timestamp};

pub use self::wellen::WellenReader;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalValue {
    pub raw_bits: String,
    pub pretty_hex: Option<String>,
}

pub trait WaveformReader {
    fn signal_value_at(&self, signal: &SignalId, time: Timestamp) -> Result<Option<SignalValue>>;
}
