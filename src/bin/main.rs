use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand};
use dac26_mcp::ast::{AstProvider, SvParserProvider};
use dac26_mcp::block::{Blockizer, DataflowBlockizer};

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
    Slice,
    Coverage,
    Wave,
}

#[derive(Debug, Args)]
struct BlockizeArgs {
    #[arg(long = "sv", required = true)]
    sv_files: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Blockize(args) => run_blockize(args),
        Commands::Slice | Commands::Coverage | Commands::Wave => Ok(()),
    }
}

fn run_blockize(args: BlockizeArgs) -> Result<()> {
    let parsed_files = SvParserProvider.parse_files(&args.sv_files)?;
    let block_set = DataflowBlockizer.blockize(&parsed_files)?;
    println!("{}", serde_json::to_string_pretty(&block_set)?);
    Ok(())
}
