use clap::{Parser, Subcommand};

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
    Blockize,
    Slice,
    Coverage,
    Wave,
}

fn main() {
    let _ = Cli::parse();
}
