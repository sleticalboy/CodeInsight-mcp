use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "codeinsight")]
#[command(about = "Local-first code intelligence MCP server for AI agents")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Index a local repository.
    Index(IndexArgs),
    /// Print a project overview from the local index.
    Overview(ProjectArgs),
    /// Search symbols by name.
    Symbols(SymbolArgs),
    /// Print a source file outline.
    Outline(OutlineArgs),
    /// Print the indexed dependency graph.
    DependencyGraph(DependencyGraphArgs),
    /// Find text references for a symbol in indexed files.
    FindReferences(FindReferencesArgs),
    /// Build an agent-ready context pack from seed symbols.
    ContextPack(ContextPackArgs),
    /// Start the MCP server.
    Serve(ServeArgs),
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    pub root: PathBuf,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct ProjectArgs {
    pub root: PathBuf,
}

#[derive(Debug, Args)]
pub struct SymbolArgs {
    pub root: PathBuf,
    pub query: String,
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct OutlineArgs {
    pub path: PathBuf,
}

#[derive(Debug, Args)]
pub struct DependencyGraphArgs {
    pub root: PathBuf,
    #[arg(long, default_value_t = 500)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct FindReferencesArgs {
    pub root: PathBuf,
    pub symbol: String,
    #[arg(long, default_value_t = 100)]
    pub limit: usize,
    #[arg(long)]
    pub include_definitions: bool,
}

#[derive(Debug, Args)]
pub struct ContextPackArgs {
    pub root: PathBuf,
    #[arg(long)]
    pub task: String,
    #[arg(long = "symbol", required = true)]
    pub symbols: Vec<String>,
    #[arg(long, default_value_t = 6000)]
    pub token_budget: usize,
}

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[arg(long, value_enum, default_value_t = Transport::Stdio)]
    pub transport: Transport,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Transport {
    Stdio,
}
