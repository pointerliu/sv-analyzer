use std::collections::HashMap;
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use wellen::simple::{self, Waveform};
use wellen::{Hierarchy, SignalRef};

use crate::coverage::CoverageTracker;
use crate::types::Timestamp;

const TRACE_PREFIX: &str = "vlCoverageLineTrace_";

#[derive(Debug)]
pub struct VcdCoverageTracker {
    annotation_query_times: Vec<i64>,
    annotation_sample_times: Vec<i64>,
    traces_by_line: HashMap<(String, usize), Vec<CoverageTrace>>,
    clock_period: Option<i64>,
    #[allow(dead_code)]
    clock_signal_ref: Option<SignalRef>,
    rising_edges: Vec<i64>,
}

#[derive(Debug)]
struct CoverageTrace {
    samples: Vec<CoverageSample>,
}

#[derive(Debug, Clone, Copy)]
struct CoverageSample {
    time: i64,
    count: u64,
}

#[derive(Debug)]
struct TraceDescriptor {
    file: String,
    line: usize,
    signal_ref: SignalRef,
}

impl VcdCoverageTracker {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Self::open_internal(path, None)
    }

    pub fn open_with_clock(
        path: impl AsRef<Path>,
        clock_signal: &str,
        clk_step: i64,
    ) -> Result<Self> {
        Self::open_internal(path, Some((clock_signal, clk_step)))
    }

    fn open_internal(path: impl AsRef<Path>, clock_signal: Option<(&str, i64)>) -> Result<Self> {
        let mut waveform = simple::read(path.as_ref())
            .with_context(|| format!("failed to open waveform {}", path.as_ref().display()))?;

        let descriptors = collect_trace_descriptors(&waveform);
        let mut signal_refs: Vec<_> = descriptors
            .iter()
            .map(|descriptor| descriptor.signal_ref)
            .collect();

        let (clock_signal_ref, clock_period) = match clock_signal {
            Some((clock_name, clk_step)) => {
                if clk_step <= 0 {
                    return Err(anyhow!("clk_step must be positive, got {clk_step}"));
                }
                let ref_opt = lookup_signal_ref(waveform.hierarchy(), clock_name)?;
                if let Some(signal_ref) = ref_opt {
                    signal_refs.push(signal_ref);
                }
                (ref_opt, Some(clk_step))
            }
            None => {
                let auto_detected = auto_detect_clock(&mut waveform, &mut signal_refs)?;
                (auto_detected.signal_ref, auto_detected.period)
            }
        };
        waveform.load_signals(&signal_refs);

        let raw_times = waveform
            .time_table()
            .iter()
            .map(|time| {
                i64::try_from(*time).map_err(|_| anyhow!("waveform time exceeds i64 range: {time}"))
            })
            .collect::<Result<Vec<_>>>()?;

        let (annotation_query_times, annotation_sample_times, rising_edges) =
            match (clock_signal_ref, clock_period) {
                (Some(signal_ref), Some(clk_step)) => {
                    let signal = waveform
                        .get_signal(signal_ref)
                        .ok_or_else(|| anyhow!("clock signal data not loaded"))?;
                    let rising = collect_rising_edges(signal, &raw_times)?;
                    let (query_times, sample_times) =
                        build_annotation_timeline(signal, &raw_times, clk_step)?;
                    (query_times, sample_times, rising)
                }
                (Some(_), None) => unreachable!(),
                (None, None) => (raw_times.clone(), raw_times.clone(), vec![]),
                (None, Some(_)) => unreachable!(),
            };

        let mut traces_by_line = HashMap::new();
        for descriptor in descriptors {
            let signal = waveform.get_signal(descriptor.signal_ref).ok_or_else(|| {
                anyhow!(
                    "signal data not loaded for {}:{}",
                    descriptor.file,
                    descriptor.line
                )
            })?;
            let samples = extract_samples(signal, &raw_times)?;

            traces_by_line
                .entry((descriptor.file, descriptor.line))
                .or_insert_with(Vec::new)
                .push(CoverageTrace { samples });
        }

        Ok(Self {
            annotation_query_times,
            annotation_sample_times,
            traces_by_line,
            clock_period,
            clock_signal_ref,
            rising_edges,
        })
    }

    fn resolved_annotation_index(&self, time: Timestamp) -> Option<usize> {
        if time.0 < 0 {
            return None;
        }

        match self.annotation_query_times.binary_search(&time.0) {
            Ok(index) => Some(index),
            Err(0) => None,
            Err(index) => Some(index - 1),
        }
    }

    fn hit_count_on_annotation(&self, file: &str, line: usize, annotation_index: usize) -> u64 {
        let annotation_time = self.annotation_sample_times[annotation_index];
        self.traces_for(file, line)
            .into_iter()
            .flatten()
            .map(|trace| trace.count_at(annotation_time))
            .sum()
    }

    fn traces_for(&self, file: &str, line: usize) -> Option<&[CoverageTrace]> {
        let normalized = normalize_file_key(file);
        self.traces_by_line
            .get(&(normalized, line))
            .map(Vec::as_slice)
    }
}

