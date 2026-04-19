use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use wellen::simple::{self, Waveform};
use wellen::{Hierarchy, SignalRef, SignalValue as WellenSignalValue};

use crate::types::{SignalNode, StableSliceGraphJson, StableSliceNodeJson, Timestamp};
use crate::wave::{SignalValue, WaveformReader};

#[derive(Debug)]
pub struct WellenReader {
    waveform: Waveform,
    signal_lookup: HashMap<String, Option<SignalRef>>,
    time_table: Vec<u64>,
    scope_remap: HashMap<String, String>,
}

impl WellenReader {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut waveform = simple::read(path.as_ref())
            .with_context(|| format!("failed to open waveform {}", path.as_ref().display()))?;

        let signal_lookup = build_signal_lookup(waveform.hierarchy());
        let signal_refs: Vec<_> = signal_lookup.values().filter_map(|value| *value).collect();
        waveform.load_signals(&signal_refs);
        let time_table = waveform.time_table().to_vec();
        let scope_remap = build_scope_remap(waveform.hierarchy());

        Ok(Self {
            waveform,
            signal_lookup,
            time_table,
            scope_remap,
        })
    }

    pub fn signal_names(&self) -> impl Iterator<Item = &str> {
        self.signal_lookup.keys().map(String::as_str)
    }

    /// Map a logical hierarchical scope (with generate-block wrappers stripped,
    /// as produced by sv-parser-driven block elaboration) to the actual FST scope
    /// that includes Verilator's `gen_*` / `unnamedblk*` wrapper segments.
    /// Returns `None` when the logical scope has no FST counterpart.
    pub fn remap_scope(&self, logical: &str) -> Option<&str> {
        if logical.is_empty() {
            return None;
        }
        if self.scope_remap.is_empty() {
            return None;
        }
        if let Some(fst) = self.scope_remap.get(logical) {
            return Some(fst.as_str());
        }
        None
    }

    /// Map a logical fully-qualified signal name to its FST counterpart by
    /// remapping the enclosing scope segments. Returns `None` when the FST
    /// has no matching scope; the caller should fall back to the logical name.
    pub fn remap_signal(&self, logical_signal: &str) -> Option<String> {
        let (scope, var) = logical_signal.rsplit_once('.')?;
        let fst_scope = self.remap_scope(scope)?;
        if fst_scope == scope {
            None
        } else {
            Some(format!("{fst_scope}.{var}"))
        }
    }
}

/// Rewrite scopes and hierarchical signal names in a stable slice graph so
/// they refer to FST-truthful paths (with Verilator generate-block wrappers
/// reinserted). Entries with no FST counterpart are left untouched.
pub fn apply_scope_remap_to_graph(reader: &WellenReader, graph: &mut StableSliceGraphJson) {
    if let Some(remapped) = reader.remap_signal(&graph.target) {
        graph.target = remapped;
    }
    for block in graph.blocks.iter_mut() {
        if let Some(fst_scope) = reader.remap_scope(&block.scope) {
            if fst_scope != block.scope {
                block.scope = fst_scope.to_string();
            }
        }
    }
    for edge in graph.edges.iter_mut() {
        if let Some(signal) = edge.signal.as_mut() {
            if let Some(remapped) = reader.remap_signal(&signal.name) {
                signal.name = remapped;
            }
        }
    }
    for node in graph.nodes.iter_mut() {
        if let StableSliceNodeJson::Literal { signal, .. } = node {
            if let Some(remapped) = reader.remap_signal(&signal.name) {
                signal.name = remapped;
            }
        }
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

fn is_wrapper_segment(segment: &str) -> bool {
    segment.starts_with("gen_") || segment.starts_with("unnamedblk")
}

fn strip_wrapper_segments(fst_scope: &str) -> String {
    fst_scope
        .split('.')
        .filter(|seg| !is_wrapper_segment(seg))
        .collect::<Vec<_>>()
        .join(".")
}

/// Build a `logical_scope -> fst_scope` map by walking every FST scope and
/// computing its logical key as the dotted path with `gen_*` / `unnamedblk*`
/// wrapper segments stripped. Two FST scopes can collide on the same logical
/// key (e.g. a parent scope `foo` and its generate child `foo.gen_bar` both
/// strip to `foo`); the shorter FST path wins so the bare logical scope
/// resolves to itself rather than to a nested generate block.
fn build_scope_remap(hierarchy: &Hierarchy) -> HashMap<String, String> {
    let mut map: HashMap<String, String> = HashMap::new();
    for scope in hierarchy.iter_scopes() {
        let full = scope.full_name(hierarchy);
        let logical = strip_wrapper_segments(&full);
        if logical.is_empty() {
            continue;
        }
        match map.get(&logical) {
            Some(existing) if existing.len() <= full.len() => {}
            _ => {
                map.insert(logical, full);
            }
        }
    }
    map
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
