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
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct BlockizeRequest {
    pub sv_files: Vec<PathBuf>,
    pub parse_options: ParseOptions,
}

pub struct CreateBlockizeArtifactRequest {
    pub sv_files: Vec<PathBuf>,
    pub parse_options: ParseOptions,
    pub artifact_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlockizeArtifactResponse {
    pub path: String,
    pub mode: String,
    pub block_set: BlockSet,
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

pub struct WaveSignalSearchRequest {
    pub vcd: PathBuf,
    pub query: String,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct WaveSignalSearchResponse {
    pub query: String,
    pub matches: Vec<String>,
}

pub struct CreateStaticSliceArtifactRequest {
    pub sv_files: Vec<PathBuf>,
    pub parse_options: ParseOptions,
    pub signal: String,
    pub artifact_dir: Option<PathBuf>,
}

pub struct CreateDynamicSliceArtifactRequest {
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
    pub artifact_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceArtifactResponse {
    pub path: String,
    pub target: String,
    pub mode: String,
    pub graph: crate::types::StableSliceGraphJson,
}

pub struct SliceArtifactQueryRequest {
    pub slice_json: Option<PathBuf>,
    pub artifact_dir: Option<PathBuf>,
    pub signal: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceSignalDriversResponse {
    pub slice_json: String,
    pub target: String,
    pub signals: Vec<SignalNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceBlockDriversResponse {
    pub slice_json: String,
    pub target: String,
    pub blocks: Vec<crate::types::BlockJson>,
}

#[derive(Debug, Clone, Default)]
pub struct BlocksQueryRequest {
    pub input: PathBuf,
    pub block_id: Option<u64>,
    pub output_signals: Vec<String>,
    pub input_signals: Vec<String>,
    pub scope: Option<String>,
    pub block_type: Option<String>,
    pub circuit_type: Option<String>,
    pub source_file: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BlocksQueryOutput {
    pub match_count: usize,
    pub blocks: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct SavedBlockSetJson {
    blocks: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct BlockQueryView {
    id: u64,
    block_type: String,
    circuit_type: String,
    module_scope: String,
    source_file: String,
    #[serde(default)]
    input_signals: Vec<String>,
    #[serde(default)]
    output_signals: Vec<String>,
}

pub fn blockize(req: BlockizeRequest) -> Result<BlockSet> {
    let parsed_files = parse_sv_files(&req.sv_files, &req.parse_options)?;
    let block_set =
        elaborate_block_set(&parsed_files, &DataflowBlockizer.blockize(&parsed_files)?)?;
    Ok(block_set)
}

pub fn create_blockize_json(
    req: CreateBlockizeArtifactRequest,
) -> Result<BlockizeArtifactResponse> {
    let block_set = blockize(BlockizeRequest {
        sv_files: req.sv_files,
        parse_options: req.parse_options,
    })?;
    write_blockize_artifact(block_set, req.artifact_dir)
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

pub fn wave_signal_search(req: WaveSignalSearchRequest) -> Result<WaveSignalSearchResponse> {
    let reader = WellenReader::open_metadata(&req.vcd)?;
    let matches = reader.search_signal_names(&req.query, req.limit);
    Ok(WaveSignalSearchResponse {
        query: req.query,
        matches,
    })
}

pub fn create_slice_json_static(
    req: CreateStaticSliceArtifactRequest,
) -> Result<SliceArtifactResponse> {
    let graph = slice_static(StaticSliceRequest {
        sv_files: req.sv_files,
        parse_options: req.parse_options,
        signal: req.signal,
    })?;
    write_slice_artifact("static", graph, req.artifact_dir)
}

pub fn create_slice_json_dynamic(
    req: CreateDynamicSliceArtifactRequest,
) -> Result<SliceArtifactResponse> {
    let graph = slice_dynamic(DynamicSliceRequest {
        sv_files: req.sv_files,
        parse_options: req.parse_options,
        signal: req.signal,
        vcd: req.vcd,
        tree_json: req.tree_json,
        tree_meta_json: req.tree_meta_json,
        time: req.time,
        min_time: req.min_time,
        clock: req.clock,
        clk_step: req.clk_step,
    })?;
    write_slice_artifact("dynamic", graph, req.artifact_dir)
}

pub fn query_slice_signal_drivers(
    req: SliceArtifactQueryRequest,
) -> Result<SliceSignalDriversResponse> {
    let path = resolve_slice_json(req.slice_json, req.artifact_dir)?;
    let graph = read_slice_graph(&path)?;
    let target = req.signal.unwrap_or_else(|| graph.target.clone());
    let driver_node_ids = direct_driver_node_ids(&graph, &target);
    let mut signals_by_name = BTreeMap::new();

    for edge in &graph.edges {
        if driver_node_ids.contains(&edge.to) {
            if let Some(signal) = &edge.signal {
                signals_by_name.insert(signal.name.clone(), signal.clone());
            }
        }
    }

    Ok(SliceSignalDriversResponse {
        slice_json: path.display().to_string(),
        target,
        signals: signals_by_name.into_values().collect(),
    })
}

pub fn query_slice_block_drivers(
    req: SliceArtifactQueryRequest,
) -> Result<SliceBlockDriversResponse> {
    let path = resolve_slice_json(req.slice_json, req.artifact_dir)?;
    let graph = read_slice_graph(&path)?;
    let target = req.signal.unwrap_or_else(|| graph.target.clone());
    let driver_block_ids = direct_driver_block_ids(&graph, &target);
    let mut blocks = graph
        .blocks
        .into_iter()
        .filter(|block| driver_block_ids.contains(&block.id.0))
        .collect::<Vec<_>>();
    blocks.sort_by_key(|block| block.id.0);

    Ok(SliceBlockDriversResponse {
        slice_json: path.display().to_string(),
        target,
        blocks,
    })
}

pub fn blocks_query(req: BlocksQueryRequest) -> Result<BlocksQueryOutput> {
    let input = fs::File::open(&req.input)
        .with_context(|| format!("failed to open blockize JSON: {}", req.input.display()))?;
    let saved: SavedBlockSetJson = serde_json::from_reader(input)
        .with_context(|| format!("failed to parse blockize JSON from {}", req.input.display()))?;

    let mut blocks = Vec::new();
    for block in saved.blocks {
        let query_view: BlockQueryView =
            serde_json::from_value(block.clone()).with_context(|| {
                format!("failed to parse a block entry from {}", req.input.display())
            })?;

        if block_matches(&query_view, &req) {
            blocks.push(block);
        }
    }

    Ok(BlocksQueryOutput {
        match_count: blocks.len(),
        blocks,
    })
}

fn write_slice_artifact(
    mode: &str,
    graph: crate::types::StableSliceGraphJson,
    artifact_dir: Option<PathBuf>,
) -> Result<SliceArtifactResponse> {
    let artifact_dir = artifact_dir.unwrap_or_else(|| PathBuf::from(".sva"));
    fs::create_dir_all(&artifact_dir).with_context(|| {
        format!(
            "failed to create artifact directory {}",
            artifact_dir.display()
        )
    })?;

    let target = graph.target.clone();
    let path = artifact_dir.join(format!(
        "slice-{mode}-{}-{}.json",
        safe_filename_segment(&target),
        unix_nanos()?
    ));
    fs::write(&path, serde_json::to_vec_pretty(&graph)?)
        .with_context(|| format!("failed to write slice artifact {}", path.display()))?;

    Ok(SliceArtifactResponse {
        path: path.display().to_string(),
        target,
        mode: mode.to_string(),
        graph,
    })
}

fn write_blockize_artifact(
    block_set: BlockSet,
    artifact_dir: Option<PathBuf>,
) -> Result<BlockizeArtifactResponse> {
    let artifact_dir = artifact_dir.unwrap_or_else(|| PathBuf::from(".sva"));
    fs::create_dir_all(&artifact_dir).with_context(|| {
        format!(
            "failed to create artifact directory {}",
            artifact_dir.display()
        )
    })?;

    let path = artifact_dir.join(format!("blockize-{}.json", unix_nanos()?));
    fs::write(&path, serde_json::to_vec_pretty(&block_set)?)
        .with_context(|| format!("failed to write blockize artifact {}", path.display()))?;

    Ok(BlockizeArtifactResponse {
        path: path.display().to_string(),
        mode: "blockize".to_string(),
        block_set,
    })
}

fn resolve_slice_json(
    slice_json: Option<PathBuf>,
    artifact_dir: Option<PathBuf>,
) -> Result<PathBuf> {
    if let Some(slice_json) = slice_json {
        return Ok(slice_json);
    }

    latest_slice_artifact(&artifact_dir.unwrap_or_else(|| PathBuf::from(".sva")))
}

fn latest_slice_artifact(artifact_dir: &Path) -> Result<PathBuf> {
    let entries = fs::read_dir(artifact_dir).with_context(|| {
        format!(
            "failed to read artifact directory {}",
            artifact_dir.display()
        )
    })?;
    let mut candidates = Vec::new();

    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to iterate artifact directory {}",
                artifact_dir.display()
            )
        })?;
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with("slice-") {
            continue;
        }
        let modified = entry
            .metadata()
            .and_then(|metadata| metadata.modified())
            .ok()
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        candidates.push((modified, path));
    }

    candidates
        .into_iter()
        .max_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)))
        .map(|(_, path)| path)
        .with_context(|| {
            format!(
                "no slice JSON artifact found in {}; call create_slice_json_static or create_slice_json_dynamic first",
                artifact_dir.display()
            )
        })
}

fn read_slice_graph(path: &Path) -> Result<crate::types::StableSliceGraphJson> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open slice JSON artifact {}", path.display()))?;
    serde_json::from_reader(file)
        .with_context(|| format!("failed to parse slice JSON artifact {}", path.display()))
}

fn direct_driver_block_ids(
    graph: &crate::types::StableSliceGraphJson,
    target: &str,
) -> BTreeSet<u64> {
    let node_to_block_id = block_node_id_map(graph);
    direct_driver_node_ids(graph, target)
        .into_iter()
        .filter_map(|node_id| node_to_block_id.get(&node_id).copied())
        .collect()
}

fn direct_driver_node_ids(
    graph: &crate::types::StableSliceGraphJson,
    target: &str,
) -> BTreeSet<usize> {
    let block_node_ids = block_node_id_map(graph)
        .into_keys()
        .collect::<BTreeSet<_>>();

    let driver_nodes_for_signal = graph
        .edges
        .iter()
        .filter_map(|edge| {
            edge.signal
                .as_ref()
                .is_some_and(|signal| signal.name == target)
                .then_some(edge.from)
        })
        .filter(|node_id| block_node_ids.contains(node_id))
        .collect::<BTreeSet<_>>();

    if !driver_nodes_for_signal.is_empty() {
        return driver_nodes_for_signal;
    }

    let outgoing_block_node_ids = graph
        .edges
        .iter()
        .map(|edge| edge.from)
        .filter(|node_id| block_node_ids.contains(node_id))
        .collect::<BTreeSet<_>>();

    block_node_ids
        .difference(&outgoing_block_node_ids)
        .copied()
        .collect()
}

fn block_node_id_map(graph: &crate::types::StableSliceGraphJson) -> BTreeMap<usize, u64> {
    graph
        .nodes
        .iter()
        .filter_map(|node| match node {
            crate::types::StableSliceNodeJson::Block { id, block_id, .. } => {
                Some((*id, block_id.0))
            }
            crate::types::StableSliceNodeJson::Literal { .. } => None,
        })
        .collect()
}

fn safe_filename_segment(value: &str) -> String {
    let safe = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if safe.is_empty() {
        "slice".to_string()
    } else {
        safe
    }
}

fn unix_nanos() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time is before UNIX_EPOCH")?
        .as_nanos())
}

