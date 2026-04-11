use std::fs::File;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sva_core::ast::ParseOptions;
use sva_core::services::{
    blockize, coverage_report, slice_dynamic, slice_static, wave_value, BlockizeRequest,
    CoverageReportRequest, DynamicSliceRequest, StaticSliceRequest, WaveValueRequest,
};

#[derive(Debug, Parser)]
#[command(name = "dataflow-engine")]
#[command(bin_name = "dataflow-engine")]
#[command(about = "Trait-based dataflow engine CLI scaffold")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Blockize(BlockizeArgs),
    Blocks(BlocksArgs),
    Slice(SliceArgs),
    Coverage(CoverageArgs),
    Wave(WaveArgs),
}

#[derive(Debug, Args)]
struct BlocksArgs {
    #[command(subcommand)]
    command: BlocksCommand,
}

#[derive(Debug, Subcommand)]
enum BlocksCommand {
    Query(BlockQueryArgs),
}

#[derive(Debug, Clone, ValueEnum)]
enum BlockTypeFilter {
    ModInput,
    ModOutput,
    Always,
    Assign,
}

impl BlockTypeFilter {
    fn json_name(&self) -> &'static str {
        match self {
            Self::ModInput => "ModInput",
            Self::ModOutput => "ModOutput",
            Self::Always => "Always",
            Self::Assign => "Assign",
        }
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum CircuitTypeFilter {
    Combinational,
    Sequential,
}

impl CircuitTypeFilter {
    fn json_name(&self) -> &'static str {
        match self {
            Self::Combinational => "Combinational",
            Self::Sequential => "Sequential",
        }
    }
}

#[derive(Debug, Args)]
struct BlockQueryArgs {
    #[arg(long, help = "Path to a saved blockize JSON file")]
    input: PathBuf,
    #[arg(long, help = "Exact block id match")]
    block_id: Option<u64>,
    #[arg(
        long = "output-signal",
        help = "Repeatable output signal filter; all must match"
    )]
    output_signals: Vec<String>,
    #[arg(
        long = "input-signal",
        help = "Repeatable input signal filter; all must match"
    )]
    input_signals: Vec<String>,
    #[arg(long, help = "Hierarchical scope prefix filter")]
    scope: Option<String>,
    #[arg(long, value_enum, help = "Exact block type filter")]
    block_type: Option<BlockTypeFilter>,
    #[arg(long, value_enum, help = "Exact circuit type filter")]
    circuit_type: Option<CircuitTypeFilter>,
    #[arg(long = "source-file", help = "Source file suffix filter")]
    source_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SavedBlockSetJson {
    blocks: Vec<Value>,
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

#[derive(Debug, Serialize)]
struct BlockQueryOutput {
    match_count: usize,
    blocks: Vec<Value>,
}

#[derive(Debug, Args)]
struct BlockizeArgs {
    #[arg(long = "sv")]
    sv_files: Vec<PathBuf>,
    #[arg(long, help = "Directory of .sv sources to parse recursively")]
    project_path: Option<PathBuf>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Comma-separated include paths for sv_parser"
    )]
    include_paths: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct CoverageArgs {
    #[arg(long = "sv")]
    sv_files: Vec<PathBuf>,
    #[arg(long, help = "Directory of .sv sources to parse recursively")]
    project_path: Option<PathBuf>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Comma-separated include paths for sv_parser"
    )]
    include_paths: Vec<PathBuf>,
    #[arg(long)]
    vcd: PathBuf,
    #[arg(long)]
    time: i64,
}

#[derive(Debug, Args)]
struct SliceArgs {
    #[arg(long = "sv")]
    sv_files: Vec<PathBuf>,
    #[arg(long, help = "Directory of .sv sources to parse recursively")]
    project_path: Option<PathBuf>,
    #[arg(
        long,
        value_delimiter = ',',
        help = "Comma-separated include paths for sv_parser"
    )]
    include_paths: Vec<PathBuf>,
    #[arg(long, help = "hierarchical signal name (e.g. tb.dut.u_stage3.result)")]
    signal: String,
    #[arg(long)]
    vcd: Option<PathBuf>,
    #[arg(long)]
    time: Option<i64>,
    #[arg(long = "min-time")]
    min_time: Option<i64>,
    #[arg(long = "static", default_value_t = false)]
    static_slice: bool,
    #[arg(long)]
    clock: Option<String>,
    #[arg(long)]
    clk_step: Option<i64>,
}

#[derive(Debug, Args)]
struct WaveArgs {
    #[arg(long)]
    vcd: PathBuf,
    #[arg(long, help = "hierarchical signal name (e.g. tb.dut.u_stage3.result)")]
    signal: String,
    #[arg(long)]
    time: i64,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Blockize(args) => run_blockize(args),
        Commands::Blocks(args) => run_blocks(args),
        Commands::Slice(args) => run_slice(args),
        Commands::Coverage(args) => run_coverage(args),
        Commands::Wave(args) => run_wave(args),
    }
}

