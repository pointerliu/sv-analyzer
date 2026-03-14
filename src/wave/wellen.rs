use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use wellen::simple::{self, Waveform};
use wellen::{Hierarchy, SignalRef, SignalValue as WellenSignalValue};

use crate::types::{SignalNode, Timestamp};
use crate::wave::{SignalValue, WaveformReader};

#[derive(Debug)]
pub struct WellenReader {
    waveform: Waveform,
    signal_lookup: HashMap<String, Option<SignalRef>>,
    time_table: Vec<u64>,
}

impl WellenReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut waveform = simple::read(path.as_ref())
            .with_context(|| format!("failed to open waveform {}", path.as_ref().display()))?;

        let signal_lookup = build_signal_lookup(waveform.hierarchy());
        let signal_refs: Vec<_> = signal_lookup.values().filter_map(|value| *value).collect();
        waveform.load_signals(&signal_refs);
        let time_table = waveform.time_table().to_vec();

        Ok(Self {
            waveform,
            signal_lookup,
            time_table,
        })
    }
}

impl WaveformReader for WellenReader {
    fn signal_value_at(&self, signal: &SignalNode, time: Timestamp) -> Result<Option<SignalValue>> {
        if time.0 < 0 {
            return Ok(None);
        }

        let signal_ref = match self.signal_lookup.get(signal.as_str()) {
            Some(Some(signal_ref)) => *signal_ref,
            Some(None) => return Ok(None),
            None => return Ok(None),
        };

        let waveform_signal = self
            .waveform
            .get_signal(signal_ref)
            .ok_or_else(|| anyhow!("signal data not loaded for {}", signal.as_str()))?;

        let time_idx = match self.time_table.binary_search(&(time.0 as u64)) {
            Ok(index) => index,
            Err(0) => return Ok(None),
            Err(index) => index - 1,
        };

        let offset = match waveform_signal.get_offset(time_idx as u32) {
            Some(offset) => offset,
            None => return Ok(None),
        };

        let value = waveform_signal.get_value_at(&offset, 0);
        let raw_bits = match safe_raw_bits(value) {
            Some(bits) => bits,
            None => return Ok(None),
        };
        let pretty_hex = pretty_hex(&raw_bits);

        Ok(Some(SignalValue {
            raw_bits,
            pretty_hex,
        }))
    }
}

fn safe_raw_bits(value: WellenSignalValue<'_>) -> Option<String> {
    match value {
        WellenSignalValue::Binary(..)
        | WellenSignalValue::FourValue(..)
        | WellenSignalValue::NineValue(..) => value.to_bit_string(),
        WellenSignalValue::Event | WellenSignalValue::String(..) | WellenSignalValue::Real(..) => {
            None
        }
    }
}

fn build_signal_lookup(hierarchy: &Hierarchy) -> HashMap<String, Option<SignalRef>> {
    let mut lookup = HashMap::new();

    for var in hierarchy.iter_vars() {
        let full_name = var.full_name(hierarchy);
        let signal_ref = var.signal_ref();

        lookup.insert(full_name.clone(), Some(signal_ref));

        if let Some((_, suffix)) = full_name.split_once('.') {
            match lookup.get(suffix) {
                None => {
                    lookup.insert(suffix.to_string(), Some(signal_ref));
                }
                Some(Some(existing)) if *existing == signal_ref => {}
                Some(_) => {
                    lookup.insert(suffix.to_string(), None);
                }
            }
        }
    }

    lookup
}

fn pretty_hex(raw_bits: &str) -> Option<String> {
    if raw_bits.is_empty() || raw_bits.chars().any(|bit| !matches!(bit, '0' | '1')) {
        return None;
    }

    let padded_len = raw_bits.len().next_multiple_of(4);
    let mut padded = String::with_capacity(padded_len);
    padded.extend(std::iter::repeat_n('0', padded_len - raw_bits.len()));
    padded.push_str(raw_bits);

    let mut hex = String::with_capacity(padded_len / 4);
    for nibble in padded.as_bytes().chunks(4) {
        let mut value = 0u8;
        for bit in nibble {
            value = (value << 1) | (bit - b'0');
        }
        hex.push(char::from_digit(value.into(), 16).unwrap());
    }

    Some(format!("0x{hex}"))
}
