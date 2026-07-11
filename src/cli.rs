use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "codeinsight")]
#[command(about = "Local-first code intelligence MCP server for AI agents")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Index a local repository.
    Index(IndexArgs),
    /// Create a sample project configuration file.
    InitConfig(InitConfigArgs),
    /// Print project configuration status.
    ConfigStatus(ProjectArgs),
    /// Print a project overview from the local index.
    Overview(ProjectArgs),
    /// Search symbols by name.
    Symbols(SymbolArgs),
    /// Print a source file outline.
    Outline(OutlineArgs),
    /// Print the indexed dependency graph.
    DependencyGraph(DependencyGraphArgs),
    /// Estimate local impact radius from seed symbols or files.
    ImpactAnalysis(ImpactAnalysisArgs),
    /// Find text references for a symbol in indexed files.
    FindReferences(FindReferencesArgs),
    /// Search indexed code with an embedding provider.
    SemanticSearch(SemanticSearchArgs),
    /// Build or refresh local semantic index chunks.
    SemanticIndex(SemanticIndexArgs),
    /// Print configured embedding provider and optional local semantic index status.
    EmbeddingStatus(EmbeddingStatusArgs),
    /// Build an agent-ready context pack from seed symbols, seed files, or inferred entrypoints.
    ContextPack(ContextPackArgs),
    /// Find callers for a function or method.
    Callers(CallQueryArgs),
    /// Find callees for a function or method.
    Callees(CallQueryArgs),
    /// Start the MCP server.
    Serve(ServeArgs),
    /// Print build version information.
    Version,
}

#[derive(Debug, Args)]
pub struct IndexArgs {
    pub root: PathBuf,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct InitConfigArgs {
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
    #[arg(long = "file")]
    pub files: Vec<String>,
    #[arg(long = "language")]
    pub languages: Vec<String>,
    #[arg(long, default_value_t = 500)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct ImpactAnalysisArgs {
    pub root: PathBuf,
    #[arg(long = "symbol")]
    pub symbols: Vec<String>,
    #[arg(long = "file")]
    pub files: Vec<String>,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
    #[arg(long, default_value_t = 1)]
    pub depth: usize,
    #[arg(long, default_value = "full")]
    pub format: String,
    #[arg(long, default_value_t = 20)]
    pub evidence_limit: usize,
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
pub struct SemanticSearchArgs {
    pub root: PathBuf,
    pub query: String,
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Args)]
pub struct SemanticIndexArgs {
    pub root: PathBuf,
    #[arg(long, default_value_t = 80)]
    pub chunk_lines: usize,
    #[arg(long)]
    pub explain: bool,
}

#[derive(Debug, Args)]
pub struct EmbeddingStatusArgs {
    pub root: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ContextPackArgs {
    pub root: PathBuf,
    #[arg(long)]
    pub task: String,
    #[arg(long = "symbol")]
    pub symbols: Vec<String>,
    #[arg(long = "file")]
    pub files: Vec<String>,
    #[arg(long, default_value_t = 6000)]
    pub token_budget: usize,
}

#[derive(Debug, Args)]
pub struct CallQueryArgs {
    pub root: PathBuf,
    pub symbol: String,
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
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
