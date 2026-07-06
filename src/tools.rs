use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use crate::{
    index,
    model::{
        CallEdge, ContextFile, ContextPack, ContextRange, DependencyGraph, ProjectIndexReport,
        ProjectOverview, ReferenceMatch, Symbol, SymbolKind,
    },
    storage::Store,
};

const CONTEXT_SCORE_SEED_FILE: i32 = 130;
const CONTEXT_SCORE_SEED_HEADER: i32 = 140;
const CONTEXT_SCORE_SYMBOL_DEFINITION: i32 = 90;
const CONTEXT_SCORE_CALL_GRAPH: i32 = 75;
const CONTEXT_SCORE_REFERENCE_BASE: i32 = 60;
const CONTEXT_SCORE_LOCAL_DEPENDENCY: i32 = 40;
const CONTEXT_SCORE_TASK_MATCH_BOOST: i32 = 30;
const CONTEXT_SCORE_SEED_SYMBOL_TASK_MATCH_BOOST: i32 = 5;
const CONTEXT_MAX_SYMBOL_LINES: usize = 80;
const CONTEXT_MAX_MERGED_RANGE_LINES: usize = 80;

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
    files: Vec<String>,
    token_budget: usize,
) -> Result<()> {
    let pack = context_pack_value(root, task, symbols, files, token_budget)?;
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
    seed_files: Vec<String>,
    token_budget: usize,
) -> Result<ContextPack> {
    let root = root.canonicalize()?;
    if seed_symbols.is_empty() && seed_files.is_empty() {
        bail!("context_pack requires at least one seed symbol or file");
    }

    let budget = token_budget.max(500);
    let task_keywords = task_keywords(&task);
    let mut symbols = Vec::new();
    let mut references = Vec::new();
    let seed_files = seed_files
        .iter()
        .map(|file| normalize_seed_file(&root, file))
        .collect::<Result<Vec<_>>>()?;
    let seed_file_set = seed_files.iter().cloned().collect::<BTreeSet<_>>();

    for seed in &seed_symbols {
        symbols.extend(symbol_search_value(root.clone(), seed, 8)?);
        references.extend(find_references_value(root.clone(), seed, 20, false)?);
    }
    for file in &seed_files {
        let mut file_symbols = file_outline_value(root.join(file))?;
        for symbol in &mut file_symbols {
            symbol.file = file.clone();
        }
        symbols.extend(file_symbols);
    }

    let mut ranges_by_file: BTreeMap<String, Vec<ContextCandidateRange>> = BTreeMap::new();
    for file in &seed_files {
        for range in seed_file_ranges(&root, file, &symbols, &task_keywords) {
            push_context_range(
                &mut ranges_by_file,
                file.clone(),
                range.start_line,
                range.end_line,
                range.reason,
                range.score,
            );
        }
    }
    for symbol in &symbols {
        if seed_file_set.contains(&symbol.file) {
            continue;
        }
        push_context_range(
            &mut ranges_by_file,
            symbol.file.clone(),
            symbol.start_line,
            capped_symbol_end_line(symbol),
            format!("Defines symbol {}", symbol.qualified_name),
            CONTEXT_SCORE_SYMBOL_DEFINITION + symbol_task_boost(symbol, &task_keywords),
        );
    }
    for reference in &references {
        let start_line = reference.line.saturating_sub(2).max(1);
        let end_line = reference.line + 2;
        push_context_range(
            &mut ranges_by_file,
            reference.file.clone(),
            start_line,
            end_line,
            format!("References symbol near line {}", reference.line),
            reference_score(reference) + reference_task_boost(reference, &task_keywords),
        );
    }

    let mut callee_graph_seeds = seed_symbols.iter().cloned().collect::<BTreeSet<_>>();
    let mut caller_graph_seeds = seed_symbols.iter().cloned().collect::<BTreeSet<_>>();
    let mut seed_file_primary_symbols: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for symbol in &symbols {
        if seed_file_set.contains(&symbol.file) && is_primary_seed_symbol(symbol) {
            callee_graph_seeds.insert(symbol.qualified_name.clone());
            seed_file_primary_symbols
                .entry(symbol.file.clone())
                .or_default()
                .insert(symbol.qualified_name.clone());
        }
    }
    for symbols in seed_file_primary_symbols.values() {
        if symbols.len() <= 4 {
            caller_graph_seeds.extend(symbols.iter().cloned());
        }
    }

    let store = Store::open(&root)?;
    for seed in &caller_graph_seeds {
        for call in store.callers(seed, 20)? {
            push_context_range(
                &mut ranges_by_file,
                call.file.clone(),
                call.line.saturating_sub(2).max(1),
                call.line + 2,
                format!("Call graph caller of {} via {}", call.callee, call.caller),
                CONTEXT_SCORE_CALL_GRAPH + call_task_boost(&call, &task_keywords),
            );
        }
    }
    for seed in &callee_graph_seeds {
        for call in store.callees(seed, 20)? {
            let Some(callee_file) = call.callee_file.clone() else {
                continue;
            };
            push_context_range(
                &mut ranges_by_file,
                callee_file,
                1,
                40,
                format!("Call graph target of {} via {}", call.caller, call.callee),
                CONTEXT_SCORE_CALL_GRAPH + call_task_boost(&call, &task_keywords),
            );
        }
    }

    let selected_files = ranges_by_file.keys().cloned().collect::<Vec<_>>();
    for dependency in store.resolved_dependencies_for_files(&selected_files)? {
        let score =
            CONTEXT_SCORE_LOCAL_DEPENDENCY + dependency_task_boost(&dependency, &task_keywords);
        if let Some(resolved_file) = dependency.resolved_file {
            push_context_range(
                &mut ranges_by_file,
                resolved_file,
                1,
                40,
                format!(
                    "Local dependency of {} via {}",
                    dependency.source_file, dependency.target
                ),
                score,
            );
        }
    }

    let mut candidates = ranges_by_file
        .into_iter()
        .map(|(file, ranges)| {
            let total_score = ranges.iter().map(|range| range.score).sum();
            let mut ranges = merge_ranges(ranges);
            ranges.sort_by(compare_context_ranges_for_budget);
            let max_score = ranges.iter().map(|range| range.score).max().unwrap_or(0);
            ContextFileCandidate {
                file,
                ranges,
                max_score,
                total_score,
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(compare_context_file_candidates);

    let mut estimated_tokens = estimate_tokens(&task);
    let mut files = Vec::new();
    let mut truncated = false;

    for candidate in candidates {
        let path = root.join(&candidate.file);
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let lines = source.lines().collect::<Vec<_>>();
        let mut context_ranges = Vec::new();
        let mut selected_max_score = 0;

        for range in candidate.ranges {
            let mut end_line = range.end_line.min(lines.len().max(1));
            let mut excerpt = excerpt_lines(&lines, range.start_line, end_line);
            let mut range_tokens = estimate_tokens(&excerpt);
            if estimated_tokens + range_tokens > budget {
                truncated = true;
                if range.score >= CONTEXT_SCORE_SEED_FILE {
                    let remaining_budget = budget.saturating_sub(estimated_tokens);
                    if let Some((fitted_end_line, fitted_excerpt, fitted_tokens)) =
                        fit_context_range_to_budget(
                            &lines,
                            range.start_line,
                            end_line,
                            remaining_budget,
                        )
                    {
                        end_line = fitted_end_line;
                        excerpt = fitted_excerpt;
                        range_tokens = fitted_tokens;
                    } else {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            estimated_tokens += range_tokens;
            selected_max_score = selected_max_score.max(range.score);
            context_ranges.push(ContextRange {
                start_line: range.start_line,
                end_line,
                importance: importance_for_score(range.score).to_string(),
                excerpt,
            });
        }

        if !context_ranges.is_empty() {
            files.push(ContextFile {
                file: candidate.file,
                reason: format!(
                    "Selected for {} relevance to requested task",
                    importance_for_score(selected_max_score)
                ),
                ranges: context_ranges,
            });
        }
    }

    let summary = if seed_symbols.is_empty() && seed_files.is_empty() {
        "No seed symbols or files were provided; context pack is empty.".to_string()
    } else if seed_files.is_empty() {
        format!(
            "Context pack for task using seed symbols: {}.",
            seed_symbols.join(", ")
        )
    } else if seed_symbols.is_empty() {
        format!(
            "Context pack for task using seed files: {}.",
            seed_files.join(", ")
        )
    } else {
        format!(
            "Context pack for task using seed symbols: {} and seed files: {}.",
            seed_symbols.join(", "),
            seed_files.join(", ")
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

#[derive(Debug, Clone)]
struct ContextCandidateRange {
    start_line: usize,
    end_line: usize,
    reason: String,
    score: i32,
}

#[derive(Debug)]
struct ContextFileCandidate {
    file: String,
    ranges: Vec<ContextCandidateRange>,
    max_score: i32,
    total_score: i32,
}

fn seed_file_ranges(
    root: &Path,
    file: &str,
    symbols: &[Symbol],
    task_keywords: &[String],
) -> Vec<ContextCandidateRange> {
    let path = root.join(file);
    let source = fs::read_to_string(path).unwrap_or_default();
    let lines = source.lines().collect::<Vec<_>>();
    let line_count = lines.len().max(1);
    let mut ranges = Vec::new();

    if let Some(end_line) = header_range_end(&lines) {
        ranges.push(ContextCandidateRange {
            start_line: 1,
            end_line,
            reason: format!("Seed file header and imports for task: {file}"),
            score: CONTEXT_SCORE_SEED_HEADER,
        });
    }

    let mut primary_symbols = symbols
        .iter()
        .filter(|symbol| symbol.file == file && is_primary_seed_symbol(symbol))
        .collect::<Vec<_>>();
    primary_symbols.sort_by_key(|symbol| (symbol.start_line, symbol.end_line));

    for symbol in primary_symbols.into_iter().take(12) {
        ranges.push(ContextCandidateRange {
            start_line: symbol.start_line.saturating_sub(2).max(1),
            end_line: (capped_symbol_end_line(symbol) + 2).min(line_count),
            reason: format!("Seed file defines symbol {}", symbol.qualified_name),
            score: CONTEXT_SCORE_SEED_FILE + seed_symbol_task_boost(symbol, task_keywords),
        });
    }

    if ranges.is_empty() {
        ranges.push(ContextCandidateRange {
            start_line: 1,
            end_line: line_count.min(80),
            reason: format!("Seed file requested for task: {file}"),
            score: CONTEXT_SCORE_SEED_FILE,
        });
    }

    ranges
}

fn header_range_end(lines: &[&str]) -> Option<usize> {
    let mut end_line = None;
    let mut saw_header = false;

    for (index, line) in lines.iter().enumerate().take(40) {
        let line_number = index + 1;
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if saw_header {
                end_line = Some(line_number);
            }
            continue;
        }

        if is_header_line(trimmed) {
            saw_header = true;
            end_line = Some(line_number);
            continue;
        }

        if !saw_header && is_leading_comment(trimmed) {
            end_line = Some(line_number);
            continue;
        }

        break;
    }

    end_line
}

fn is_primary_seed_symbol(symbol: &Symbol) -> bool {
    !symbol.qualified_name.contains('.')
        && matches!(
            symbol.kind,
            SymbolKind::Class | SymbolKind::Function | SymbolKind::Interface | SymbolKind::Struct
        )
}

fn capped_symbol_end_line(symbol: &Symbol) -> usize {
    symbol
        .end_line
        .min(symbol.start_line + CONTEXT_MAX_SYMBOL_LINES.saturating_sub(1))
}

fn is_header_line(trimmed: &str) -> bool {
    trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("mod ")
        || trimmed.starts_with("pub mod ")
        || trimmed.starts_with("extern crate ")
        || trimmed.starts_with("#![")
        || trimmed.starts_with("package ")
}

fn is_leading_comment(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with('#')
}

fn push_context_range(
    ranges_by_file: &mut BTreeMap<String, Vec<ContextCandidateRange>>,
    file: String,
    start_line: usize,
    end_line: usize,
    reason: String,
    score: i32,
) {
    ranges_by_file
        .entry(file)
        .or_default()
        .push(ContextCandidateRange {
            start_line,
            end_line,
            reason,
            score,
        });
}

fn compare_context_file_candidates(
    left: &ContextFileCandidate,
    right: &ContextFileCandidate,
) -> Ordering {
    right
        .max_score
        .cmp(&left.max_score)
        .then_with(|| right.total_score.cmp(&left.total_score))
        .then_with(|| left.file.cmp(&right.file))
}

fn compare_context_ranges_for_budget(
    left: &ContextCandidateRange,
    right: &ContextCandidateRange,
) -> Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| left.start_line.cmp(&right.start_line))
        .then_with(|| left.end_line.cmp(&right.end_line))
}

fn merge_ranges(mut ranges: Vec<ContextCandidateRange>) -> Vec<ContextCandidateRange> {
    ranges.sort_by_key(|range| (range.start_line, range.end_line));
    let mut merged: Vec<ContextCandidateRange> = Vec::new();

    for range in ranges {
        if let Some(last) = merged.last_mut()
            && range.start_line <= last.end_line + 2
            && range.score == last.score
            && range_len(last.start_line, range.end_line) <= CONTEXT_MAX_MERGED_RANGE_LINES
        {
            last.end_line = last.end_line.max(range.end_line);
            last.score = last.score.max(range.score);
            if !last.reason.contains(&range.reason) {
                last.reason.push_str("; ");
                last.reason.push_str(&range.reason);
            }
            continue;
        }
        merged.push(range);
    }

    merged
}

fn range_len(start_line: usize, end_line: usize) -> usize {
    end_line.saturating_sub(start_line) + 1
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

fn fit_context_range_to_budget(
    lines: &[&str],
    start_line: usize,
    end_line: usize,
    token_budget: usize,
) -> Option<(usize, String, usize)> {
    if token_budget == 0 {
        return None;
    }

    let max_end_line = end_line.min(lines.len().max(1));
    if start_line > max_end_line {
        return None;
    }

    let first_excerpt = excerpt_lines(lines, start_line, start_line);
    let first_tokens = estimate_tokens(&first_excerpt);
    if first_tokens > token_budget {
        return None;
    }

    let mut best_end_line = start_line;
    let mut best_excerpt = first_excerpt;
    let mut best_tokens = first_tokens;
    let mut low = start_line + 1;
    let mut high = max_end_line;

    while low <= high {
        let mid = low + (high - low) / 2;
        let excerpt = excerpt_lines(lines, start_line, mid);
        let tokens = estimate_tokens(&excerpt);
        if tokens <= token_budget {
            best_end_line = mid;
            best_excerpt = excerpt;
            best_tokens = tokens;
            low = mid + 1;
        } else {
            high = mid.saturating_sub(1);
        }
    }

    Some((best_end_line, best_excerpt, best_tokens))
}

fn importance_for_score(score: i32) -> &'static str {
    if score >= CONTEXT_SCORE_SYMBOL_DEFINITION {
        "high"
    } else {
        "medium"
    }
}

fn reference_score(reference: &ReferenceMatch) -> i32 {
    CONTEXT_SCORE_REFERENCE_BASE + (reference.confidence * 10.0).round() as i32
}

fn symbol_task_boost(symbol: &Symbol, keywords: &[String]) -> i32 {
    task_match_boost(
        keywords,
        [
            symbol.name.as_str(),
            symbol.qualified_name.as_str(),
            symbol.file.as_str(),
        ]
        .into_iter(),
    )
}

fn seed_symbol_task_boost(symbol: &Symbol, keywords: &[String]) -> i32 {
    if symbol_task_boost(symbol, keywords) > 0 {
        CONTEXT_SCORE_SEED_SYMBOL_TASK_MATCH_BOOST
    } else {
        0
    }
}

fn reference_task_boost(reference: &ReferenceMatch, keywords: &[String]) -> i32 {
    task_match_boost(
        keywords,
        [
            reference.file.as_str(),
            reference.context.as_str(),
            reference.reference_kind.as_str(),
        ]
        .into_iter(),
    )
}

fn dependency_task_boost(dependency: &crate::model::Dependency, keywords: &[String]) -> i32 {
    task_match_boost(
        keywords,
        [
            dependency.source_file.as_str(),
            dependency.target.as_str(),
            dependency.resolved_file.as_deref().unwrap_or_default(),
        ]
        .into_iter(),
    )
}

fn call_task_boost(call: &CallEdge, keywords: &[String]) -> i32 {
    task_match_boost(
        keywords,
        [
            call.file.as_str(),
            call.caller.as_str(),
            call.callee.as_str(),
            call.callee_file.as_deref().unwrap_or_default(),
        ]
        .into_iter(),
    )
}

fn task_match_boost<'a>(keywords: &[String], fields: impl Iterator<Item = &'a str>) -> i32 {
    if keywords.is_empty() {
        return 0;
    }

    let haystack = fields
        .map(|field| field.to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    if keywords.iter().any(|keyword| haystack.contains(keyword)) {
        CONTEXT_SCORE_TASK_MATCH_BOOST
    } else {
        0
    }
}

fn task_keywords(task: &str) -> Vec<String> {
    task.split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| word.len() >= 3 && !is_task_stop_word(word))
        .take(16)
        .collect()
}

fn is_task_stop_word(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "and"
            | "for"
            | "with"
            | "from"
            | "into"
            | "this"
            | "that"
            | "understand"
            | "flow"
            | "code"
            | "file"
            | "module"
    )
}

fn normalize_seed_file(root: &Path, file: &str) -> Result<String> {
    let path = PathBuf::from(file);
    let absolute = if path.is_absolute() {
        path
    } else {
        root.join(path)
    };
    let canonical = absolute
        .canonicalize()
        .with_context(|| format!("failed to resolve seed file: {file}"))?;
    let relative = canonical
        .strip_prefix(root)
        .with_context(|| format!("seed file is outside project root: {file}"))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
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
