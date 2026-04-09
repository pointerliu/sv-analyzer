use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;
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
    Slice(SliceArgs),
    Coverage(CoverageArgs),
    Wave(WaveArgs),
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
        Commands::Slice(args) => run_slice(args),
        Commands::Coverage(args) => run_coverage(args),
        Commands::Wave(args) => run_wave(args),
    }
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

fn parse_options(project_path: Option<PathBuf>, include_paths: Vec<PathBuf>) -> ParseOptions {
    ParseOptions {
        project_path,
        include_paths,
    }
}
