use std::{collections::BTreeMap, fs, path::PathBuf};

use anyhow::Result;

use crate::{
    index,
    model::{
        CallEdge, ContextFile, ContextPack, ContextRange, DependencyGraph, ProjectIndexReport,
        ProjectOverview, ReferenceMatch, Symbol,
    },
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

pub fn context_pack(
    root: PathBuf,
    task: String,
    symbols: Vec<String>,
    token_budget: usize,
) -> Result<()> {
    let pack = context_pack_value(root, task, symbols, token_budget)?;
    print_json(&pack)
}

pub fn callers(root: PathBuf, symbol: String, limit: usize) -> Result<()> {
    let calls = callers_value(root, &symbol, limit)?;
    print_json(&calls)
}

pub fn callees(root: PathBuf, symbol: String, limit: usize) -> Result<()> {
    let calls = callees_value(root, &symbol, limit)?;
    print_json(&calls)
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

pub fn context_pack_value(
    root: PathBuf,
    task: String,
    seed_symbols: Vec<String>,
    token_budget: usize,
) -> Result<ContextPack> {
    let root = root.canonicalize()?;
    let budget = token_budget.max(500);
    let mut symbols = Vec::new();
    let mut references = Vec::new();

    for seed in &seed_symbols {
        symbols.extend(symbol_search_value(root.clone(), seed, 8)?);
        references.extend(find_references_value(root.clone(), seed, 20, false)?);
    }

    let mut ranges_by_file: BTreeMap<String, Vec<(usize, usize, String)>> = BTreeMap::new();
    for symbol in &symbols {
        ranges_by_file
            .entry(symbol.file.clone())
            .or_default()
            .push((
                symbol.start_line,
                symbol.end_line,
                format!("Defines symbol {}", symbol.qualified_name),
            ));
    }
    for reference in &references {
        let start_line = reference.line.saturating_sub(2).max(1);
        let end_line = reference.line + 2;
        ranges_by_file
            .entry(reference.file.clone())
            .or_default()
            .push((
                start_line,
                end_line,
                format!("References symbol near line {}", reference.line),
            ));
    }

    let mut estimated_tokens = estimate_tokens(&task);
    let mut files = Vec::new();
    let mut truncated = false;

    for (file, ranges) in ranges_by_file {
        let path = root.join(&file);
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let lines = source.lines().collect::<Vec<_>>();
        let mut context_ranges = Vec::new();

        for (start_line, end_line, reason) in merge_ranges(ranges) {
            let excerpt = excerpt_lines(&lines, start_line, end_line);
            let range_tokens = estimate_tokens(&excerpt);
            if estimated_tokens + range_tokens > budget {
                truncated = true;
                continue;
            }
            estimated_tokens += range_tokens;
            context_ranges.push(ContextRange {
                start_line,
                end_line: end_line.min(lines.len().max(1)),
                importance: importance_for_reason(&reason).to_string(),
                excerpt,
            });
        }

        if !context_ranges.is_empty() {
            let reason = context_ranges
                .first()
                .map(|range| range.importance.clone())
                .unwrap_or_else(|| "medium".to_string());
            files.push(ContextFile {
                file,
                reason: format!("Selected for {reason} relevance to requested task"),
                ranges: context_ranges,
            });
        }
    }

    files.sort_by_key(|file| {
        if file.ranges.iter().any(|range| range.importance == "high") {
            0
        } else {
            1
        }
    });

    let summary = if seed_symbols.is_empty() {
        "No seed symbols were provided; context pack is empty.".to_string()
    } else {
        format!(
            "Context pack for task using seed symbols: {}.",
            seed_symbols.join(", ")
        )
    };

    Ok(ContextPack {
        task,
        summary,
        files,
        symbols,
        references,
        estimated_tokens,
        truncated,
    })
}

pub fn callers_value(root: PathBuf, symbol: &str, limit: usize) -> Result<Vec<CallEdge>> {
    let root = root.canonicalize()?;
    let store = Store::open(&root)?;
    store.callers(symbol, limit)
}

pub fn callees_value(root: PathBuf, symbol: &str, limit: usize) -> Result<Vec<CallEdge>> {
    let root = root.canonicalize()?;
    let store = Store::open(&root)?;
    store.callees(symbol, limit)
}

fn merge_ranges(mut ranges: Vec<(usize, usize, String)>) -> Vec<(usize, usize, String)> {
    ranges.sort_by_key(|(start, end, _)| (*start, *end));
    let mut merged: Vec<(usize, usize, String)> = Vec::new();

    for (start, end, reason) in ranges {
        if let Some((_, last_end, last_reason)) = merged.last_mut()
            && start <= *last_end + 2
        {
            *last_end = (*last_end).max(end);
            if !last_reason.contains(&reason) {
                last_reason.push_str("; ");
                last_reason.push_str(&reason);
            }
            continue;
        }
        merged.push((start, end, reason));
    }

    merged
}

fn excerpt_lines(lines: &[&str], start_line: usize, end_line: usize) -> String {
    let start = start_line.saturating_sub(1);
    let end = end_line.min(lines.len());
    lines
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{:>4}: {}", start + index + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn importance_for_reason(reason: &str) -> &'static str {
    if reason.contains("Defines symbol") {
        "high"
    } else {
        "medium"
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
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