impl CoverageTracker for VcdCoverageTracker {
    fn is_line_covered_at(&self, file: &str, line: usize, time: Timestamp) -> Result<bool> {
        Ok(self.delta_hits(file, line, time)? > 0)
    }

    fn hit_count_at(&self, file: &str, line: usize, time: Timestamp) -> Result<u64> {
        let Some(annotation_index) = self.resolved_annotation_index(time) else {
            return Ok(0);
        };

        Ok(self.hit_count_on_annotation(file, line, annotation_index))
    }

    fn delta_hits(&self, file: &str, line: usize, time: Timestamp) -> Result<u64> {
        let Some(annotation_index) = self.resolved_annotation_index(time) else {
            return Ok(0);
        };

        let current = self.hit_count_on_annotation(file, line, annotation_index);
        let previous = annotation_index
            .checked_sub(1)
            .map(|previous_index| self.hit_count_on_annotation(file, line, previous_index))
            .unwrap_or(0);

        Ok(current.saturating_sub(previous))
    }

    fn clock_period(&self) -> Option<i64> {
        self.clock_period
    }

    fn is_posedge_time(&self, time: i64) -> bool {
        self.rising_edges.binary_search(&time).is_ok()
    }
}

impl CoverageTrace {
    fn count_at(&self, time: i64) -> u64 {
        match self
            .samples
            .binary_search_by_key(&time, |sample| sample.time)
        {
            Ok(index) => self.samples[index].count,
            Err(0) => 0,
            Err(index) => self.samples[index - 1].count,
        }
    }
}

fn collect_trace_descriptors(waveform: &Waveform) -> Vec<TraceDescriptor> {
    waveform
        .hierarchy()
        .iter_vars()
        .filter_map(|var| {
            let full_name = var.full_name(waveform.hierarchy());
            let base_name = full_name.rsplit('.').next().unwrap_or(full_name.as_str());
            let (file, line) = parse_trace_name(base_name)?;

            Some(TraceDescriptor {
                file,
                line,
                signal_ref: var.signal_ref(),
            })
        })
        .collect()
}

fn lookup_signal_ref(hierarchy: &Hierarchy, signal: &str) -> Result<Option<SignalRef>> {
    let lookup = build_signal_lookup(hierarchy);
    match lookup.get(signal) {
        Some(Some(signal_ref)) => Ok(Some(*signal_ref)),
        Some(None) => Err(anyhow!("clock signal lookup is ambiguous for {signal}")),
        None => Err(anyhow!("clock signal not found: {signal}")),
    }
}

struct AutoDetectResult {
    signal_ref: Option<SignalRef>,
    period: Option<i64>,
}

fn auto_detect_clock(
    waveform: &mut simple::Waveform,
    signal_refs: &mut Vec<SignalRef>,
) -> Result<AutoDetectResult> {
    let hierarchy = waveform.hierarchy();
    let all_vars: Vec<_> = hierarchy.iter_vars().collect();

    let clk_candidates: Vec<_> = all_vars
        .iter()
        .filter_map(|var| {
            let full_name = var.full_name(hierarchy);
            let base_name = full_name.rsplit('.').next().unwrap_or(full_name.as_str());
            let name_lower = base_name.to_lowercase();
            if name_lower.contains("clk") {
                Some((var.signal_ref(), full_name.clone(), base_name.to_string()))
            } else {
                None
            }
        })
        .collect();

    if clk_candidates.is_empty() {
        return Err(anyhow!(
            "auto-detect failed: no clock signal found (no signal containing 'clk' in name)"
        ));
    }

    for (signal_ref, _full_name, _base_name) in &clk_candidates {
        signal_refs.push(*signal_ref);
    }

    waveform.load_signals(signal_refs);

    let raw_times: Vec<i64> = waveform.time_table().iter().map(|t| *t as i64).collect();

    for (signal_ref, _full_name, _base_name) in &clk_candidates {
        let signal = match waveform.get_signal(*signal_ref) {
            Some(s) => s,
            None => continue,
        };

        if let Some(period) = detect_clock_period(signal, &raw_times) {
            return Ok(AutoDetectResult {
                signal_ref: Some(*signal_ref),
                period: Some(period),
            });
        }
    }

    let clk_names: Vec<_> = clk_candidates
        .iter()
        .map(|(_, _, name)| name.as_str())
        .collect();
    Err(anyhow!(
        "auto-detect failed: found clock candidates {:?} but none have constant period between rising edges",
        clk_names
    ))
}

