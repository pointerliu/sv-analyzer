use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use dac26_mcp::ast::{AstProvider, SvParserProvider};
use dac26_mcp::block::{elaborate_block_set, Blockizer, DataflowBlockizer};
use dac26_mcp::coverage::{assignment_statement_coverage_report, VcdCoverageTracker};
use dac26_mcp::slicer::{BluesSlicer, SliceRequest, Slicer, StaticSlicer};
use dac26_mcp::types::{SignalNode, Timestamp};
use dac26_mcp::wave::{WaveformReader, WellenReader};
use serde::Serialize;
use std::sync::Arc;

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
    Slice(SliceArgs),
    Coverage(CoverageArgs),
    Wave(WaveArgs),
}

#[derive(Debug, Args)]
struct BlockizeArgs {
    #[arg(long = "sv", required = true)]
    sv_files: Vec<PathBuf>,
}

#[derive(Debug, Args)]
struct CoverageArgs {
    #[arg(long = "sv", required = true)]
    sv_files: Vec<PathBuf>,
    #[arg(long)]
    vcd: PathBuf,
    #[arg(long)]
    time: i64,
}

#[derive(Debug, Args)]
struct SliceArgs {
    #[arg(long = "sv", required = true)]
    sv_files: Vec<PathBuf>,
    #[arg(long)]
    signal: String,
    #[arg(long)]
    vcd: Option<PathBuf>,
    #[arg(long)]
    time: Option<i64>,
    #[arg(long = "min-time")]
    min_time: Option<i64>,
    #[arg(long = "static", default_value_t = false)]
    static_slice: bool,
}

#[derive(Debug, Args)]
struct WaveArgs {
    #[arg(long)]
    vcd: PathBuf,
    #[arg(long)]
    signal: String,
    #[arg(long)]
    time: i64,
}

#[derive(Debug, Serialize)]
struct WaveOutput {
    signal: String,
    time: i64,
    value: Option<WaveValueOutput>,
}

#[derive(Debug, Serialize)]
struct WaveValueOutput {
    raw_bits: String,
    pretty_hex: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Blockize(args) => run_blockize(args),
        Commands::Slice(args) => run_slice(args),
        Commands::Coverage(args) => run_coverage(args),
        Commands::Wave(args) => run_wave(args),
    }
}

fn run_blockize(args: BlockizeArgs) -> Result<()> {
    let parsed_files = SvParserProvider.parse_files(&args.sv_files)?;
    let block_set = DataflowBlockizer.blockize(&parsed_files)?;
    println!("{}", serde_json::to_string_pretty(&block_set)?);
    Ok(())
}

fn run_coverage(args: CoverageArgs) -> Result<()> {
    let parsed_files = SvParserProvider.parse_files(&args.sv_files)?;
    let waveform = WellenReader::open(&args.vcd)?;
    let report =
        assignment_statement_coverage_report(&parsed_files, &waveform, Timestamp(args.time))?;

    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn run_slice(args: SliceArgs) -> Result<()> {
    if args.static_slice {
        let parsed_files = SvParserProvider.parse_files(&args.sv_files)?;
        let block_set =
            elaborate_block_set(&parsed_files, &DataflowBlockizer.blockize(&parsed_files)?)?;
        let request = SliceRequest {
            signal: SignalNode::named(args.signal),
            time: Timestamp(0),
            min_time: Timestamp(0),
        };

        let stable_json =
            Slicer::slice(&StaticSlicer::new(block_set), &request)?.stable_json_graph()?;
        println!("{}", serde_json::to_string_pretty(&stable_json)?);
        return Ok(());
    }

    let parsed_files = SvParserProvider.parse_files(&args.sv_files)?;
    let block_set =
        elaborate_block_set(&parsed_files, &DataflowBlockizer.blockize(&parsed_files)?)?;
    let vcd = args
        .vcd
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("--vcd is required unless --static is set"))?;
    let time = args
        .time
        .ok_or_else(|| anyhow::anyhow!("--time is required unless --static is set"))?;
    let min_time = args
        .min_time
        .ok_or_else(|| anyhow::anyhow!("--min-time is required unless --static is set"))?;
    let _waveform_reader = WellenReader::open(vcd)?;
    let coverage = Arc::new(VcdCoverageTracker::open(vcd)?);
    let request = SliceRequest {
        signal: SignalNode::named(args.signal),
        time: Timestamp(time),
        min_time: Timestamp(min_time),
    };

    let stable_json =
        Slicer::slice(&BluesSlicer::new(block_set, coverage), &request)?.stable_json_graph()?;

    println!("{}", serde_json::to_string_pretty(&stable_json)?);
    Ok(())
}

fn run_wave(args: WaveArgs) -> Result<()> {
    let reader = WellenReader::open(&args.vcd)?;
    let signal = SignalNode::named(args.signal.clone());
    let value = reader.signal_value_at(&signal, Timestamp(args.time))?;
    let output = WaveOutput {
        signal: args.signal,
        time: args.time,
        value: value.map(|value| WaveValueOutput {
            raw_bits: value.raw_bits,
            pretty_hex: value.pretty_hex,
        }),
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