fn block_matches(block: &BlockQueryView, req: &BlocksQueryRequest) -> bool {
    req.block_id.is_none_or(|block_id| block.id == block_id)
        && contains_all_signals(&block.output_signals, &req.output_signals)
        && contains_all_signals(&block.input_signals, &req.input_signals)
        && req
            .scope
            .as_deref()
            .is_none_or(|scope| scope_matches(&block.module_scope, scope))
        && req
            .block_type
            .as_deref()
            .is_none_or(|block_type| normalize_block_type(block_type) == block.block_type)
        && req
            .circuit_type
            .as_deref()
            .is_none_or(|circuit_type| normalize_circuit_type(circuit_type) == block.circuit_type)
        && req
            .source_file
            .as_deref()
            .is_none_or(|suffix| block.source_file.ends_with(suffix))
}

fn contains_all_signals(block_signals: &[String], requested_signals: &[String]) -> bool {
    requested_signals
        .iter()
        .all(|requested| block_signals.iter().any(|signal| signal == requested))
}

fn scope_matches(module_scope: &str, scope_prefix: &str) -> bool {
    module_scope == scope_prefix
        || module_scope
            .strip_prefix(scope_prefix)
            .is_some_and(|suffix| suffix.starts_with('.'))
}

fn normalize_block_type(value: &str) -> String {
    match normalize_filter_token(value).as_str() {
        "modinput" => "ModInput",
        "modoutput" => "ModOutput",
        "always" => "Always",
        "assign" => "Assign",
        _ => value,
    }
    .to_string()
}

fn normalize_circuit_type(value: &str) -> String {
    match normalize_filter_token(value).as_str() {
        "combinational" => "Combinational",
        "sequential" => "Sequential",
        _ => value,
    }
    .to_string()
}

fn normalize_filter_token(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_'))
        .flat_map(char::to_lowercase)
        .collect()
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
