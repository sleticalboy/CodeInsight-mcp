mod cli;
mod config;
mod embedding;
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
        Command::InitConfig(args) => tools::init_config(args.root, args.force)?,
        Command::ConfigStatus(args) => tools::config_status(args.root)?,
        Command::Overview(args) => tools::project_overview(args.root)?,
        Command::Symbols(args) => tools::symbol_search(args.root, args.query, args.limit)?,
        Command::Outline(args) => tools::file_outline(args.path)?,
        Command::DependencyGraph(args) => {
            tools::dependency_graph(args.root, args.files, args.languages, args.limit)?
        }
        Command::ImpactAnalysis(args) => tools::impact_analysis(
            args.root,
            args.symbols,
            args.files,
            args.limit,
            args.depth,
            args.format,
            args.evidence_limit,
        )?,
        Command::FindReferences(args) => {
            tools::find_references(args.root, args.symbol, args.limit, args.include_definitions)?
        }
        Command::SemanticSearch(args) => tools::semantic_search(args.root, args.query, args.limit)?,
        Command::SemanticIndex(args) => {
            tools::semantic_index(args.root, args.chunk_lines, args.explain)?
        }
        Command::EmbeddingStatus(args) => tools::embedding_status(args.root)?,
        Command::ContextPack(args) => tools::context_pack(
            args.root,
            args.task,
            args.symbols,
            args.files,
            args.token_budget,
        )?,
        Command::Callers(args) => tools::callers(args.root, args.symbol, args.limit)?,
        Command::Callees(args) => tools::callees(args.root, args.symbol, args.limit)?,
        Command::Serve(args) => mcp::serve(args.transport).await?,
        Command::Version => tools::version()?,
    }

    Ok(())
}
