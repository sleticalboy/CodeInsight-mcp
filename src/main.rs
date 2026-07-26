mod cli;
mod config;
mod embedding;
mod index;
mod language;
mod mcp;
mod model;
mod storage;
mod tools;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command};
use std::io::Read;

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
        Command::DependencyGraph(args) => tools::dependency_graph(
            args.root,
            args.files,
            args.languages,
            args.kinds,
            args.limit,
            args.offset,
        )?,
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
        Command::AgentRoute(args) => {
            let mut backend_evidence = if let Some(path) = args.backend_evidence.as_deref() {
                Some(tools::read_agent_route_backend_evidence(path)?)
            } else if let Some(json) = args.backend_evidence_json.as_deref() {
                let mut stdin_json = String::new();
                let json = if json == "-" {
                    std::io::stdin()
                        .read_to_string(&mut stdin_json)
                        .context("failed to read backend evidence JSON from stdin")?;
                    stdin_json.as_str()
                } else {
                    json
                };
                Some(
                    serde_json::from_str(json)
                        .context("failed to parse inline backend evidence JSON")?,
                )
            } else {
                None
            };
            if args.backend_fallback {
                backend_evidence
                    .as_mut()
                    .context("--backend-fallback requires backend evidence")?
                    .use_as_fallback = true;
            }
            if args.prefer_backend_context {
                backend_evidence
                    .as_mut()
                    .context("--prefer-backend-context requires backend evidence")?
                    .prefer_for_context = true;
            }
            tools::agent_route(
                args.root,
                args.task,
                args.symbols,
                args.files,
                args.token_budget,
                args.force_index,
                args.impact_limit,
                args.impact_depth,
                args.impact_evidence_limit,
                backend_evidence,
            )?
        }
        Command::Callers(args) => tools::callers(args.root, args.symbol, args.limit)?,
        Command::Callees(args) => tools::callees(args.root, args.symbol, args.limit)?,
        Command::Serve(args) => mcp::serve(args.transport).await?,
        Command::Version => tools::version()?,
    }

    Ok(())
}
