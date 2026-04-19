use crate::ast::{ParseOptions, SvParserProvider};
use crate::block::Blockizer;
use crate::block::{elaborate_block_set, BlockSet, DataflowBlockizer};
use crate::coverage::CoverageTracker;
use crate::coverage::{
    assignment_statement_coverage_report, ElaboratedCoverageTracker, StatementCoverageReport,
    VcdCoverageTracker, VerilatorElaborationIndex,
};
use crate::error::{FuzzyMatch, SignalNotFound};
use crate::slicer::SliceRequest;
use crate::slicer::{BluesSlicer, StaticSlicer};
use crate::types::{SignalNode, Timestamp};
use crate::wave::WaveformReader;
use crate::wave::{apply_scope_remap_to_graph, WellenReader};
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct BlockizeRequest {
    pub sv_files: Vec<PathBuf>,
    pub parse_options: ParseOptions,
}

pub struct StaticSliceRequest {
    pub sv_files: Vec<PathBuf>,
    pub parse_options: ParseOptions,
    pub signal: String,
}

pub struct DynamicSliceRequest {
    pub sv_files: Vec<PathBuf>,
    pub parse_options: ParseOptions,
    pub signal: String,
    pub vcd: PathBuf,
    pub tree_json: Option<PathBuf>,
    pub tree_meta_json: Option<PathBuf>,
    pub time: i64,
    pub min_time: i64,
    pub clock: Option<String>,
    pub clk_step: Option<i64>,
}

pub struct CoverageReportRequest {
    pub sv_files: Vec<PathBuf>,
    pub parse_options: ParseOptions,
    pub vcd: PathBuf,
    pub time: i64,
}

pub struct WaveValueRequest {
    pub vcd: PathBuf,
    pub signal: String,
    pub time: i64,
}

pub fn blockize(req: BlockizeRequest) -> Result<BlockSet> {
    let parsed_files = parse_sv_files(&req.sv_files, &req.parse_options)?;
    let block_set =
        elaborate_block_set(&parsed_files, &DataflowBlockizer.blockize(&parsed_files)?)?;
    Ok(block_set)
}

pub fn slice_static(req: StaticSliceRequest) -> Result<crate::types::StableSliceGraphJson> {
    let parsed_files = parse_sv_files(&req.sv_files, &req.parse_options)?;
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
    let parsed_files = parse_sv_files(&req.sv_files, &req.parse_options)?;
    let block_set =
        elaborate_block_set(&parsed_files, &DataflowBlockizer.blockize(&parsed_files)?)?;

    let base_coverage: Arc<dyn CoverageTracker + Send + Sync> = match (&req.clock, req.clk_step) {
        (Some(clock_name), Some(clk_step)) => Arc::new(VcdCoverageTracker::open_with_clock(
            &req.vcd, clock_name, clk_step,
        )?),
        (None, None) => Arc::new(VcdCoverageTracker::open(&req.vcd)?),
        _ => anyhow::bail!("both --clock and --clk-step must be provided together"),
    };
    let coverage: Arc<dyn CoverageTracker + Send + Sync> =
        if let Some(tree_json) = req.tree_json.as_ref() {
            Arc::new(ElaboratedCoverageTracker::new(
                base_coverage,
                VerilatorElaborationIndex::from_tree_json_file_with_meta(
                    tree_json,
                    req.tree_meta_json.as_ref(),
                )?,
            ))
        } else {
            base_coverage
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

    let mut stable_json = BluesSlicer::new(block_set, coverage)
        .slice(&request)?
        .stable_json_graph()?;

    // Rewrite scopes/signal names to FST-truthful paths so downstream tools
    // (waveform sampling, hierarchical lookups) see the generate-block
    // wrappers Verilator emits.
    let waveform_reader = WellenReader::open(&req.vcd)?;
    apply_scope_remap_to_graph(&waveform_reader, &mut stable_json);

    Ok(stable_json)
}

pub fn coverage_report(req: CoverageReportRequest) -> Result<StatementCoverageReport> {
    let parsed_files = parse_sv_files(&req.sv_files, &req.parse_options)?;
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

fn parse_sv_files(
    sv_files: &[PathBuf],
    parse_options: &ParseOptions,
) -> Result<Vec<crate::ast::ParsedFile>> {
    let resolved_files = resolve_sv_files(sv_files, parse_options.project_path.as_deref())?;
    SvParserProvider.parse_files_with_options(&resolved_files, parse_options)
}

fn resolve_sv_files(sv_files: &[PathBuf], project_path: Option<&Path>) -> Result<Vec<PathBuf>> {
    let mut resolved_files = sv_files.to_vec();

    if let Some(project_path) = project_path {
        resolved_files.extend(discover_project_sv_files(project_path)?);
    }

    resolved_files.sort();
    resolved_files.dedup();

    if resolved_files.is_empty() {
        anyhow::bail!("at least one SystemVerilog source is required via --sv or --project-path");
    }

    Ok(resolved_files)
}

fn discover_project_sv_files(project_path: &Path) -> Result<Vec<PathBuf>> {
    let metadata = fs::metadata(project_path)
        .with_context(|| format!("failed to read project path {}", project_path.display()))?;

    if metadata.is_file() {
        if is_systemverilog_source(project_path) {
            return Ok(vec![project_path.to_path_buf()]);
        }

        anyhow::bail!(
            "project path {} must be a directory or .sv file",
            project_path.display()
        );
    }

    if !metadata.is_dir() {
        anyhow::bail!(
            "project path {} must be a directory or .sv file",
            project_path.display()
        );
    }

    let mut discovered_files = Vec::new();
    collect_project_sv_files(project_path, &mut discovered_files)?;
    discovered_files.sort();
    Ok(discovered_files)
}

fn collect_project_sv_files(
    project_path: &Path,
    discovered_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let metadata = fs::metadata(project_path)
        .with_context(|| format!("failed to read project path {}", project_path.display()))?;

    if metadata.is_file() {
        if is_systemverilog_source(project_path) {
            discovered_files.push(project_path.to_path_buf());
        }

        return Ok(());
    }

    let mut entries = fs::read_dir(project_path)
        .with_context(|| format!("failed to read directory {}", project_path.display()))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| format!("failed to iterate directory {}", project_path.display()))?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        collect_project_sv_files(&entry.path(), discovered_files)?;
    }

    Ok(())
}

fn is_systemverilog_source(path: &Path) -> bool {
    matches!(path.extension().and_then(|ext| ext.to_str()), Some("sv"))
}
