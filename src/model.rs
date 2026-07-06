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
    pub embeddings: usize,
    pub chunk_lines: usize,
    pub provider: String,
    pub vector_status: String,
    pub errors: Vec<IndexError>,
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
    pub file: String,
    pub start_line: usize,
    pub end_line: usize,
    pub token_estimate: usize,
    pub text: String,
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
    pub files: Vec<ContextFile>,
    pub symbols: Vec<Symbol>,
    pub references: Vec<ReferenceMatch>,
    pub estimated_tokens: usize,
    pub truncated: bool,
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
