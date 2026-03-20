use crate::ast::AstProvider;
use crate::ast::SvParserProvider;
use crate::block::Blockizer;
use crate::block::{elaborate_block_set, BlockSet, DataflowBlockizer};
use crate::coverage::CoverageTracker;
use crate::coverage::{
    assignment_statement_coverage_report, StatementCoverageReport, VcdCoverageTracker,
};
use crate::error::{FuzzyMatch, SignalNotFound};
use crate::slicer::SliceRequest;
use crate::slicer::{BluesSlicer, StaticSlicer};
use crate::types::{SignalNode, Timestamp};
use crate::wave::WaveformReader;
use crate::wave::WellenReader;
use anyhow::Result;
use std::sync::Arc;

pub struct BlockizeRequest {
    pub sv_files: Vec<std::path::PathBuf>,
}

pub struct StaticSliceRequest {
    pub sv_files: Vec<std::path::PathBuf>,
    pub signal: String,
}

pub struct DynamicSliceRequest {
    pub sv_files: Vec<std::path::PathBuf>,
    pub signal: String,
    pub vcd: std::path::PathBuf,
    pub time: i64,
    pub min_time: i64,
    pub clock: Option<String>,
    pub clk_step: Option<i64>,
}

pub struct CoverageReportRequest {
    pub sv_files: Vec<std::path::PathBuf>,
    pub vcd: std::path::PathBuf,
    pub time: i64,
}

pub struct WaveValueRequest {
    pub vcd: std::path::PathBuf,
    pub signal: String,
    pub time: i64,
}

pub fn blockize(req: BlockizeRequest) -> Result<BlockSet> {
    let parsed_files = SvParserProvider.parse_files(&req.sv_files)?;
    let block_set = DataflowBlockizer.blockize(&parsed_files)?;
    Ok(block_set)
}

pub fn slice_static(req: StaticSliceRequest) -> Result<crate::types::StableSliceGraphJson> {
    let parsed_files = SvParserProvider.parse_files(&req.sv_files)?;
    let block_set =
        elaborate_block_set(&parsed_files, &DataflowBlockizer.blockize(&parsed_files)?)?;
    let request = SliceRequest {
        signal: SignalNode::named(req.signal),
        time: Timestamp(0),
        min_time: Timestamp(0),
    };
    let stable_json = StaticSlicer::new(block_set)
        .slice(&request)?
        .stable_json_graph()?;
    Ok(stable_json)
}

pub fn slice_dynamic(req: DynamicSliceRequest) -> Result<crate::types::StableSliceGraphJson> {
    let parsed_files = SvParserProvider.parse_files(&req.sv_files)?;
    let block_set =
        elaborate_block_set(&parsed_files, &DataflowBlockizer.blockize(&parsed_files)?)?;

    let coverage: Arc<dyn CoverageTracker + Send + Sync> = match (&req.clock, req.clk_step) {
        (Some(clock_name), Some(clk_step)) => Arc::new(VcdCoverageTracker::open_with_clock(
            &req.vcd, clock_name, clk_step,
        )?),
        (None, None) => Arc::new(VcdCoverageTracker::open(&req.vcd)?),
        _ => anyhow::bail!("both --clock and --clk-step must be provided together"),
    };

    if !coverage.is_posedge_time(req.time) {
        anyhow::bail!(
            "validation failed: --time {} is not a valid posedge time",
            req.time
        );
    }

    if let Some(clk_period) = coverage.clock_period() {
        let prev_time = req.time - clk_period;
        if prev_time >= req.min_time && !coverage.is_posedge_time(prev_time) {
            anyhow::bail!(
                "validation failed: time {} - clock period {} = {} is not a valid posedge time",
                req.time,
                clk_period,
                prev_time
            );
        }
    }

    let request = SliceRequest {
        signal: SignalNode::named(req.signal),
        time: Timestamp(req.time),
        min_time: Timestamp(req.min_time),
    };

    let stable_json = BluesSlicer::new(block_set, coverage)
        .slice(&request)?
        .stable_json_graph()?;
    Ok(stable_json)
}

pub fn coverage_report(req: CoverageReportRequest) -> Result<StatementCoverageReport> {
    let parsed_files = SvParserProvider.parse_files(&req.sv_files)?;
    let waveform = WellenReader::open(&req.vcd)?;
    let report =
        assignment_statement_coverage_report(&parsed_files, &waveform, Timestamp(req.time))?;
    Ok(report)
}

pub fn wave_value(req: WaveValueRequest) -> Result<crate::wave::SignalValue> {
    let reader = WellenReader::open(&req.vcd)?;
    let signal_name = req.signal.clone();
    let signal = SignalNode::named(signal_name.clone());
    let value = reader
        .signal_value_at(&signal, Timestamp(req.time))?
        .ok_or_else(|| {
            let candidates: Vec<String> = reader.signal_names().map(|s| s.to_string()).collect();
            let suggestions = FuzzyMatch::find_top_n(&signal_name, &candidates);
            SignalNotFound {
                signal: signal_name,
                suggestions,
            }
        })?;
    Ok(value)
}