fn detect_clock_period(signal: &wellen::Signal, raw_times: &[i64]) -> Option<i64> {
    let mut rising_times: Vec<i64> = Vec::new();
    let mut previous_bit: Option<char> = None;

    for (time_idx, value) in signal.iter_changes() {
        let time = *raw_times.get(time_idx as usize)?;
        let bits = value.to_bit_string()?;
        let current_bit = bits.chars().next()?;

        if matches!((previous_bit, current_bit), (Some('0'), '1')) {
            rising_times.push(time);
        }
        previous_bit = Some(current_bit);
    }

    if rising_times.len() < 2 {
        return None;
    }

    let first_interval = rising_times[1] - rising_times[0];
    if first_interval <= 0 {
        return None;
    }

    for window in rising_times.windows(2) {
        let interval = window[1] - window[0];
        if interval != first_interval {
            return None;
        }
    }

    Some(first_interval)
}

fn collect_rising_edges(signal: &wellen::Signal, raw_times: &[i64]) -> Result<Vec<i64>> {
    let mut rising_times: Vec<i64> = Vec::new();
    let mut previous_bit: Option<char> = None;

    for (time_idx, value) in signal.iter_changes() {
        let time = *raw_times
            .get(time_idx as usize)
            .ok_or_else(|| anyhow!("time index {} missing from waveform", time_idx))?;
        let bits = value
            .to_bit_string()
            .ok_or_else(|| anyhow!("signal value at time {} is not bit string", time))?;
        let current_bit = bits
            .chars()
            .next()
            .ok_or_else(|| anyhow!("signal at time {} has no bits", time))?;

        if matches!((previous_bit, current_bit), (Some('0'), '1')) {
            rising_times.push(time);
        }
        previous_bit = Some(current_bit);
    }

    Ok(rising_times)
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

fn parse_trace_name(name: &str) -> Option<(String, usize)> {
    let rest = name.strip_prefix(TRACE_PREFIX)?;
    let (file, line_and_type) = rest.rsplit_once("__")?;
    let (line, _) = line_and_type.rsplit_once('_')?;
    let line = line.parse().ok()?;
    Some((normalize_file_key(file), line))
}

fn extract_samples(
    signal: &wellen::Signal,
    annotation_times: &[i64],
) -> Result<Vec<CoverageSample>> {
    let mut samples: Vec<CoverageSample> = Vec::new();

    for (time_idx, value) in signal.iter_changes() {
        let time = *annotation_times.get(time_idx as usize).ok_or_else(|| {
            anyhow!("signal time index {time_idx} missing from waveform time table")
        })?;
        let bits = value
            .to_bit_string()
            .ok_or_else(|| anyhow!("coverage trace contains unsupported value at time {time}"))?;
        let count = parse_counter_bits(&bits)?;

        if let Some(last) = samples.last_mut() {
            if last.time == time {
                last.count = count;
                continue;
            }
        }

        samples.push(CoverageSample { time, count });
    }

    Ok(samples)
}

fn build_annotation_timeline(
    signal: &wellen::Signal,
    raw_times: &[i64],
    clk_step: i64,
) -> Result<(Vec<i64>, Vec<i64>)> {
    let mut annotation_query_times = Vec::new();
    let mut annotation_sample_times = Vec::new();
    let mut previous_bit = None;
    let mut annotation_time = 0i64;

    for (time_idx, value) in signal.iter_changes() {
        let time = *raw_times.get(time_idx as usize).ok_or_else(|| {
            anyhow!("clock time index {time_idx} missing from waveform time table")
        })?;
        let bits = value
            .to_bit_string()
            .ok_or_else(|| anyhow!("clock signal contains unsupported value at time {time}"))?;
        let current_bit = parse_single_bit(&bits)?;

        if matches!((previous_bit, current_bit), (Some('0'), '1')) {
            annotation_query_times.push(annotation_time);
            annotation_sample_times.push(time);
            annotation_time = annotation_time
                .checked_add(clk_step)
                .ok_or_else(|| anyhow!("annotation timeline overflow for clk_step {clk_step}"))?;
        }

        previous_bit = Some(current_bit);
    }

    Ok((annotation_query_times, annotation_sample_times))
}

fn parse_counter_bits(bits: &str) -> Result<u64> {
    if bits.is_empty() {
        return Ok(0);
    }

    if bits.chars().any(|bit| !matches!(bit, '0' | '1')) {
        return Err(anyhow!(
            "coverage counter is not a clean binary value: {bits}"
        ));
    }

    u64::from_str_radix(bits, 2)
        .with_context(|| format!("coverage counter does not fit in u64: {bits}"))
}

fn parse_single_bit(bits: &str) -> Result<char> {
    match bits {
        "0" | "1" => Ok(bits.as_bytes()[0] as char),
        _ => Err(anyhow!(
            "clock signal is not a clean single-bit value: {bits}"
        )),
    }
}

fn normalize_file_key(file: &str) -> String {
    file.replace('\\', "/")
}
