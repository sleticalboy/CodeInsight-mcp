use std::{fs, path::PathBuf};

use anyhow::Result;

use crate::{
    index,
    model::{DependencyGraph, ProjectIndexReport, ProjectOverview, ReferenceMatch, Symbol},
    storage::Store,
};

pub fn index_project(root: PathBuf, force: bool) -> Result<()> {
    let report = index_project_value(root, force)?;
    print_json(&report)
}

pub fn project_overview(root: PathBuf) -> Result<()> {
    let overview = project_overview_value(root)?;
    print_json(&overview)
}

pub fn symbol_search(root: PathBuf, query: String, limit: usize) -> Result<()> {
    let symbols = symbol_search_value(root, &query, limit)?;
    print_json(&symbols)
}

pub fn file_outline(path: PathBuf) -> Result<()> {
    let symbols = file_outline_value(path)?;
    print_json(&symbols)
}

pub fn dependency_graph(root: PathBuf, limit: usize) -> Result<()> {
    let graph = dependency_graph_value(root, limit)?;
    print_json(&graph)
}

pub fn find_references(
    root: PathBuf,
    symbol: String,
    limit: usize,
    include_definitions: bool,
) -> Result<()> {
    let references = find_references_value(root, &symbol, limit, include_definitions)?;
    print_json(&references)
}

pub fn index_project_value(root: PathBuf, force: bool) -> Result<ProjectIndexReport> {
    index::index_project(&root, force)
}

pub fn project_overview_value(root: PathBuf) -> Result<ProjectOverview> {
    let root = root.canonicalize()?;
    let store = Store::open(&root)?;
    store.overview(&root)
}

pub fn symbol_search_value(root: PathBuf, query: &str, limit: usize) -> Result<Vec<Symbol>> {
    let root = root.canonicalize()?;
    let store = Store::open(&root)?;
    store.search_symbols(query, limit)
}

pub fn file_outline_value(path: PathBuf) -> Result<Vec<Symbol>> {
    index::outline_file(&path)
}

pub fn dependency_graph_value(root: PathBuf, limit: usize) -> Result<DependencyGraph> {
    let root = root.canonicalize()?;
    let store = Store::open(&root)?;
    store.dependency_graph(&root, limit)
}

pub fn find_references_value(
    root: PathBuf,
    symbol: &str,
    limit: usize,
    include_definitions: bool,
) -> Result<Vec<ReferenceMatch>> {
    let root = root.canonicalize()?;
    let store = Store::open(&root)?;
    let files = store.indexed_files()?;
    let mut matches = Vec::new();

    for relative_path in files {
        if matches.len() >= limit {
            break;
        }

        let path = root.join(&relative_path);
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };

        for (line_index, line) in source.lines().enumerate() {
            if matches.len() >= limit {
                break;
            }
            if !include_definitions && looks_like_definition(line, symbol) {
                continue;
            }

            for column in symbol_columns(line, symbol) {
                matches.push(ReferenceMatch {
                    file: relative_path.clone(),
                    line: line_index + 1,
                    column: column + 1,
                    context: line.trim().to_string(),
                    reference_kind: classify_reference(line, symbol).to_string(),
                    confidence: confidence_for_line(line, symbol),
                });
                if matches.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(matches)
}

fn symbol_columns(line: &str, symbol: &str) -> Vec<usize> {
    if symbol.is_empty() {
        return Vec::new();
    }

    let mut columns = Vec::new();
    let mut search_start = 0;
    while let Some(offset) = line[search_start..].find(symbol) {
        let column = search_start + offset;
        let end = column + symbol.len();
        if is_boundary(line, column, end) {
            columns.push(column);
        }
        search_start = end;
    }
    columns
}

fn is_boundary(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();
    before.is_none_or(|ch| !is_identifier_char(ch))
        && after.is_none_or(|ch| !is_identifier_char(ch))
}

fn is_identifier_char(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

fn looks_like_definition(line: &str, symbol: &str) -> bool {
    let trimmed = line.trim_start();
    let patterns = [
        format!("fn {symbol}"),
        format!("def {symbol}"),
        format!("function {symbol}"),
        format!("class {symbol}"),
        format!("struct {symbol}"),
        format!("interface {symbol}"),
        format!("type {symbol}"),
        format!("const {symbol}"),
        format!("let {symbol}"),
        format!("var {symbol}"),
    ];
    patterns.iter().any(|pattern| trimmed.starts_with(pattern))
}

fn classify_reference(line: &str, symbol: &str) -> &'static str {
    let trimmed = line.trim_start();
    if looks_like_definition(line, symbol) {
        "definition"
    } else if trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("use ")
        || trimmed.contains(" require(")
    {
        "import"
    } else if trimmed.contains(&format!("{symbol}(")) || trimmed.contains(&format!(".{symbol}(")) {
        "call"
    } else {
        "text"
    }
}

fn confidence_for_line(line: &str, symbol: &str) -> f64 {
    match classify_reference(line, symbol) {
        "call" => 0.8,
        "import" => 0.7,
        "definition" => 0.6,
        _ => 0.4,
    }
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
