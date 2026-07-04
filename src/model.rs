use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Go,
    JavaScript,
    Python,
    Rust,
    TypeScript,
    Tsx,
}

impl Language {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Go => "go",
            Self::JavaScript => "javascript",
            Self::Python => "python",
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

#[derive(Debug, Serialize)]
pub struct ProjectIndexReport {
    pub root: String,
    pub indexed_files: usize,
    pub skipped_files: usize,
    pub symbols: usize,
    pub duration_ms: u128,
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
