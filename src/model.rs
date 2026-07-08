use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    C,
    Cpp,
    #[serde(rename = "csharp")]
    CSharp,
    Go,
    Java,
    #[serde(rename = "javascript")]
    JavaScript,
    Php,
    Python,
    Ruby,
    Rust,
    #[serde(rename = "typescript")]
    TypeScript,
    Tsx,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::CSharp => "csharp",
            Self::Go => "go",
            Self::Java => "java",
            Self::JavaScript => "javascript",
            Self::Php => "php",
            Self::Python => "python",
            Self::Ruby => "ruby",
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::Tsx => "tsx",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Class,
    Function,
    Method,
    Interface,
    Struct,
    Variable,
    Constant,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
    pub hash: String,
    pub line_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Symbol {
    pub name: String,
    pub qualified_name: String,
    pub kind: SymbolKind,
    pub language: Language,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dependency {
    pub source_file: String,
    pub target: String,
    pub resolved_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_alias: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported_symbol: Option<String>,
    pub kind: String,
    pub language: Language,
    pub line: usize,
}

#[derive(Debug, Serialize)]
pub struct DependencyGraph {
    pub root: String,
    pub dependencies: Vec<Dependency>,
    pub nodes: usize,
    pub edges: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReferenceMatch {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub context: String,
    pub reference_kind: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticSearchResult {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f64,
    pub excerpt: String,
}

#[derive(Debug, Serialize)]
pub struct SemanticIndexReport {
    pub root: String,
    pub indexed_files: usize,
    pub chunks: usize,
    pub chunks_added: usize,
    pub chunks_updated: usize,
    pub chunks_removed: usize,
    pub embeddings: usize,
    pub embeddings_generated: usize,
    pub embeddings_reused: usize,
    pub chunk_lines: usize,
    pub provider: String,
    pub vector_status: String,
    pub errors: Vec<IndexError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub changes: Option<Vec<SemanticChunkChange>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticChunkWriteStats {
    pub total: usize,
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub changes: Vec<SemanticChunkChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SemanticChunkChange {
    pub change: String,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmbeddingProviderStatus {
    pub provider: String,
    pub model: String,
    pub configured: bool,
    pub source: String,
    pub provider_env: String,
    pub supported_providers: Vec<String>,
    pub batch_size: usize,
    pub batch_size_env: String,
    pub help: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama: Option<OllamaEmbeddingStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai: Option<OpenAiEmbeddingStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<SemanticIndexStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OllamaEmbeddingStatus {
    pub base_url: String,
    pub base_url_env: String,
    pub model_env: String,
    pub timeout_secs: u64,
    pub timeout_secs_env: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpenAiEmbeddingStatus {
    pub base_url: String,
    pub base_url_env: String,
    pub api_key_env: String,
    pub api_key_configured: bool,
    pub model_env: String,
    pub timeout_secs: u64,
    pub timeout_secs_env: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticIndexStatus {
    pub root: String,
    pub chunks: usize,
    pub embeddings: usize,
    pub vector_status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VersionInfo {
    pub name: String,
    pub version: String,
    pub target_arch: String,
    pub target_os: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigInitReport {
    pub root: String,
    pub path: String,
    pub created: bool,
    pub overwritten: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigStatusReport {
    pub root: String,
    pub path: String,
    pub exists: bool,
    pub loaded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,
    pub configured_test_commands: Vec<String>,
    pub configured_suggested_checks: usize,
    pub detected_test_commands: Vec<String>,
    pub commands_override_builtin: bool,
}

#[derive(Debug, Clone)]
pub struct SemanticChunkInput {
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub content_hash: String,
    pub token_estimate: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct SemanticChunk {
    pub id: i64,
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub token_estimate: usize,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct SemanticEmbeddingInput {
    pub chunk_id: i64,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SemanticEmbeddingMatch {
    pub chunk: SemanticChunk,
    pub vector: Vec<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallEdge {
    pub file: String,
    pub caller: String,
    pub callee: String,
    pub callee_file: Option<String>,
    pub language: Language,
    pub line: usize,
    pub column: usize,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPack {
    pub task: String,
    pub summary: String,
    pub semantic_status: ContextSemanticStatus,
    pub files: Vec<ContextFile>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<ReferenceMatch>,
    pub estimated_tokens: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSemanticStatus {
    pub provider: String,
    pub model: String,
    pub provider_configured: bool,
    pub vector_status: String,
    pub vector_candidates: usize,
    pub fallback_candidates: usize,
    pub selected_vector_ranges: usize,
    pub selected_fallback_ranges: usize,
    pub recommendation: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextFile {
    pub file: String,
    pub reason: String,
    pub ranges: Vec<ContextRange>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextRange {
    pub start_line: usize,
    pub end_line: usize,
    pub importance: String,
    pub reason: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactFile {
    pub file: String,
    pub score: i32,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactPath {
    pub kind: String,
    pub depth: usize,
    pub from: String,
    pub to: String,
    pub file: String,
    pub via: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactCounts {
    pub impacted_files: usize,
    pub paths: usize,
    pub symbols: usize,
    pub references: usize,
    pub callers: usize,
    pub callees: usize,
    pub dependencies: usize,
    pub errors: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SuggestedCheck {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ImpactAnalysisReport {
    pub root: String,
    pub summary: String,
    pub risk_level: String,
    pub impact_counts: ImpactCounts,
    pub top_reasons: Vec<String>,
    pub suggested_checks: Vec<SuggestedCheck>,
    pub depth: usize,
    pub format: String,
    pub evidence_limit: usize,
    pub seed_symbols: Vec<String>,
    pub seed_files: Vec<String>,
    pub impacted_files: Vec<ImpactFile>,
    pub paths: Vec<ImpactPath>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<ReferenceMatch>,
    pub callers: Vec<CallEdge>,
    pub callees: Vec<CallEdge>,
    pub dependencies: Vec<Dependency>,
    pub errors: Vec<IndexError>,
}

#[derive(Debug, Serialize)]
pub struct ProjectIndexReport {
    pub root: String,
    pub schema_version: i64,
    pub index_version: String,
    pub indexed_files: usize,
    pub changed_files: usize,
    pub unchanged_files: usize,
    pub deleted_files: usize,
    pub skipped_files: usize,
    pub symbols: usize,
    pub changed_symbols: usize,
    pub errors: Vec<IndexError>,
    pub duration_ms: u128,
}

#[derive(Debug, Serialize)]
pub struct IndexError {
    pub file: String,
    pub stage: String,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ProjectOverview {
    pub root: String,
    pub indexed_files: usize,
    pub symbols: usize,
    pub languages: Vec<LanguageStat>,
    pub top_directories: Vec<DirectoryStat>,
}

#[derive(Debug, Serialize)]
pub struct LanguageStat {
    pub language: String,
    pub files: usize,
    pub lines: usize,
}

#[derive(Debug, Serialize)]
pub struct DirectoryStat {
    pub directory: String,
    pub files: usize,
}