fn run_blocks(args: BlocksArgs) -> Result<()> {
    match args.command {
        BlocksCommand::Query(args) => run_block_query(args),
    }
}

fn run_block_query(args: BlockQueryArgs) -> Result<()> {
    let input = File::open(&args.input)
        .with_context(|| format!("failed to open blockize JSON: {}", args.input.display()))?;
    let saved: SavedBlockSetJson = serde_json::from_reader(input).with_context(|| {
        format!(
            "failed to parse blockize JSON from {}",
            args.input.display()
        )
    })?;

    let mut blocks = Vec::new();

    for block in saved.blocks {
        let query_view: BlockQueryView =
            serde_json::from_value(block.clone()).with_context(|| {
                format!(
                    "failed to parse a block entry from {}",
                    args.input.display()
                )
            })?;

        if block_matches(&query_view, &args) {
            blocks.push(block);
        }
    }

    let output = BlockQueryOutput {
        match_count: blocks.len(),
        blocks,
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn run_blockize(args: BlockizeArgs) -> Result<()> {
    let block_set = blockize(BlockizeRequest {
        sv_files: args.sv_files,
        parse_options: parse_options(args.project_path, args.include_paths),
    })?;
    println!("{}", serde_json::to_string_pretty(&block_set)?);
    Ok(())
}

fn run_coverage(args: CoverageArgs) -> Result<()> {
    let report = coverage_report(CoverageReportRequest {
        sv_files: args.sv_files,
        parse_options: parse_options(args.project_path, args.include_paths),
        vcd: args.vcd,
        time: args.time,
    })?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run_slice(args: SliceArgs) -> Result<()> {
    if args.static_slice {
        run_static_slice(args)
    } else {
        run_blues(args)
    }
}

fn run_static_slice(args: SliceArgs) -> Result<()> {
    let stable_json = slice_static(StaticSliceRequest {
        sv_files: args.sv_files,
        parse_options: parse_options(args.project_path, args.include_paths),
        signal: args.signal,
    })?;
    println!("{}", serde_json::to_string_pretty(&stable_json)?);
    Ok(())
}

fn run_blues(args: SliceArgs) -> Result<()> {
    let parse_options = parse_options(args.project_path, args.include_paths);
    let vcd = args
        .vcd
        .ok_or_else(|| anyhow::anyhow!("--vcd is required unless --static is set"))?;
    let time = args
        .time
        .ok_or_else(|| anyhow::anyhow!("--time is required unless --static is set"))?;
    let min_time = args
        .min_time
        .ok_or_else(|| anyhow::anyhow!("--min-time is required unless --static is set"))?;

    let stable_json = slice_dynamic(DynamicSliceRequest {
        sv_files: args.sv_files,
        parse_options,
        signal: args.signal,
        vcd,
        time,
        min_time,
        clock: args.clock,
        clk_step: args.clk_step,
    })?;
    println!("{}", serde_json::to_string_pretty(&stable_json)?);
    Ok(())
}

fn run_wave(args: WaveArgs) -> Result<()> {
    let value = wave_value(WaveValueRequest {
        vcd: args.vcd,
        signal: args.signal.clone(),
        time: args.time,
    })?;
    #[derive(serde::Serialize)]
    struct WaveOutput {
        signal: String,
        time: i64,
        value: WaveValueOutput,
    }
    #[derive(serde::Serialize)]
    struct WaveValueOutput {
        raw_bits: String,
        pretty_hex: Option<String>,
    }
    let output = WaveOutput {
        signal: args.signal,
        time: args.time,
        value: WaveValueOutput {
            raw_bits: value.raw_bits,
            pretty_hex: value.pretty_hex,
        },
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn block_matches(block: &BlockQueryView, args: &BlockQueryArgs) -> bool {
    args.block_id.is_none_or(|block_id| block.id == block_id)
        && contains_all_signals(&block.output_signals, &args.output_signals)
        && contains_all_signals(&block.input_signals, &args.input_signals)
        && args
            .scope
            .as_deref()
            .is_none_or(|scope| scope_matches(&block.module_scope, scope))
        && args
            .block_type
            .as_ref()
            .is_none_or(|block_type| block.block_type == block_type.json_name())
        && args
            .circuit_type
            .as_ref()
            .is_none_or(|circuit_type| block.circuit_type == circuit_type.json_name())
        && args
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

fn parse_options(project_path: Option<PathBuf>, include_paths: Vec<PathBuf>) -> ParseOptions {
    ParseOptions {
        project_path,
        include_paths,
    }
}
