mod cli;
mod index;
mod language;
mod mcp;
mod model;
mod storage;
mod tools;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Index(args) => tools::index_project(args.root, args.force)?,
        Command::Overview(args) => tools::project_overview(args.root)?,
        Command::Symbols(args) => tools::symbol_search(args.root, args.query, args.limit)?,
        Command::Outline(args) => tools::file_outline(args.path)?,
        Command::Serve(args) => mcp::serve(args.transport).await?,
    }

    Ok(())
}
