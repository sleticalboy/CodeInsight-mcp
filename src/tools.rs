use std::{
    cmp::{Ordering, Reverse},
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    config::{
        ConfiguredSuggestedCheck, init_project_config, load_project_config, project_config_path,
        suggested_test_commands_for_root,
    },
    embedding, index,
    language::detect_language,
    model::{
        AgentRouteBackendAgreement, AgentRouteBackendCandidate,
        AgentRouteBackendCandidateContinuation, AgentRouteBackendCandidateDisposition,
        AgentRouteBackendEvidence, AgentRouteBackendNormalization, AgentRouteExecutionStep,
        AgentRouteQuality, AgentRouteReport, AgentRouteRoutingDecision, AgentRouteStep, CallEdge,
        ConfigInitReport, ConfigStatusReport, ContextBudget, ContextContinuationSummary,
        ContextFile, ContextOmittedCandidate, ContextPack, ContextRange, ContextReadLess,
        ContextReadingRange, ContextReadingStep, ContextSeed, ContextSeedLocation,
        ContextSemanticStatus, ContextSourceCount, ContextSuggestedTool, Dependency,
        DependencyGraph, EmbeddingProviderStatus, ImpactAnalysisReport, ImpactBreakdown,
        ImpactCounts, ImpactFile, ImpactPath, IndexError, IndexScopeReport, Language,
        OllamaEmbeddingStatus, OpenAiEmbeddingStatus, ProjectIndexReport, ProjectOverview,
        ReferenceMatch, SemanticChunk, SemanticChunkInput, SemanticEmbeddingInput,
        SemanticEmbeddingMatch, SemanticIndexReport, SemanticIndexStatus, SemanticSearchResult,
        SuggestedCheck, Symbol, SymbolKind, VersionInfo,
    },
    storage::Store,
};

const CONTEXT_SCORE_SEED_FILE: i32 = 130;
const BACKEND_EVIDENCE_CANDIDATE_LIMIT: usize = 16;
const BACKEND_EVIDENCE_CANDIDATE_LOCATION_LIMIT: usize = 16;
const BACKEND_EVIDENCE_TOOL_ERROR_CHARS_LIMIT: usize = 256;
const BACKEND_EVIDENCE_TOOL_RESULT_WRAPPER_LIMIT: usize = 4;
const BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT: usize = 16;
const BACKEND_EVIDENCE_TOOL_RESULT_ITEMS_LIMIT: usize = 64;
const BACKEND_EVIDENCE_PER_CANDIDATE_LIMIT: usize = 6;
const BACKEND_EVIDENCE_TOTAL_CANDIDATE_ITEMS_LIMIT: usize = 24;
const BACKEND_EVIDENCE_SOURCES_LIMIT: usize = 12;
const BACKEND_EVIDENCE_NOTES_LIMIT: usize = 6;
const BACKEND_EVIDENCE_PROVIDER_CHARS_LIMIT: usize = 128;
const BACKEND_EVIDENCE_FILE_CHARS_LIMIT: usize = 512;
const BACKEND_EVIDENCE_SYMBOL_CHARS_LIMIT: usize = 160;
const BACKEND_EVIDENCE_SOURCE_CHARS_LIMIT: usize = 160;
const BACKEND_EVIDENCE_REASON_CHARS_LIMIT: usize = 320;
const BACKEND_EVIDENCE_ITEM_CHARS_LIMIT: usize = 160;
const BACKEND_EVIDENCE_NOTE_CHARS_LIMIT: usize = 320;

#[derive(Clone, Copy)]
enum BackendContextMode {
    Fallback,
    Preferred,
}

impl BackendContextMode {
    fn seed_source(self) -> &'static str {
        match self {
            Self::Fallback => "backend_fallback",
            Self::Preferred => "backend_preferred",
        }
    }
}

struct BackendContextSelection {
    candidates: Vec<AgentRouteBackendCandidate>,
    candidate_dispositions: Vec<AgentRouteBackendCandidateDisposition>,
}

struct BackendContextAttempt {
    context_pack: Option<ContextPack>,
    selection: BackendContextSelection,
}

struct BackendToolResultSpec<'a> {
    source: &'a str,
    items_keys: &'a [&'a str],
    preferred_items_key: Option<&'a str>,
    total_keys: &'a [&'a str],
    total_items_keys: &'a [&'a str],
    file_keys: &'a [&'a str],
    symbol_keys: &'a [&'a str],
}

struct BackendToolCandidateBatch {
    candidates: Vec<AgentRouteBackendCandidate>,
    source: Option<String>,
    evidence_count: usize,
    unfetched_items: usize,
    omitted_items: usize,
    latency_ms: u64,
}

impl BackendContextSelection {
    fn files(&self) -> Vec<String> {
        self.candidates
            .iter()
            .map(|candidate| candidate.file.clone())
            .collect()
    }

    fn symbols(&self) -> Vec<String> {
        self.candidates
            .iter()
            .filter_map(|candidate| candidate.symbol.clone())
            .collect()
    }

    fn dispositions(&self) -> Vec<AgentRouteBackendCandidateDisposition> {
        self.candidate_dispositions.clone()
    }
}

fn backend_candidate_context_seed_files(candidates: &[AgentRouteBackendCandidate]) -> Vec<String> {
    candidates
        .iter()
        .flat_map(|candidate| {
            if candidate.locations.is_empty() {
                return vec![candidate.file.clone()];
            }
            candidate
                .locations
                .iter()
                .map(|location| {
                    if location.start_line == location.end_line {
                        format!("{}#L{}", candidate.file, location.start_line)
                    } else {
                        format!(
                            "{}#L{}-L{}",
                            candidate.file, location.start_line, location.end_line
                        )
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

const CONTEXT_SCORE_SEED_HEADER: i32 = 140;
const CONTEXT_SCORE_TASK_LOCATION: i32 = 240;
const CONTEXT_SCORE_TASK_LOCATION_CONTEXT: i32 = 220;
const CONTEXT_SCORE_SYMBOL_DEFINITION: i32 = 90;
const CONTEXT_SCORE_TYPE_RELATION: i32 = 82;
const CONTEXT_SCORE_CALL_GRAPH: i32 = 75;
const CONTEXT_SCORE_REFERENCE_BASE: i32 = 60;
const CONTEXT_SCORE_SEMANTIC_CHUNK: i32 = 50;
const CONTEXT_SCORE_SEMANTIC_VECTOR: i32 = 70;
const CONTEXT_SCORE_LOCAL_DEPENDENCY: i32 = 40;
const CONTEXT_SCORE_TASK_MATCH_BOOST: i32 = 30;
const CONTEXT_SCORE_SEED_SYMBOL_TASK_MATCH_BOOST: i32 = 5;
const CONTEXT_SCORE_LOW_VALUE_FILE_PENALTY: i32 = 35;
const CONTEXT_PACK_NO_SEED_ERROR: &str = "context_pack could not infer source seed files from the current index; run index or provide --symbol/--file";
const CONTEXT_SCORE_LOW_VALUE_FILE_TEST_BOOST: i32 = 35;
const CONTEXT_MAX_SYMBOL_LINES: usize = 80;
const CONTEXT_MAX_MERGED_RANGE_LINES: usize = 80;
const CONTEXT_RANGE_REASON_MAX_BYTES: usize = 1200;
const CONTEXT_RANGE_REASON_OMITTED: &str = "additional matching signals omitted for brevity";
const CONTEXT_OMITTED_CANDIDATE_LIMIT: usize = 8;
const CONTEXT_OMITTED_CANDIDATE_RANGE_LIMIT: usize = 4;
const CONTEXT_TYPE_RELATION_DEPENDENCY_LIMIT: usize = 80;
const AUTO_SEED_TEXT_SCAN_LINES: usize = 160;

const IMPACT_SCORE_SEED_FILE: i32 = 100;
const IMPACT_SCORE_SYMBOL_DEFINITION: i32 = 90;
const IMPACT_SCORE_SEED_FILE_SYMBOL: i32 = 80;
const IMPACT_SCORE_REFERENCE: i32 = 40;
const IMPACT_SCORE_CALLER: i32 = 70;
const IMPACT_SCORE_CALLEE_SOURCE: i32 = 45;
const IMPACT_SCORE_CALLEE_TARGET: i32 = 65;
const IMPACT_SCORE_DEPENDENCY_SOURCE: i32 = 55;
const IMPACT_SCORE_DEPENDENCY_TARGET: i32 = 60;
const IMPACT_SCORE_TYPE_RELATION_SOURCE: i32 = 68;
const IMPACT_SCORE_CALLER_DEPTH_BASE: i32 = 70;
const IMPACT_SCORE_CALLER_DEPTH_DECAY: i32 = 15;
const IMPACT_SCORE_DEPENDENCY_DEPTH_BASE: i32 = 60;
const IMPACT_SCORE_DEPENDENCY_DEPTH_DECAY: i32 = 10;
const IMPACT_SCORE_DEPTH_FLOOR: i32 = 20;
const IMPACT_RISK_HIGH_FILE_COUNT: usize = 10;
const IMPACT_RISK_HIGH_SCORE: i32 = 300;
const IMPACT_RISK_HIGH_DEPTH: usize = 3;
const IMPACT_RISK_MEDIUM_FILE_COUNT: usize = 4;
const IMPACT_RISK_MEDIUM_SCORE: i32 = 160;
const IMPACT_RISK_MEDIUM_DEPTH: usize = 2;
const IMPACT_FILE_SYMBOL_SCAN_PER_FILE: usize = 256;
const IMPACT_FILE_SYMBOL_SCAN_MAX: usize = 4096;

pub fn index_project(root: PathBuf, force: bool) -> Result<()> {
    let report = index_project_value(root, force)?;
    print_json(&report)
}

pub fn init_config(root: PathBuf, force: bool) -> Result<()> {
    let report = init_config_value(root, force)?;
    print_json(&report)
}

pub fn config_status(root: PathBuf) -> Result<()> {
    let report = config_status_value(root)?;
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

pub fn dependency_graph(
    root: PathBuf,
    files: Vec<String>,
    languages: Vec<String>,
    kinds: Vec<String>,
    limit: usize,
    offset: usize,
) -> Result<()> {
    let graph = dependency_graph_value(root, files, languages, kinds, limit, offset)?;
    print_json(&graph)
}

pub fn impact_analysis(
    root: PathBuf,
    symbols: Vec<String>,
    files: Vec<String>,
    limit: usize,
    depth: usize,
    format: String,
    evidence_limit: usize,
) -> Result<()> {
    let report = impact_analysis_value(root, symbols, files, limit, depth, format, evidence_limit)?;
    print_json(&report)
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

pub fn semantic_search(root: PathBuf, query: String, limit: usize) -> Result<()> {
    let results = semantic_search_value(root, &query, limit)?;
    print_json(&results)
}

pub fn semantic_index(root: PathBuf, chunk_lines: usize, explain: bool) -> Result<()> {
    let report = semantic_index_value(root, chunk_lines, explain)?;
    print_json(&report)
}

pub fn embedding_status(root: Option<PathBuf>) -> Result<()> {
    let status = embedding_status_value(root)?;
    print_json(&status)
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

pub fn agent_route(
    root: PathBuf,
    task: String,
    symbols: Vec<String>,
    files: Vec<String>,
    token_budget: usize,
    force_index: bool,
    impact_limit: usize,
    impact_depth: usize,
    impact_evidence_limit: usize,
    include_impact: bool,
    compact: bool,
    response_token_budget: Option<usize>,
    backend_evidence: Option<AgentRouteBackendEvidence>,
) -> Result<()> {
    let report = agent_route_value(
        root,
        task,
        symbols,
        files,
        token_budget,
        force_index,
        impact_limit,
        impact_depth,
        impact_evidence_limit,
        include_impact,
        backend_evidence,
    )?;
    print_json(&agent_route_response_value(
        &report,
        compact,
        response_token_budget,
    )?)
}

pub fn agent_route_response_value(
    report: &AgentRouteReport,
    compact: bool,
    response_token_budget: Option<usize>,
) -> Result<Value> {
    if response_token_budget.is_some() && !compact {
        bail!("response_token_budget requires compact response mode");
    }
    if response_token_budget.is_some_and(|budget| budget < 500) {
        bail!("response_token_budget must be >= 500");
    }

    let mut value = serde_json::to_value(report)?;
    if !compact {
        return Ok(value);
    }

    let Some(route) = value.as_object_mut() else {
        return Ok(value);
    };
    route.insert("response_mode".to_string(), json!("compact"));
    route.remove("route");
    route.remove("index_report");
    route.remove("overview");

    if let Some(routing_decision) = route
        .get_mut("routing_decision")
        .and_then(Value::as_object_mut)
    {
        routing_decision.remove("backend_evidence");
    }

    if let Some(context_pack) = route.get_mut("context_pack").and_then(Value::as_object_mut) {
        for key in [
            "task",
            "seed_strategy",
            "semantic_status",
            "omitted_candidates",
            "symbols",
            "references",
            "estimated_tokens",
            "truncated",
        ] {
            context_pack.remove(key);
        }
        if let Some(files) = context_pack.get_mut("files").and_then(Value::as_array_mut) {
            for file in files {
                let Some(file) = file.as_object_mut() else {
                    continue;
                };
                file.retain(|key, _| matches!(key.as_str(), "file" | "selection_rank" | "ranges"));
                if let Some(ranges) = file.get_mut("ranges").and_then(Value::as_array_mut) {
                    for range in ranges {
                        if let Some(range) = range.as_object_mut() {
                            range.retain(|key, _| {
                                matches!(key.as_str(), "start_line" | "end_line" | "excerpt")
                            });
                        }
                    }
                }
            }
        }
    }

    if let Some(impact_analysis) = route
        .get_mut("impact_analysis")
        .and_then(Value::as_object_mut)
    {
        for key in [
            "root",
            "depth",
            "format",
            "evidence_limit",
            "seed_symbols",
            "seed_files",
            "paths",
            "symbols",
            "references",
            "callers",
            "callees",
            "dependencies",
            "errors",
        ] {
            impact_analysis.remove(key);
        }
    }

    if let Some(response_token_budget) = response_token_budget {
        apply_compact_response_budget(&mut value, response_token_budget)?;
    }

    Ok(value)
}

fn apply_compact_response_budget(value: &mut Value, requested_tokens: usize) -> Result<()> {
    let Some(route) = value.as_object_mut() else {
        return Ok(());
    };
    route.insert(
        "response_budget".to_string(),
        json!({
            "requested_tokens": requested_tokens,
            "estimated_tokens": 0,
            "truncated": false,
            "omitted_excerpts": 0,
            "estimator": "utf8_bytes_div_4"
        }),
    );

    let mut omitted_excerpts = 0;
    loop {
        let estimated_tokens = refresh_response_token_estimate(value)?;
        if estimated_tokens <= requested_tokens {
            return Ok(());
        }

        if !remove_last_non_requested_compact_excerpt(value) {
            bail!(
                "response_token_budget {requested_tokens} is too small; compact route contract requires at least {estimated_tokens} estimated tokens"
            );
        }
        omitted_excerpts += 1;
        if let Some(response_budget) = value
            .get_mut("response_budget")
            .and_then(Value::as_object_mut)
        {
            response_budget.insert("truncated".to_string(), json!(true));
            response_budget.insert("omitted_excerpts".to_string(), json!(omitted_excerpts));
        }
    }
}

fn refresh_response_token_estimate(value: &mut Value) -> Result<usize> {
    let mut previous = usize::MAX;
    for _ in 0..4 {
        let estimated_tokens = estimate_tokens(&serde_json::to_string(value)?);
        if let Some(response_budget) = value
            .get_mut("response_budget")
            .and_then(Value::as_object_mut)
        {
            response_budget.insert("estimated_tokens".to_string(), json!(estimated_tokens));
        }
        if estimated_tokens == previous {
            return Ok(estimated_tokens);
        }
        previous = estimated_tokens;
    }

    Ok(estimate_tokens(&serde_json::to_string(value)?))
}

fn compact_requested_locations(value: &Value) -> BTreeMap<String, Vec<(u64, u64)>> {
    let mut requested_locations = BTreeMap::<String, Vec<(u64, u64)>>::new();
    let Some(seeds) = value
        .pointer("/context_pack/selected_seeds")
        .and_then(Value::as_array)
    else {
        return requested_locations;
    };

    for seed in seeds {
        if seed.get("kind").and_then(Value::as_str) != Some("file") {
            continue;
        }
        let Some(file) = seed.get("value").and_then(Value::as_str) else {
            continue;
        };
        let Some(locations) = seed.get("locations").and_then(Value::as_array) else {
            continue;
        };
        for location in locations {
            let Some(start_line) = location.get("start_line").and_then(Value::as_u64) else {
                continue;
            };
            let end_line = location
                .get("end_line")
                .and_then(Value::as_u64)
                .unwrap_or(start_line);
            requested_locations
                .entry(file.to_string())
                .or_default()
                .push((start_line, end_line));
        }
    }

    requested_locations
}

fn compact_range_overlaps_requested_location(
    file: &str,
    range: &Value,
    requested_locations: &BTreeMap<String, Vec<(u64, u64)>>,
) -> bool {
    let Some(start_line) = range.get("start_line").and_then(Value::as_u64) else {
        return false;
    };
    let end_line = range
        .get("end_line")
        .and_then(Value::as_u64)
        .unwrap_or(start_line);
    requested_locations.get(file).is_some_and(|locations| {
        locations.iter().any(|(requested_start, requested_end)| {
            start_line <= *requested_end && end_line >= *requested_start
        })
    })
}

fn remove_last_non_requested_compact_excerpt(value: &mut Value) -> bool {
    let requested_locations = compact_requested_locations(value);
    let Some(files) = value
        .pointer_mut("/context_pack/files")
        .and_then(Value::as_array_mut)
    else {
        return false;
    };

    for file in files.iter_mut().rev() {
        let file_name = file
            .get("file")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let Some(ranges) = file.get_mut("ranges").and_then(Value::as_array_mut) else {
            continue;
        };
        for range in ranges.iter_mut().rev() {
            if compact_range_overlaps_requested_location(&file_name, range, &requested_locations) {
                continue;
            }
            if range
                .as_object_mut()
                .is_some_and(|range| range.remove("excerpt").is_some())
            {
                return true;
            }
        }
    }
    false
}

pub fn read_agent_route_backend_evidence(path: &Path) -> Result<AgentRouteBackendEvidence> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read backend evidence file: {}", path.display()))?;
    serde_json::from_str(&contents)
        .with_context(|| format!("failed to parse backend evidence file: {}", path.display()))
}

pub fn callers(root: PathBuf, symbol: String, limit: usize) -> Result<()> {
    let calls = callers_value(root, &symbol, limit)?;
    print_json(&calls)
}

pub fn callees(root: PathBuf, symbol: String, limit: usize) -> Result<()> {
    let calls = callees_value(root, &symbol, limit)?;
    print_json(&calls)
}

pub fn version() -> Result<()> {
    print_json(&version_value())
}

pub fn version_value() -> VersionInfo {
    VersionInfo {
        name: env!("CARGO_PKG_NAME").to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        target_arch: std::env::consts::ARCH.to_string(),
        target_os: std::env::consts::OS.to_string(),
    }
}

pub fn index_project_value(root: PathBuf, force: bool) -> Result<ProjectIndexReport> {
    index::index_project(&root, force)
}

pub fn agent_route_value(
    root: PathBuf,
    task: String,
    symbols: Vec<String>,
    files: Vec<String>,
    token_budget: usize,
    force_index: bool,
    impact_limit: usize,
    impact_depth: usize,
    impact_evidence_limit: usize,
    include_impact: bool,
    backend_evidence: Option<AgentRouteBackendEvidence>,
) -> Result<AgentRouteReport> {
    let root = root.canonicalize()?;
    let index_report = index_project_value(root.clone(), force_index)?;
    let backend_evidence = backend_evidence
        .map(|evidence| normalize_agent_route_backend_evidence(&root, evidence))
        .transpose()?;
    let overview = project_overview_value(root.clone())?;
    let mut context_pack = match context_pack_value(
        root.clone(),
        task.clone(),
        symbols.clone(),
        files.clone(),
        token_budget,
    ) {
        Ok(context_pack) => context_pack,
        Err(error) if is_context_pack_no_seed_error(&error) => {
            empty_context_pack_for_blocked_route(task.clone(), token_budget, overview.total_lines)
        }
        Err(error) if is_context_pack_invalid_seed_error(&error) => {
            empty_context_pack_for_invalid_seed_route(
                task.clone(),
                token_budget,
                overview.total_lines,
                &files,
                error.to_string(),
            )
        }
        Err(error) => return Err(error),
    };
    let local_first_file = context_pack
        .reading_plan
        .first()
        .map(|step| step.file.clone());
    let has_explicit_seed = !files.is_empty() || !symbols.is_empty();
    let mut backend_context_mode = None;
    let mut backend_context_selection = None;
    if !has_explicit_seed
        && let Some(evidence) = backend_evidence
            .as_ref()
            .filter(|evidence| evidence.prefer_for_context)
    {
        let attempt = backend_seed_context_pack(
            &root,
            &task,
            token_budget,
            evidence,
            BackendContextMode::Preferred,
        )?;
        if let Some(preferred_context) = attempt.context_pack {
            context_pack = preferred_context;
            backend_context_mode = Some(BackendContextMode::Preferred);
        }
        backend_context_selection = Some(attempt.selection);
    } else if context_pack.reading_plan.is_empty()
        && let Some(evidence) = backend_evidence
            .as_ref()
            .filter(|evidence| evidence.use_as_fallback)
    {
        let attempt = backend_seed_context_pack(
            &root,
            &task,
            token_budget,
            evidence,
            BackendContextMode::Fallback,
        )?;
        if let Some(fallback_context) = attempt.context_pack {
            context_pack = fallback_context;
            backend_context_mode = Some(BackendContextMode::Fallback);
        }
        backend_context_selection = Some(attempt.selection);
    }
    add_index_scope_hint_to_blocked_context(&mut context_pack, &index_report.index_scope);

    let blocked_context_status = context_pack
        .reading_plan
        .is_empty()
        .then(|| context_pack.continuation_summary.status.clone());
    let mut impact_seed_files = if blocked_context_status.is_some() {
        Vec::new()
    } else if backend_context_mode.is_some() {
        backend_context_selection
            .as_ref()
            .map(BackendContextSelection::files)
            .unwrap_or_default()
    } else {
        files
            .iter()
            .map(|file| normalize_seed_file(&root, file))
            .collect::<Result<Vec<_>>>()?
    };
    if impact_seed_files.is_empty()
        && let Some(first_file) = context_pack.files.first()
    {
        impact_seed_files.push(first_file.file.clone());
    }
    impact_seed_files.sort();
    impact_seed_files.dedup();

    let mut impact_seed_symbols = if blocked_context_status.is_some() {
        Vec::new()
    } else if backend_context_mode.is_some() {
        backend_context_selection
            .as_ref()
            .map(BackendContextSelection::symbols)
            .unwrap_or_default()
    } else {
        symbols
    };
    if impact_seed_symbols.is_empty() {
        impact_seed_symbols.extend(
            context_pack
                .selected_seeds
                .iter()
                .filter(|seed| seed.source == "task_match")
                .flat_map(|seed| seed.matched_symbols.iter().cloned())
                .take(3),
        );
    }
    impact_seed_symbols.sort();
    impact_seed_symbols.dedup();

    let (impact_status, impact_analysis) = if let Some(status) = blocked_context_status.as_deref() {
        (agent_route_skipped_impact_status(status).to_string(), None)
    } else if impact_seed_files.is_empty() && impact_seed_symbols.is_empty() {
        ("skipped_no_seed".to_string(), None)
    } else if !include_impact {
        ("deferred_by_request".to_string(), None)
    } else {
        let report = impact_analysis_value(
            root.clone(),
            impact_seed_symbols.clone(),
            impact_seed_files.clone(),
            impact_limit,
            impact_depth,
            "summary".to_string(),
            impact_evidence_limit,
        )?;
        ("complete".to_string(), Some(report))
    };

    let route = vec![
        AgentRouteStep {
            order: 1,
            tool: "index_project".to_string(),
            status: "complete".to_string(),
            reason: if force_index {
                "refreshed local repository index with force_index enabled".to_string()
            } else {
                "refreshed local repository index and reused unchanged files".to_string()
            },
        },
        AgentRouteStep {
            order: 2,
            tool: "project_overview".to_string(),
            status: "complete".to_string(),
            reason: format!(
                "found {} entrypoints, {} type-relation edges, and {} recommended next tools",
                overview.entrypoints.len(),
                overview.dependency_summary.type_relation_edges,
                overview.recommended_next_tools.len()
            ),
        },
        AgentRouteStep {
            order: 3,
            tool: "context_pack".to_string(),
            status: agent_route_context_status(&context_pack),
            reason: agent_route_context_reason(&context_pack),
        },
        AgentRouteStep {
            order: 4,
            tool: "impact_analysis".to_string(),
            status: impact_status.clone(),
            reason: match &impact_analysis {
                Some(report) => agent_route_impact_reason(report),
                None => agent_route_skipped_impact_reason(&impact_status),
            },
        },
    ];
    let current_reading_step = context_pack.reading_plan.first().cloned();
    let deferred_impact_tool = (impact_status == "deferred_by_request").then(|| {
        agent_route_deferred_impact_suggested_tool(
            &root,
            &impact_seed_symbols,
            &impact_seed_files,
            impact_limit,
            impact_depth,
            impact_evidence_limit,
        )
    });
    let routing_decision = agent_route_routing_decision(
        &root,
        &context_pack,
        &impact_status,
        backend_evidence,
        local_first_file.as_deref(),
        backend_context_mode,
        backend_context_selection.as_ref(),
    );
    let execution_plan = agent_route_execution_plan(
        &context_pack,
        &impact_status,
        impact_analysis.as_ref(),
        deferred_impact_tool,
        &impact_seed_files,
        &routing_decision.backend_route_agreement,
        routing_decision.backend_selected_candidate.as_ref(),
    );

    Ok(AgentRouteReport {
        root: root.display().to_string(),
        task,
        token_budget,
        backend_status: None,
        routing_decision,
        route,
        execution_plan,
        current_reading_step,
        impact_seed_files,
        impact_seed_symbols,
        impact_status,
        index_report,
        overview,
        context_pack,
        impact_analysis,
    })
}

fn agent_route_routing_decision(
    root: &Path,
    context_pack: &ContextPack,
    impact_status: &str,
    backend_evidence: Option<AgentRouteBackendEvidence>,
    local_first_file: Option<&str>,
    backend_context_mode: Option<BackendContextMode>,
    backend_context_selection: Option<&BackendContextSelection>,
) -> AgentRouteRoutingDecision {
    let first_seed = context_pack.selected_seeds.first();
    let first_step = context_pack.reading_plan.first();
    let candidate_dispositions = backend_context_selection
        .map(BackendContextSelection::dispositions)
        .unwrap_or_default();
    let next_candidate_continuation = backend_candidate_continuation(
        root,
        &context_pack.task,
        context_pack.budget.applied_token_budget,
        &candidate_dispositions,
    );
    let backend_route_agreement = agent_route_backend_agreement(
        local_first_file,
        backend_evidence.as_ref(),
        backend_context_mode,
        backend_context_selection
            .map(BackendContextSelection::files)
            .unwrap_or_default(),
        candidate_dispositions,
        next_candidate_continuation,
    );
    let route_quality = agent_route_quality(
        context_pack,
        impact_status,
        backend_evidence.as_ref(),
        &backend_route_agreement,
    );
    let (continuation_source, continuation_status, continuation_next_action) =
        match backend_route_agreement.next_candidate_continuation.as_ref() {
            Some(continuation) => (
                "backend_route_agreement".to_string(),
                "backend_candidate_available".to_string(),
                continuation.next_action.clone(),
            ),
            None => (
                "context_pack".to_string(),
                context_pack.continuation_summary.status.clone(),
                context_pack.continuation_summary.next_action.clone(),
            ),
        };
    let backend_selected_candidate = first_step.and_then(|step| {
        backend_evidence.as_ref().and_then(|backend| {
            backend
                .candidates
                .iter()
                .find(|candidate| candidate.file == step.file)
                .cloned()
        })
    });

    AgentRouteRoutingDecision {
        seed_strategy: context_pack.seed_strategy.clone(),
        route_quality,
        backend_route_agreement,
        backend_evidence,
        backend_selected_candidate,
        first_seed_kind: first_seed.map(|seed| seed.kind.clone()),
        first_seed_source: first_seed.map(|seed| seed.source.clone()),
        first_seed_value: first_seed.map(|seed| seed.value.clone()),
        first_seed_role: first_seed.and_then(|seed| seed.role.clone()),
        first_seed_matched_keywords: first_seed
            .map(|seed| seed.matched_keywords.clone())
            .unwrap_or_default(),
        first_seed_matched_symbols: first_seed
            .map(|seed| seed.matched_symbols.clone())
            .unwrap_or_default(),
        first_file: first_step.map(|step| step.file.clone()),
        first_selection_rank: first_step.map(|step| step.selection_rank),
        first_focus: first_step.map(|step| step.focus.clone()),
        first_question: first_step.map(|step| step.question.clone()),
        first_next_action: first_step.map(|step| step.next_action.clone()),
        first_selection_reason: first_step.map(|step| step.selection_reason.clone()),
        first_suggested_tool: first_step.map(|step| step.suggested_tool.clone()),
        selected_file_count: context_pack.files.len(),
        selected_range_count: context_pack.budget.selected_ranges,
        omitted_file_count: context_pack.budget.omitted_files,
        baseline_source_lines: context_pack.read_less.baseline_source_lines,
        selected_source_lines: context_pack.read_less.selected_source_lines,
        source_lines_avoided: context_pack.read_less.source_lines_avoided,
        line_reduction: context_pack.read_less.line_reduction.clone(),
        read_less_ratio: context_pack.read_less.read_less_ratio.clone(),
        continuation_source,
        continuation_status,
        continuation_next_action,
        impact_status: impact_status.to_string(),
    }
}

fn agent_route_backend_agreement(
    local_first_file: Option<&str>,
    backend_evidence: Option<&AgentRouteBackendEvidence>,
    backend_context_mode: Option<BackendContextMode>,
    selected_context_files: Vec<String>,
    candidate_dispositions: Vec<AgentRouteBackendCandidateDisposition>,
    next_candidate_continuation: Option<AgentRouteBackendCandidateContinuation>,
) -> AgentRouteBackendAgreement {
    let Some(backend) = backend_evidence else {
        return AgentRouteBackendAgreement {
            status: "no_backend".to_string(),
            message: "No external graph backend evidence was provided.".to_string(),
            recommended_action: "read_selected_context".to_string(),
            provider: None,
            local_first_file: local_first_file.map(str::to_string),
            backend_first_file: None,
            selected_context_file: None,
            selected_context_files: Vec::new(),
            candidate_file_count: 0,
            candidate_dispositions: Vec::new(),
            next_candidate_continuation: None,
            common_files: Vec::new(),
        };
    };

    let backend_first_file = backend.candidate_files.first().cloned();
    let local_first_file = local_first_file.map(str::to_string);
    let local_first_file_ref = local_first_file.as_deref();
    let common_files = local_first_file_ref
        .map(|local| {
            backend
                .candidate_files
                .iter()
                .filter(|candidate| candidate.as_str() == local)
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if !candidate_dispositions.is_empty()
        && candidate_dispositions.iter().all(|candidate| {
            matches!(
                candidate.context_reason.as_str(),
                "missing_file" | "unindexed_file"
            )
        })
    {
        let recommended_action = if local_first_file.is_some() {
            "read_selected_context"
        } else {
            "provide_valid_backend_candidate"
        };
        let message = if candidate_dispositions
            .iter()
            .all(|candidate| candidate.context_reason == "missing_file")
        {
            format!(
                "Backend {} supplied {} candidate file(s), but none exist in the current local checkout.",
                backend.provider,
                candidate_dispositions.len()
            )
        } else {
            format!(
                "Backend {} supplied {} candidate file(s), but none are usable from the current local source index.",
                backend.provider,
                candidate_dispositions.len()
            )
        };
        return AgentRouteBackendAgreement {
            status: "backend_unavailable".to_string(),
            message,
            recommended_action: recommended_action.to_string(),
            provider: Some(backend.provider.clone()),
            local_first_file,
            backend_first_file,
            selected_context_file: None,
            selected_context_files: Vec::new(),
            candidate_file_count: backend.candidate_files.len(),
            candidate_dispositions,
            next_candidate_continuation,
            common_files,
        };
    }

    if let Some(mode) = backend_context_mode {
        let selected_context_file = selected_context_files.first().cloned();
        let selected_context_count = selected_context_files.len();
        let (status, message) = match mode {
            BackendContextMode::Fallback => (
                "backend_fallback",
                format!(
                    "Local routing was blocked, so backend {} fallback evidence seeded bounded context with {} ranked file(s), starting with {}.",
                    backend.provider,
                    selected_context_count,
                    selected_context_file.as_deref().unwrap_or("unknown")
                ),
            ),
            BackendContextMode::Preferred => {
                let message = match (local_first_file_ref, selected_context_file.as_deref()) {
                    (Some(local), Some(selected)) if local == selected => format!(
                        "Explicit backend preference confirmed local route {} and seeded bounded context from backend {}.",
                        selected, backend.provider
                    ),
                    (Some(local), Some(selected)) => format!(
                        "Explicit backend preference selected {} from backend {} instead of local route {}.",
                        selected, backend.provider, local
                    ),
                    (None, Some(selected)) => format!(
                        "Explicit backend preference seeded bounded context with {} from backend {} because local routing produced no first-read file.",
                        selected, backend.provider
                    ),
                    (_, None) => format!(
                        "Explicit backend preference was requested from backend {}, but no bounded context file was selected.",
                        backend.provider
                    ),
                };
                ("backend_preferred", message)
            }
        };
        return AgentRouteBackendAgreement {
            status: status.to_string(),
            message,
            recommended_action: "read_backend_seeded_context".to_string(),
            provider: Some(backend.provider.clone()),
            local_first_file,
            backend_first_file,
            selected_context_file,
            selected_context_files,
            candidate_file_count: backend.candidate_files.len(),
            candidate_dispositions,
            next_candidate_continuation,
            common_files,
        };
    }

    let (status, recommended_action, message) = match (
        local_first_file_ref,
        backend_first_file.as_deref(),
        common_files.is_empty(),
    ) {
        (None, Some(backend_first), _) => (
            "backend_only",
            "provide_seed_or_use_backend_candidate",
            format!(
                "Local routing produced no first-read file, but backend {} suggested {}.",
                backend.provider, backend_first
            ),
        ),
        (None, None, _) => (
            "no_local_route",
            "provide_seed_file_or_symbol",
            format!(
                "Local routing produced no first-read file and backend {} supplied no candidate files.",
                backend.provider
            ),
        ),
        (Some(local), None, _) => (
            "backend_without_candidates",
            "read_selected_context",
            format!(
                "Local routing selected {}, but backend {} supplied no candidate files.",
                local, backend.provider
            ),
        ),
        (Some(local), Some(backend_first), false) if local == backend_first => (
            "agree",
            "read_selected_context",
            format!(
                "Backend {} and local routing agree on first-read file {}.",
                backend.provider, local
            ),
        ),
        (Some(local), Some(backend_first), false) => (
            "overlap",
            "read_selected_context_then_compare_backend_rank",
            format!(
                "Local routing selected {}, and backend {} included it after preferring {}.",
                local, backend.provider, backend_first
            ),
        ),
        (Some(local), Some(backend_first), true) => (
            "conflict",
            "compare_backend_route_before_edits",
            format!(
                "Local routing selected {}, but backend {} preferred {}.",
                local, backend.provider, backend_first
            ),
        ),
    };

    AgentRouteBackendAgreement {
        status: status.to_string(),
        message,
        recommended_action: recommended_action.to_string(),
        provider: Some(backend.provider.clone()),
        local_first_file,
        backend_first_file,
        selected_context_file: None,
        selected_context_files: Vec::new(),
        candidate_file_count: backend.candidate_files.len(),
        candidate_dispositions,
        next_candidate_continuation,
        common_files,
    }
}

fn backend_candidate_continuation(
    root: &Path,
    task: &str,
    token_budget: usize,
    candidate_dispositions: &[AgentRouteBackendCandidateDisposition],
) -> Option<AgentRouteBackendCandidateContinuation> {
    let candidate = candidate_dispositions.iter().find(|candidate| {
        matches!(
            candidate.context_reason.as_str(),
            "token_budget_exhausted" | "fallback_not_selected"
        )
    })?;
    let mut suggested_arguments = json!({
        "root": root.display().to_string(),
        "task": task,
        "files": [candidate.file.clone()],
        "token_budget": token_budget.max(4000)
    });
    if candidate.symbol_status.as_deref() == Some("valid")
        && let Some(symbol) = candidate.symbol.as_ref()
    {
        suggested_arguments["symbols"] = json!([symbol]);
    }

    Some(AgentRouteBackendCandidateContinuation {
        file: candidate.file.clone(),
        rank: candidate.rank,
        symbol: candidate
            .symbol_status
            .as_deref()
            .filter(|status| *status == "valid")
            .and(candidate.symbol.clone()),
        context_reason: candidate.context_reason.clone(),
        next_action: candidate.next_action.clone(),
        suggested_tool: ContextSuggestedTool {
            tool: "context_pack".to_string(),
            priority: 70,
            reason: "Build focused context around the highest-ranked valid backend candidate not yet selected."
                .to_string(),
            suggested_arguments,
        },
    })
}

fn agent_route_quality(
    context_pack: &ContextPack,
    impact_status: &str,
    backend_evidence: Option<&AgentRouteBackendEvidence>,
    backend_route_agreement: &AgentRouteBackendAgreement,
) -> AgentRouteQuality {
    let Some(first_step) = context_pack.reading_plan.first() else {
        let recommended_action =
            if backend_evidence.is_some() && backend_route_agreement.status != "no_backend" {
                backend_route_agreement.recommended_action.clone()
            } else {
                context_pack.continuation_summary.next_action.clone()
            };
        let mut verification_steps = vec![format!(
            "Follow {} and provide a concrete seed before editing.",
            context_pack.continuation_summary.next_action
        )];
        if backend_evidence.is_some() && backend_route_agreement.status != "no_backend" {
            verification_steps.push(backend_route_agreement.message.clone());
        }

        let mut warnings = vec![format!(
            "No reading plan was produced; context status is {}.",
            context_pack.continuation_summary.status
        )];
        if let Some(warning) = backend_evidence.and_then(backend_normalization_warning) {
            warnings.push(warning);
        }

        return AgentRouteQuality {
            level: "blocked".to_string(),
            score: 0,
            decision_summary: format!(
                "No first-read route was produced because context status is {}; ask for a seed file or symbol before broad reading.",
                context_pack.continuation_summary.status
            ),
            evidence_count: backend_evidence
                .map(backend_evidence_signal_count)
                .unwrap_or_default(),
            evidence_sources: backend_evidence
                .map(backend_evidence_sources)
                .unwrap_or_default(),
            confidence_factors: backend_evidence
                .map(|backend| {
                    vec![format!(
                        "backend {} supplied {} candidate file(s) but local routing was blocked",
                        backend.provider,
                        backend.candidate_files.len()
                    )]
                })
                .unwrap_or_default(),
            warnings,
            verification_steps,
            recommended_action,
        };
    };

    let first_seed = context_pack.selected_seeds.first();
    let source_count = first_step
        .source_mix
        .iter()
        .map(|source| source.count)
        .sum::<usize>();
    let seed_evidence_count = first_seed
        .map(|seed| seed.matched_keywords.len() + seed.matched_symbols.len())
        .unwrap_or_default();
    let mut evidence_count = source_count + seed_evidence_count + first_step.ranges.len();
    let mut evidence_sources = first_step
        .source_mix
        .iter()
        .map(|source| source.source.clone())
        .collect::<Vec<_>>();
    let local_evidence_sources_summary = if evidence_sources.is_empty() {
        "no source mix".to_string()
    } else {
        evidence_sources.join(", ")
    };

    let mut score: i32 = 50;
    let mut warnings = Vec::new();
    let mut confidence_factors = Vec::new();
    let mut verification_steps = vec![format!(
        "Read {} first and answer: {}",
        first_step.file, first_step.question
    )];
    let mut backend_route_recommended_action = None;

    if first_step.selection_rank == 1 {
        score += 15;
        confidence_factors.push("first selected file is candidate rank 1".to_string());
    } else if first_step.selection_rank <= 3 {
        score += 8;
        warnings.push(format!(
            "First selected file is candidate rank {}, not rank 1.",
            first_step.selection_rank
        ));
        confidence_factors.push(format!(
            "first selected file is a top-{} candidate",
            first_step.selection_rank
        ));
    } else {
        warnings.push(format!(
            "First selected file is candidate rank {}; verify before editing.",
            first_step.selection_rank
        ));
    }

    if first_seed.is_some() {
        score += 8;
        if let Some(seed) = first_seed {
            confidence_factors.push(format!(
                "seed evidence came from {} {}",
                seed.source, seed.value
            ));
        }
    }
    if seed_evidence_count > 0 {
        score += 8;
        confidence_factors.push(format!(
            "{} matched task keyword or symbol signals",
            seed_evidence_count
        ));
    }
    if first_step
        .source_mix
        .iter()
        .any(|source| matches!(source.source.as_str(), "seed file" | "symbol definition"))
    {
        score += 12;
        confidence_factors
            .push("selected context includes direct seed or symbol evidence".to_string());
    }
    if first_step.source_mix.iter().any(|source| {
        matches!(
            source.source.as_str(),
            "call graph" | "type relation" | "reference" | "dependency"
        )
    }) {
        score += 8;
        confidence_factors.push(format!(
            "selected context is supported by structural sources: {}",
            local_evidence_sources_summary
        ));
    }
    if first_step.source_mix.len() >= 2 {
        score += 5;
        confidence_factors.push(format!(
            "{} independent source groups support the first file",
            first_step.source_mix.len()
        ));
    }
    if !first_step.ranges.is_empty() {
        score += 5;
        confidence_factors.push(format!(
            "{} selected source ranges are available for the first read",
            first_step.ranges.len()
        ));
    }
    if impact_status == "complete" {
        score += 7;
        confidence_factors.push("pre-edit impact preview completed".to_string());
        verification_steps.push("Review impact_analysis before editing.".to_string());
    } else if impact_status == "deferred_by_request" {
        warnings.push(
            "Impact preview was deferred for the fast first read; run impact_analysis before editing."
                .to_string(),
        );
        verification_steps.push(
            "Call the execution plan's impact_analysis suggestion after reading selected context and before editing."
                .to_string(),
        );
    } else if impact_status.starts_with("skipped_") {
        warnings.push(format!(
            "Impact preview is {}; review impact before editing when a seed is available.",
            impact_status
        ));
        verification_steps.push(format!(
            "Resolve {} before treating impact scope as complete.",
            impact_status
        ));
    }
    if context_pack.continuation_summary.status == "complete" {
        score += 3;
        confidence_factors.push("no omitted continuation candidate is required".to_string());
    } else if context_pack.continuation_summary.status == "omitted_candidates_available" {
        warnings.push(
            "Lower-ranked candidates were omitted; use continuation after selected context."
                .to_string(),
        );
        verification_steps.push(format!(
            "If the first file is insufficient, run {}.",
            context_pack.continuation_summary.next_action
        ));
    } else if context_pack
        .continuation_summary
        .status
        .starts_with("blocked_")
    {
        warnings.push(format!(
            "Continuation status is {}; follow {} before broad reading.",
            context_pack.continuation_summary.status, context_pack.continuation_summary.next_action
        ));
        verification_steps.push(format!(
            "Follow continuation action {} before broad reading.",
            context_pack.continuation_summary.next_action
        ));
    }

    if context_pack.truncated {
        score -= 10;
        warnings.push(
            "Context was truncated by the token budget; continue with omitted candidates if the first read is insufficient."
                .to_string(),
        );
        verification_steps.push(
            "Treat the selected context as a starting point, not a complete repository proof."
                .to_string(),
        );
    }
    if evidence_count <= 1 {
        score -= 8;
        warnings.push("Only one evidence signal supported the first selected file.".to_string());
    }
    if let Some(backend) = backend_evidence {
        let backend_sources = backend_evidence_sources(backend);
        let backend_signal_count = backend_evidence_signal_count(backend);
        evidence_count += backend_signal_count;
        evidence_sources.extend(backend_sources.clone());
        evidence_sources.sort();
        evidence_sources.dedup();
        if backend_signal_count > 0 {
            score += 5;
            confidence_factors.push(format!(
                "backend {} supplied {} evidence signal(s)",
                backend.provider, backend_signal_count
            ));
        }
        match backend_route_agreement.status.as_str() {
            "agree" => {
                score += 10;
                confidence_factors.push(format!(
                    "backend {} independently selected the same first file",
                    backend.provider
                ));
            }
            "overlap" => {
                score += 4;
                backend_route_recommended_action =
                    Some(backend_route_agreement.recommended_action.clone());
                if let Some(first_backend_file) =
                    backend_route_agreement.backend_first_file.as_ref()
                {
                    warnings.push(format!(
                        "Backend {} ranked {} before local route {}; compare backend rank after reading selected context.",
                        backend.provider, first_backend_file, first_step.file
                    ));
                    verification_steps.push(format!(
                        "Read selected context, then compare backend {} rank-1 candidate {} with local route {}.",
                        backend.provider, first_backend_file, first_step.file
                    ));
                }
            }
            "conflict" => {
                backend_route_recommended_action =
                    Some(backend_route_agreement.recommended_action.clone());
                score -= 5;
                if let Some(first_backend_file) =
                    backend_route_agreement.backend_first_file.as_ref()
                {
                    warnings.push(format!(
                        "Backend {} preferred {}; verify before editing because local routing selected {}.",
                        backend.provider, first_backend_file, first_step.file
                    ));
                    verification_steps.push(format!(
                        "Compare local route with backend {} candidate {} before editing.",
                        backend.provider, first_backend_file
                    ));
                }
            }
            "backend_without_candidates" => {
                warnings.push(format!(
                    "Backend {} supplied no candidate files; treat local routing as uncorroborated.",
                    backend.provider
                ));
            }
            "backend_unavailable" => {
                score -= 3;
                backend_route_recommended_action =
                    Some(backend_route_agreement.recommended_action.clone());
                warnings.push(backend_route_agreement.message.clone());
                verification_steps.push(format!(
                    "Refresh backend {} evidence against the current checkout before relying on its candidate ranking.",
                    backend.provider
                ));
            }
            "backend_fallback" => {
                score += 3;
                backend_route_recommended_action =
                    Some(backend_route_agreement.recommended_action.clone());
                confidence_factors.push(format!(
                    "backend {} supplied the fallback seed for bounded local context",
                    backend.provider
                ));
                warnings.push(
                    "Local routing required a backend fallback seed; verify the selected context before editing."
                        .to_string(),
                );
            }
            "backend_preferred" => {
                score += 3;
                backend_route_recommended_action =
                    Some(backend_route_agreement.recommended_action.clone());
                confidence_factors.push(format!(
                    "explicit policy selected backend {} ranked candidates as bounded context seeds",
                    backend.provider
                ));
                if backend_route_agreement.local_first_file.as_deref()
                    != backend_route_agreement.selected_context_file.as_deref()
                {
                    warnings.push(
                        "Backend preference replaced the local first-read candidate; verify the selected backend context before editing."
                            .to_string(),
                    );
                    if let Some(local_file) = backend_route_agreement.local_first_file.as_ref() {
                        verification_steps.push(format!(
                            "After reading backend-seeded context, compare local candidate {} if the task remains unresolved.",
                            local_file
                        ));
                    }
                }
            }
            "backend_only" | "no_local_route" => {
                backend_route_recommended_action =
                    Some(backend_route_agreement.recommended_action.clone());
                warnings.push(backend_route_agreement.message.clone());
            }
            _ => {}
        }
        if let Some(confidence) = backend.confidence {
            confidence_factors.push(format!(
                "backend {} reported confidence {:.2}",
                backend.provider, confidence
            ));
        }
        if let Some(latency_ms) = backend.latency_ms {
            confidence_factors.push(format!(
                "backend {} returned evidence in {} ms",
                backend.provider, latency_ms
            ));
        }
        if let Some(warning) = backend_normalization_warning(backend) {
            warnings.push(warning);
            verification_steps.push(
                "If retained backend candidates are insufficient, rerun the backend with a narrower task instead of increasing the evidence payload."
                    .to_string(),
            );
        }
        verification_steps.push(format!(
            "Treat backend {} evidence as advisory unless the selected file and verification checks agree.",
            backend.provider
        ));
    }

    let evidence_sources_summary = if evidence_sources.is_empty() {
        "no source mix".to_string()
    } else {
        evidence_sources.join(", ")
    };
    let score = score.clamp(0, 100) as u8;
    let level = if score >= 80 {
        "high"
    } else if score >= 60 {
        "medium"
    } else {
        "low"
    };
    let recommended_action = if let Some(action) = backend_route_recommended_action {
        if context_pack.continuation_summary.status != "complete"
            && action == "compare_backend_route_before_edits"
        {
            "compare_backend_route_then_read_selected_context".to_string()
        } else {
            action
        }
    } else if context_pack.continuation_summary.status == "complete" {
        "read_selected_context".to_string()
    } else {
        "read_selected_context_then_use_continuation_if_needed".to_string()
    };
    let warning_note = if warnings.is_empty() {
        "No route-quality warnings were raised.".to_string()
    } else {
        format!(
            "Review {} route-quality warning(s) before editing.",
            warnings.len()
        )
    };
    let decision_summary = format!(
        "Read {} first with {} confidence (score {}, candidate rank {}, sources: {}). Then {}. {}",
        first_step.file,
        level,
        score,
        first_step.selection_rank,
        evidence_sources_summary,
        recommended_action,
        warning_note
    );

    AgentRouteQuality {
        level: level.to_string(),
        score,
        decision_summary,
        evidence_count,
        evidence_sources,
        confidence_factors,
        warnings,
        verification_steps,
        recommended_action,
    }
}

fn backend_evidence_signal_count(backend: &AgentRouteBackendEvidence) -> usize {
    backend.evidence_count.max(backend.candidate_files.len())
}

fn backend_evidence_sources(backend: &AgentRouteBackendEvidence) -> Vec<String> {
    let mut sources = backend
        .evidence_sources
        .iter()
        .map(|source| format!("backend:{}:{source}", backend.provider))
        .collect::<Vec<_>>();
    for candidate in &backend.candidates {
        if let Some(source) = candidate.source.as_ref() {
            sources.push(format!(
                "backend:{}:candidate_source:{source}",
                backend.provider
            ));
        }
        sources.extend(
            candidate
                .evidence
                .iter()
                .map(|item| format!("backend:{}:candidate_evidence:{item}", backend.provider)),
        );
    }
    if backend.evidence_count > 0 || !backend.candidate_files.is_empty() {
        sources.push(format!("backend:{}", backend.provider));
    }
    sources.sort();
    sources.dedup();
    sources
}

fn backend_normalization_warning(backend: &AgentRouteBackendEvidence) -> Option<String> {
    let normalization = backend.normalization.as_ref()?;
    Some(format!(
        "Backend evidence was bounded for token safety or reported incomplete: backend reported {} unfetched tool result item(s); CodeInsight omitted {} raw tool result item(s), {} candidate(s), {} candidate evidence item(s), {} source(s), and {} note(s); truncated {} text field(s).",
        normalization.unfetched_tool_result_items,
        normalization.omitted_tool_result_items,
        normalization.omitted_candidates,
        normalization.omitted_candidate_evidence_items,
        normalization.omitted_evidence_sources,
        normalization.omitted_notes,
        normalization.truncated_text_fields,
    ))
}

fn agent_route_execution_plan(
    context_pack: &ContextPack,
    impact_status: &str,
    impact_analysis: Option<&ImpactAnalysisReport>,
    deferred_impact_tool: Option<ContextSuggestedTool>,
    impact_seed_files: &[String],
    backend_route_agreement: &AgentRouteBackendAgreement,
    backend_selected_candidate: Option<&AgentRouteBackendCandidate>,
) -> Vec<AgentRouteExecutionStep> {
    let reading_files = context_pack
        .reading_plan
        .iter()
        .map(|step| step.file.clone())
        .collect::<Vec<_>>();
    let requested_locations_by_file = context_pack
        .reading_plan
        .iter()
        .filter(|step| !step.requested_locations.is_empty())
        .map(|step| (step.file.clone(), step.requested_locations.clone()))
        .collect::<BTreeMap<_, _>>();
    let first_step = context_pack.reading_plan.first();

    let mut plan = vec![AgentRouteExecutionStep {
        order: 1,
        action: "read_selected_context".to_string(),
        status: if reading_files.is_empty() {
            "blocked_no_reading_plan".to_string()
        } else {
            "ready".to_string()
        },
        instruction: match first_step {
            Some(step) => {
                let backend_context = backend_selected_candidate
                    .map(backend_candidate_summary)
                    .map(|summary| format!(" Backend evidence: {summary}."))
                    .unwrap_or_default();
                format!(
                    "Read context_pack.files[] in reading_plan[] order, starting with {} (candidate rank {}) with focus: {} Answer: {} Read-less evidence: selected {} of {} source lines, avoided {} ({} reduction, {} read-less ratio). Treat reading_plan[].reason as the current-step instruction and selection_reason as evidence for why each file was selected.{}",
                    step.file,
                    step.selection_rank,
                    step.focus,
                    step.question,
                    context_pack.read_less.selected_source_lines,
                    context_pack.read_less.baseline_source_lines,
                    context_pack.read_less.source_lines_avoided,
                    context_pack.read_less.line_reduction,
                    context_pack.read_less.read_less_ratio,
                    backend_context,
                )
            }
            None => {
                "No reading_plan was produced; narrow the task or provide seed files before broad reading."
                    .to_string()
            }
        },
        files: reading_files,
        requested_locations_by_file,
        suggested_tool: None,
        suggested_checks: Vec::new(),
    }];

    if matches!(
        backend_route_agreement.status.as_str(),
        "overlap" | "conflict" | "backend_only" | "no_local_route"
    ) {
        let mut files = backend_route_agreement
            .local_first_file
            .iter()
            .chain(backend_route_agreement.backend_first_file.iter())
            .cloned()
            .collect::<Vec<_>>();
        files.sort();
        files.dedup();
        let instruction = match backend_route_agreement.status.as_str() {
            "overlap" => format!(
                "{} Read the selected local context first, then compare the backend rank-1 candidate before choosing an edit target.",
                backend_route_agreement.message
            ),
            "backend_only" | "no_local_route" => format!(
                "{} Use the backend candidate as an explicit seed or provide a verified local seed, then rerun agent_route before editing.",
                backend_route_agreement.message
            ),
            _ => format!(
                "{} Compare the local and backend first-read candidates and resolve the conflict before editing.",
                backend_route_agreement.message
            ),
        };
        plan.push(AgentRouteExecutionStep {
            order: plan.len() + 1,
            action: backend_route_agreement.recommended_action.clone(),
            status: if backend_route_agreement.status == "overlap" {
                "available_after_selected_context".to_string()
            } else {
                "required_before_edits".to_string()
            },
            instruction,
            files,
            requested_locations_by_file: BTreeMap::new(),
            suggested_tool: None,
            suggested_checks: Vec::new(),
        });
    }

    if let Some(step) = first_step {
        plan.push(AgentRouteExecutionStep {
            order: plan.len() + 1,
            action: "use_current_reading_step_suggested_tool".to_string(),
            status: "available_after_current_file".to_string(),
            instruction: format!(
                "After reading {}, call {} only if deeper evidence is needed for {} with focus: {} Answer: {}",
                step.file, step.suggested_tool.tool, step.next_action, step.focus, step.question
            ),
            files: vec![step.file.clone()],
            requested_locations_by_file: BTreeMap::new(),
            suggested_tool: Some(step.suggested_tool.clone()),
            suggested_checks: Vec::new(),
        });
    } else {
        plan.push(AgentRouteExecutionStep {
            order: plan.len() + 1,
            action: "use_current_reading_step_suggested_tool".to_string(),
            status: "blocked_no_current_reading_step".to_string(),
            instruction: "No current_reading_step is available; provide a seed file or symbol, or add source files before requesting a suggested follow-up tool.".to_string(),
            files: Vec::new(),
            requested_locations_by_file: BTreeMap::new(),
            suggested_tool: None,
            suggested_checks: Vec::new(),
        });
    }

    let continuation = &context_pack.continuation_summary;
    let backend_continuation = backend_route_agreement.next_candidate_continuation.as_ref();
    let continuation_suggested_tool = backend_continuation
        .map(|candidate| candidate.suggested_tool.clone())
        .or_else(|| continuation.suggested_tool.clone());
    plan.push(AgentRouteExecutionStep {
        order: plan.len() + 1,
        action: backend_continuation
            .map(|candidate| candidate.next_action.clone())
            .unwrap_or_else(|| "use_continuation_if_needed".to_string()),
        status: match continuation_suggested_tool.as_ref() {
            Some(_) => "available_after_selected_context".to_string(),
            None if continuation.status == "complete" => "complete".to_string(),
            None => "manual_after_selected_context".to_string(),
        },
        instruction: backend_continuation
            .map(agent_route_backend_continuation_instruction)
            .unwrap_or_else(|| agent_route_continuation_instruction(context_pack)),
        files: backend_continuation
            .map(|candidate| vec![candidate.file.clone()])
            .unwrap_or_else(|| {
                continuation
                    .first_omitted_file
                    .iter()
                    .cloned()
                    .collect::<Vec<_>>()
            }),
        requested_locations_by_file: BTreeMap::new(),
        suggested_tool: continuation_suggested_tool,
        suggested_checks: Vec::new(),
    });

    plan.push(AgentRouteExecutionStep {
        order: plan.len() + 1,
        action: "review_impact_before_edits".to_string(),
        status: if impact_status == "deferred_by_request" {
            "required_before_edits".to_string()
        } else {
            impact_status.to_string()
        },
        instruction: match impact_analysis {
            Some(report) => agent_route_impact_instruction(report),
            None => agent_route_skipped_impact_instruction(impact_status),
        },
        files: impact_analysis
            .map(|report| report.seed_files.clone())
            .unwrap_or_else(|| {
                if impact_status == "deferred_by_request" {
                    impact_seed_files.to_vec()
                } else {
                    Vec::new()
                }
            }),
        requested_locations_by_file: BTreeMap::new(),
        suggested_tool: impact_analysis
            .map(agent_route_impact_suggested_tool)
            .or(deferred_impact_tool),
        suggested_checks: impact_analysis
            .map(|report| report.suggested_checks.clone())
            .unwrap_or_default(),
    });

    plan
}

fn agent_route_backend_continuation_instruction(
    continuation: &AgentRouteBackendCandidateContinuation,
) -> String {
    format!(
        "After reading the selected context, if it is insufficient, call {} with suggested_arguments to inspect backend candidate {} (rank {}, reason {}). Follow {} without broad repository reading.",
        continuation.suggested_tool.tool,
        continuation.file,
        continuation.rank,
        continuation.context_reason,
        continuation.next_action
    )
}

fn normalize_agent_route_backend_evidence(
    root: &Path,
    mut evidence: AgentRouteBackendEvidence,
) -> Result<AgentRouteBackendEvidence> {
    evidence.provider = evidence.provider.trim().to_string();
    if evidence.provider.is_empty() {
        bail!("backend evidence provider must not be empty");
    }
    if evidence.provider.chars().count() > BACKEND_EVIDENCE_PROVIDER_CHARS_LIMIT {
        bail!(
            "backend evidence provider must not exceed {} characters",
            BACKEND_EVIDENCE_PROVIDER_CHARS_LIMIT
        );
    }
    if let Some(confidence) = evidence.confidence
        && (!confidence.is_finite() || !(0.0..=1.0).contains(&confidence))
    {
        bail!("backend evidence confidence must be between 0.0 and 1.0");
    }
    let (omitted_tool_result_items, unfetched_tool_result_items) =
        merge_backend_tool_results(root, &mut evidence)?;

    let legacy_files = evidence
        .candidate_files
        .into_iter()
        .map(|file| {
            let file = file.trim();
            if file.is_empty() {
                bail!("backend evidence candidate file must not be empty");
            }
            normalize_backend_candidate_file(root, file)
                .with_context(|| format!("invalid backend evidence candidate file: {file}"))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut normalization = AgentRouteBackendNormalization {
        candidate_limit: BACKEND_EVIDENCE_CANDIDATE_LIMIT,
        unfetched_tool_result_items,
        omitted_tool_result_items,
        ..AgentRouteBackendNormalization::default()
    };
    let mut remaining_candidate_evidence = BACKEND_EVIDENCE_TOTAL_CANDIDATE_ITEMS_LIMIT;
    let mut seen_files = BTreeSet::new();
    let mut candidates = Vec::new();
    for mut candidate in evidence.candidates {
        let raw_file = candidate.file.trim();
        if raw_file.is_empty() {
            bail!("backend evidence candidate file must not be empty");
        }
        candidate.file = normalize_backend_candidate_file(root, raw_file)
            .with_context(|| format!("invalid backend evidence candidate file: {raw_file}"))?;
        if candidate.locations.len() > BACKEND_EVIDENCE_CANDIDATE_LOCATION_LIMIT {
            bail!(
                "backend evidence candidate locations must contain at most {} items",
                BACKEND_EVIDENCE_CANDIDATE_LOCATION_LIMIT
            );
        }
        for location in &candidate.locations {
            if location.start_line == 0 || location.end_line < location.start_line {
                bail!("backend evidence candidate location must be a valid one-based line range");
            }
        }
        candidate
            .locations
            .sort_by_key(|location| (location.start_line, location.end_line));
        candidate
            .locations
            .dedup_by_key(|location| (location.start_line, location.end_line));
        candidate.symbol = bounded_optional_string(
            candidate.symbol,
            BACKEND_EVIDENCE_SYMBOL_CHARS_LIMIT,
            &mut normalization.truncated_text_fields,
        );
        candidate.source = bounded_optional_string(
            candidate.source,
            BACKEND_EVIDENCE_SOURCE_CHARS_LIMIT,
            &mut normalization.truncated_text_fields,
        );
        candidate.reason = bounded_optional_string(
            candidate.reason,
            BACKEND_EVIDENCE_REASON_CHARS_LIMIT,
            &mut normalization.truncated_text_fields,
        );
        let (mut candidate_evidence, omitted_evidence) = normalized_bounded_strings(
            candidate.evidence,
            BACKEND_EVIDENCE_PER_CANDIDATE_LIMIT,
            BACKEND_EVIDENCE_ITEM_CHARS_LIMIT,
            &mut normalization.truncated_text_fields,
        );
        normalization.omitted_candidate_evidence_items += omitted_evidence;
        if let Some(score) = candidate.score
            && !score.is_finite()
        {
            bail!("backend evidence candidate score must be finite");
        }
        if !seen_files.insert(candidate.file.clone()) {
            normalization.omitted_candidate_evidence_items += candidate_evidence.len();
            continue;
        }
        if candidates.len() >= BACKEND_EVIDENCE_CANDIDATE_LIMIT {
            normalization.omitted_candidates += 1;
            normalization.omitted_candidate_evidence_items += candidate_evidence.len();
            continue;
        }
        if candidate_evidence.len() > remaining_candidate_evidence {
            normalization.omitted_candidate_evidence_items +=
                candidate_evidence.len() - remaining_candidate_evidence;
            candidate_evidence.truncate(remaining_candidate_evidence);
        }
        remaining_candidate_evidence =
            remaining_candidate_evidence.saturating_sub(candidate_evidence.len());
        candidate.evidence = candidate_evidence;
        candidates.push(candidate);
    }
    let mut candidate_files = candidates
        .iter()
        .map(|candidate| candidate.file.clone())
        .collect::<Vec<_>>();
    for file in legacy_files {
        if !seen_files.insert(file.clone()) {
            continue;
        }
        if candidate_files.len() >= BACKEND_EVIDENCE_CANDIDATE_LIMIT {
            normalization.omitted_candidates += 1;
            continue;
        }
        candidate_files.push(file);
    }
    evidence.candidate_files = candidate_files;
    evidence.candidates = candidates;

    let (evidence_sources, omitted_evidence_sources) = normalized_bounded_strings(
        evidence.evidence_sources,
        BACKEND_EVIDENCE_SOURCES_LIMIT,
        BACKEND_EVIDENCE_SOURCE_CHARS_LIMIT,
        &mut normalization.truncated_text_fields,
    );
    normalization.omitted_evidence_sources = omitted_evidence_sources;
    evidence.evidence_sources = evidence_sources;
    let (notes, omitted_notes) = normalized_bounded_strings(
        evidence.notes,
        BACKEND_EVIDENCE_NOTES_LIMIT,
        BACKEND_EVIDENCE_NOTE_CHARS_LIMIT,
        &mut normalization.truncated_text_fields,
    );
    normalization.omitted_notes = omitted_notes;
    evidence.notes = notes;
    evidence.normalization = backend_normalization_changed(&normalization).then_some(normalization);
    Ok(evidence)
}

fn merge_backend_tool_results(
    root: &Path,
    evidence: &mut AgentRouteBackendEvidence,
) -> Result<(usize, usize)> {
    let Some(tool_results) = evidence.tool_results.take() else {
        return Ok((0, 0));
    };
    let get_code_snippet = tool_results
        .get_code_snippet
        .map(normalize_backend_code_snippet_result)
        .transpose()?;
    let query_graph = tool_results
        .query_graph
        .map(normalize_backend_query_graph_result)
        .transpose()?;
    let trace_path = tool_results
        .trace_path
        .map(|raw| normalize_backend_trace_path_result(root, raw))
        .transpose()?;

    let tool_inputs = [
        (
            get_code_snippet,
            BackendToolResultSpec {
                source: "get_code_snippet",
                items_keys: &["results"],
                preferred_items_key: None,
                total_keys: &["total"],
                total_items_keys: &["results"],
                file_keys: &["file_path", "file"],
                symbol_keys: &["qualified_name", "name"],
            },
        ),
        (
            tool_results.search_graph,
            BackendToolResultSpec {
                source: "search_graph",
                items_keys: &["results"],
                preferred_items_key: Some("semantic_results"),
                total_keys: &["total"],
                total_items_keys: &["results"],
                file_keys: &["file_path", "file"],
                symbol_keys: &["name", "node", "qualified_name"],
            },
        ),
        (
            tool_results.search_code,
            BackendToolResultSpec {
                source: "search_code",
                items_keys: &["results", "files", "raw_matches"],
                preferred_items_key: None,
                total_keys: &["total_results"],
                total_items_keys: &["results"],
                file_keys: &["file", "file_path"],
                symbol_keys: &["node", "name", "qualified_name"],
            },
        ),
        (
            query_graph,
            BackendToolResultSpec {
                source: "query_graph",
                items_keys: &["results"],
                preferred_items_key: None,
                total_keys: &["total"],
                total_items_keys: &["results"],
                file_keys: &["file_path", "file"],
                symbol_keys: &["name", "qualified_name"],
            },
        ),
        (
            trace_path,
            BackendToolResultSpec {
                source: "trace_path",
                items_keys: &["results"],
                preferred_items_key: None,
                total_keys: &["total"],
                total_items_keys: &["results"],
                file_keys: &["file_path", "file"],
                symbol_keys: &["name", "qualified_name"],
            },
        ),
        (
            tool_results.get_architecture,
            BackendToolResultSpec {
                source: "get_architecture:entry_points",
                items_keys: &["entry_points"],
                preferred_items_key: None,
                total_keys: &[],
                total_items_keys: &[],
                file_keys: &["file", "file_path"],
                symbol_keys: &["name", "qualified_name"],
            },
        ),
    ];
    let mut candidates = Vec::new();
    let mut evidence_sources = Vec::new();
    let mut evidence_count = 0usize;
    let mut unfetched_items = 0usize;
    let mut omitted_items = 0usize;
    let mut latency_ms = 0u64;
    for (raw, spec) in tool_inputs {
        let Some(raw) = raw else {
            continue;
        };
        let batch = collect_backend_tool_candidates(raw, spec)?;
        candidates.extend(batch.candidates);
        if let Some(source) = batch.source {
            evidence_sources.push(source);
        }
        evidence_count = evidence_count.saturating_add(batch.evidence_count);
        unfetched_items = unfetched_items.saturating_add(batch.unfetched_items);
        omitted_items = omitted_items.saturating_add(batch.omitted_items);
        latency_ms = latency_ms.saturating_add(batch.latency_ms);
    }

    if candidates.is_empty()
        && evidence.candidates.is_empty()
        && evidence.candidate_files.is_empty()
    {
        bail!("backend evidence tool_results contained no candidate files");
    }
    evidence.candidates.extend(candidates);
    evidence.evidence_sources.extend(evidence_sources);
    evidence.evidence_count = evidence.evidence_count.saturating_add(evidence_count);
    if latency_ms > 0 {
        evidence.latency_ms = Some(
            evidence
                .latency_ms
                .unwrap_or_default()
                .saturating_add(latency_ms),
        );
    }
    evidence
        .notes
        .push("normalized from inline backend tool_results".to_string());
    Ok((omitted_items, unfetched_items))
}

fn normalize_backend_query_graph_result(raw: Value) -> Result<Value> {
    let pages = match raw {
        Value::Array(pages) if pages.is_empty() => {
            bail!("backend evidence query_graph tool result page array must not be empty")
        }
        Value::Array(pages) => pages,
        raw => vec![raw],
    };
    if pages.len() > BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT {
        bail!(
            "backend evidence query_graph tool result must not exceed {} pages",
            BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT
        );
    }

    let page_count = pages.len();
    let mut normalized_pages = Vec::with_capacity(page_count);
    for (page_index, raw_page) in pages.into_iter().enumerate() {
        let page_source = if page_count == 1 {
            "query_graph".to_string()
        } else {
            format!("query_graph page {}", page_index + 1)
        };
        let payload = backend_tool_result_payload(raw_page, &page_source)?;
        let columns = payload
            .get("columns")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "backend evidence {page_source} tool result field columns must be an array"
                )
            })?;
        let rows = payload
            .get("rows")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "backend evidence {page_source} tool result field rows must be an array"
                )
            })?;
        let file_index = backend_query_graph_column_index(columns, &["file_path", "file"])
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "backend evidence {page_source} tool result columns must include file_path or file"
                )
            })?;
        let symbol_index = backend_query_graph_column_index(columns, &["name"])
            .or_else(|| backend_query_graph_column_index(columns, &["qualified_name"]));
        let mut results = Vec::new();
        for row in rows.iter().take(BACKEND_EVIDENCE_TOOL_RESULT_ITEMS_LIMIT) {
            let row = row.as_array().ok_or_else(|| {
                anyhow::anyhow!(
                    "backend evidence {page_source} tool result rows must contain arrays"
                )
            })?;
            let Some(file) = row.get(file_index).and_then(Value::as_str) else {
                continue;
            };
            let file = file.trim();
            if file.is_empty() {
                continue;
            }
            let symbol = symbol_index
                .and_then(|index| row.get(index))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|symbol| !symbol.is_empty());
            results.push(json!({
                "file_path": file,
                "name": symbol,
                "label": "row"
            }));
        }
        normalized_pages.push(json!({
            "results": results,
            "total": payload
                .get("total")
                .and_then(Value::as_u64)
                .unwrap_or(rows.len() as u64),
            "elapsed_ms": payload
                .get("elapsed_ms")
                .or_else(|| payload.get("duration_ms"))
                .and_then(Value::as_u64)
                .unwrap_or_default()
        }));
    }

    Ok(if page_count == 1 {
        normalized_pages.pop().unwrap_or_default()
    } else {
        Value::Array(normalized_pages)
    })
}

fn backend_query_graph_column_index(columns: &[Value], names: &[&str]) -> Option<usize> {
    columns.iter().position(|column| {
        column.as_str().is_some_and(|column| {
            let normalized = column
                .rsplit('.')
                .next()
                .unwrap_or(column)
                .trim_matches(['`', '"'])
                .to_ascii_lowercase();
            names.contains(&normalized.as_str())
        })
    })
}

fn normalize_backend_code_snippet_result(raw: Value) -> Result<Value> {
    let pages = match raw {
        Value::Array(pages) if pages.is_empty() => {
            bail!("backend evidence get_code_snippet tool result page array must not be empty")
        }
        Value::Array(pages) => pages,
        raw => vec![raw],
    };
    if pages.len() > BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT {
        bail!(
            "backend evidence get_code_snippet tool result must not exceed {} pages",
            BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT
        );
    }

    let page_count = pages.len();
    let mut normalized_pages = Vec::with_capacity(page_count);
    for (page_index, raw_page) in pages.into_iter().enumerate() {
        let page_source = if page_count == 1 {
            "get_code_snippet".to_string()
        } else {
            format!("get_code_snippet page {}", page_index + 1)
        };
        let payload = backend_tool_result_payload(raw_page, &page_source)?;
        let file =
            first_backend_tool_string(&payload, &["file_path", "file"]).ok_or_else(|| {
                anyhow::anyhow!(
                    "backend evidence {page_source} tool result must contain file_path or file"
                )
            })?;
        let symbol = first_backend_tool_string(&payload, &["qualified_name", "name"]);
        let label = first_backend_tool_string(&payload, &["label"])
            .unwrap_or_else(|| "snippet".to_string());
        normalized_pages.push(json!({
            "results": [{
                "file_path": file,
                "qualified_name": symbol,
                "label": label
            }],
            "total": 1,
            "elapsed_ms": payload
                .get("elapsed_ms")
                .or_else(|| payload.get("duration_ms"))
                .and_then(Value::as_u64)
                .unwrap_or_default()
        }));
    }

    Ok(if page_count == 1 {
        normalized_pages.pop().unwrap_or_default()
    } else {
        Value::Array(normalized_pages)
    })
}

fn normalize_backend_trace_path_result(root: &Path, raw: Value) -> Result<Value> {
    let pages = match raw {
        Value::Array(pages) if pages.is_empty() => {
            bail!("backend evidence trace_path tool result page array must not be empty")
        }
        Value::Array(pages) => pages,
        raw => vec![raw],
    };
    if pages.len() > BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT {
        bail!(
            "backend evidence trace_path tool result must not exceed {} pages",
            BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT
        );
    }

    let store = Store::open(root)?;
    let page_count = pages.len();
    let mut normalized_pages = Vec::with_capacity(page_count);
    for (page_index, raw_page) in pages.into_iter().enumerate() {
        let page_source = if page_count == 1 {
            "trace_path".to_string()
        } else {
            format!("trace_path page {}", page_index + 1)
        };
        let payload = backend_tool_result_payload(raw_page, &page_source)?;
        let mut results = Vec::new();
        let mut total = 0usize;
        let mut found_items = false;
        if let Some(backend_symbol) = first_backend_tool_string(&payload, &["function"]) {
            found_items = true;
            total = total.saturating_add(1);
            if let Some(local_symbol) = resolve_backend_trace_symbol(&store, &backend_symbol, None)?
            {
                results.push(json!({
                    "file_path": local_symbol.file,
                    "name": local_symbol.qualified_name,
                    "label": "subject"
                }));
            }
        }
        for (items_key, label) in [("callers", "caller"), ("callees", "callee")] {
            let Some(items) = payload.get(items_key) else {
                continue;
            };
            let items = items.as_array().ok_or_else(|| {
                anyhow::anyhow!(
                    "backend evidence {page_source} tool result field {items_key} must be an array"
                )
            })?;
            found_items = true;
            total = total.saturating_add(items.len());
            for item in items {
                if results.len() >= BACKEND_EVIDENCE_TOOL_RESULT_ITEMS_LIMIT {
                    break;
                }
                let Some(backend_symbol) =
                    first_backend_tool_string(item, &["qualified_name", "name", "node"])
                else {
                    continue;
                };
                let lookup_name = item
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|name| !name.is_empty());
                let Some(local_symbol) =
                    resolve_backend_trace_symbol(&store, &backend_symbol, lookup_name)?
                else {
                    continue;
                };
                results.push(json!({
                    "file_path": local_symbol.file,
                    "name": local_symbol.qualified_name,
                    "label": label
                }));
            }
        }
        if !found_items {
            bail!(
                "backend evidence {page_source} tool result must contain function, callers, or callees"
            );
        }
        normalized_pages.push(json!({
            "results": results,
            "total": total,
            "elapsed_ms": payload
                .get("elapsed_ms")
                .or_else(|| payload.get("duration_ms"))
                .and_then(Value::as_u64)
                .unwrap_or_default()
        }));
    }

    Ok(if page_count == 1 {
        normalized_pages.pop().unwrap_or_default()
    } else {
        Value::Array(normalized_pages)
    })
}

fn collect_backend_tool_candidates(
    raw: Value,
    spec: BackendToolResultSpec<'_>,
) -> Result<BackendToolCandidateBatch> {
    let pages = match raw {
        Value::Array(pages) if pages.is_empty() => {
            bail!(
                "backend evidence {} tool result page array must not be empty",
                spec.source
            )
        }
        Value::Array(pages) => pages,
        raw => vec![raw],
    };
    if pages.len() > BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT {
        bail!(
            "backend evidence {} tool result must not exceed {} pages",
            spec.source,
            BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT
        );
    }
    let mut candidates = Vec::new();
    let mut item_count = 0usize;
    let mut fetched_total_items = 0usize;
    let mut processed_item_count = 0usize;
    let mut reported_total_items = 0usize;
    let mut last_page_has_more = false;
    let mut latency_ms = 0u64;
    let mut seen_candidate_files = BTreeSet::new();
    let mut candidate_indexes_by_file = BTreeMap::new();
    let candidate_dedupe_limit = BACKEND_EVIDENCE_TOOL_RESULT_ITEMS_LIMIT
        .saturating_mul(BACKEND_EVIDENCE_TOOL_RESULT_PAGES_LIMIT);
    let page_count = pages.len();
    for (page_index, raw_page) in pages.into_iter().enumerate() {
        let page_source = if page_count == 1 {
            spec.source.to_string()
        } else {
            format!("{} page {}", spec.source, page_index + 1)
        };
        let payload = backend_tool_result_payload(raw_page, &page_source)?;
        last_page_has_more = payload
            .get("has_more")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let preferred_items_key = match spec.preferred_items_key {
            Some(items_key) => match payload.get(items_key) {
                Some(Value::Array(items)) if items.is_empty() => None,
                Some(Value::Array(_)) => Some(items_key),
                Some(_) => {
                    bail!(
                        "backend evidence {page_source} tool result field {items_key} must be an array"
                    )
                }
                None => None,
            },
            None => None,
        };
        let selected_items_keys = preferred_items_key
            .map(|items_key| vec![items_key])
            .unwrap_or_else(|| spec.items_keys.to_vec());
        let should_read_totals = preferred_items_key.is_none()
            && selected_items_keys.iter().any(|items_key| {
                payload.get(*items_key).is_some() && spec.total_items_keys.contains(items_key)
            });
        let mut found_items = false;
        for items_key in selected_items_keys {
            let Some(value) = payload.get(items_key) else {
                continue;
            };
            let items = value.as_array().ok_or_else(|| {
                anyhow::anyhow!(
                    "backend evidence {page_source} tool result field {items_key} must be an array"
                )
            })?;
            found_items = true;
            if preferred_items_key.is_none() && spec.total_items_keys.contains(&items_key) {
                fetched_total_items = fetched_total_items.saturating_add(items.len());
            }
            for item in items {
                let (file, symbol, label) = match item {
                    Value::String(file) => {
                        let file = file.trim();
                        if file.is_empty() {
                            continue;
                        }
                        (file.to_string(), None, "File")
                    }
                    _ => {
                        let Some(file) = first_backend_tool_string(item, spec.file_keys) else {
                            continue;
                        };
                        let symbol = first_backend_tool_string(item, spec.symbol_keys);
                        let label = item.get("label").and_then(Value::as_str).unwrap_or(
                            if items_key == "raw_matches" {
                                "raw match"
                            } else {
                                "result"
                            },
                        );
                        (file, symbol, label)
                    }
                };
                let locations = backend_tool_candidate_locations(item);
                if seen_candidate_files.contains(&file) {
                    if let Some(candidate_index) = candidate_indexes_by_file.get(&file).copied() {
                        merge_backend_candidate_locations(
                            &mut candidates[candidate_index],
                            locations,
                        );
                    }
                    continue;
                }
                if seen_candidate_files.len() < candidate_dedupe_limit {
                    seen_candidate_files.insert(file.clone());
                }
                item_count = item_count.saturating_add(1);
                if processed_item_count >= BACKEND_EVIDENCE_TOOL_RESULT_ITEMS_LIMIT {
                    continue;
                }
                processed_item_count = processed_item_count.saturating_add(1);
                let reason = if spec.source == "get_architecture:entry_points" {
                    "get_architecture entry point".to_string()
                } else {
                    format!("{} {label}", spec.source)
                };
                let score = item
                    .get("score")
                    .or_else(|| item.get("similarity"))
                    .or_else(|| item.get("confidence"))
                    .and_then(Value::as_f64);
                let candidate_index = candidates.len();
                candidates.push(AgentRouteBackendCandidate {
                    file: file.clone(),
                    symbol,
                    locations,
                    source: Some(spec.source.to_string()),
                    score,
                    reason: Some(reason),
                    evidence: vec![spec.source.to_string()],
                });
                candidate_indexes_by_file.insert(file, candidate_index);
            }
        }
        if !found_items {
            let expected_items_keys =
                preferred_items_key.map(str::to_string).unwrap_or_else(|| {
                    let mut items_keys = spec.items_keys.to_vec();
                    if let Some(items_key) = spec.preferred_items_key {
                        items_keys.push(items_key);
                    }
                    items_keys.join(" or ")
                });
            bail!(
                "backend evidence {page_source} tool result must contain an array field named {}",
                expected_items_keys
            );
        }
        if should_read_totals {
            for total_key in spec.total_keys {
                reported_total_items = reported_total_items.max(
                    payload
                        .get(total_key)
                        .and_then(Value::as_u64)
                        .and_then(|total| usize::try_from(total).ok())
                        .unwrap_or_default(),
                );
            }
        }
        latency_ms = latency_ms.saturating_add(
            payload
                .get("elapsed_ms")
                .or_else(|| payload.get("duration_ms"))
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        );
    }
    let evidence_count = candidates.len();
    let unfetched_items = reported_total_items
        .saturating_sub(fetched_total_items)
        .max(usize::from(last_page_has_more));
    Ok(BackendToolCandidateBatch {
        candidates,
        source: (evidence_count > 0).then(|| spec.source.to_string()),
        evidence_count,
        unfetched_items,
        omitted_items: item_count.saturating_sub(BACKEND_EVIDENCE_TOOL_RESULT_ITEMS_LIMIT),
        latency_ms,
    })
}

fn merge_backend_candidate_locations(
    candidate: &mut AgentRouteBackendCandidate,
    locations: Vec<ContextSeedLocation>,
) {
    for location in locations {
        if candidate.locations.len() >= BACKEND_EVIDENCE_CANDIDATE_LOCATION_LIMIT {
            break;
        }
        if !candidate.locations.iter().any(|existing| {
            existing.start_line == location.start_line && existing.end_line == location.end_line
        }) {
            candidate.locations.push(location);
        }
    }
}

fn backend_tool_result_payload(raw: Value, source: &str) -> Result<Value> {
    let mut current = raw;
    for wrapper_depth in 0..=BACKEND_EVIDENCE_TOOL_RESULT_WRAPPER_LIMIT {
        if let Some(error) = current.get("error") {
            let message = error
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| error.as_str())
                .unwrap_or("unspecified JSON-RPC error");
            bail!(
                "backend evidence {source} tool result returned JSON-RPC error: {}",
                bounded_backend_tool_error(message)
            );
        }
        if current.get("isError").and_then(Value::as_bool) == Some(true) {
            let message =
                backend_tool_result_text(&current).unwrap_or("unspecified MCP tool error");
            bail!(
                "backend evidence {source} tool result returned MCP error: {}",
                bounded_backend_tool_error(message)
            );
        }
        if let Some(payload) = current
            .get("structuredContent")
            .filter(|value| value.is_object())
        {
            return Ok(payload.clone());
        }
        if let Some(result) = current.get("result").cloned() {
            if wrapper_depth == BACKEND_EVIDENCE_TOOL_RESULT_WRAPPER_LIMIT {
                bail!(
                    "backend evidence {source} tool result must not exceed {} nested result wrappers",
                    BACKEND_EVIDENCE_TOOL_RESULT_WRAPPER_LIMIT
                );
            }
            current = result;
            continue;
        }
        if let Some(payload) = backend_tool_result_json(&current) {
            return Ok(payload);
        }
        if backend_tool_result_text(&current).is_some() {
            bail!("backend evidence {source} text content contains no valid JSON");
        }
        if current.is_object() {
            return Ok(current);
        }
        bail!("backend evidence {source} tool result must be a JSON object");
    }
    unreachable!()
}

fn backend_tool_result_text(value: &Value) -> Option<&str> {
    value.get("content")?.as_array()?.iter().find_map(|item| {
        (item.get("type").and_then(Value::as_str) == Some("text"))
            .then(|| item.get("text").and_then(Value::as_str))
            .flatten()
    })
}

fn backend_tool_result_json(value: &Value) -> Option<Value> {
    value
        .get("content")?
        .as_array()?
        .iter()
        .filter(|item| item.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|item| item.get("text").and_then(Value::as_str))
        .find_map(|text| serde_json::from_str(text).ok())
}

fn bounded_backend_tool_error(message: &str) -> String {
    let normalized = message.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = if normalized.is_empty() {
        "unspecified backend error"
    } else {
        normalized.as_str()
    };
    normalized
        .chars()
        .take(BACKEND_EVIDENCE_TOOL_ERROR_CHARS_LIMIT)
        .collect()
}

fn first_backend_tool_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn backend_tool_candidate_locations(value: &Value) -> Vec<ContextSeedLocation> {
    let Some(start_line) = value
        .get("start_line")
        .and_then(Value::as_u64)
        .and_then(|line| usize::try_from(line).ok())
        .filter(|line| *line > 0)
    else {
        return Vec::new();
    };
    let end_line = value
        .get("end_line")
        .and_then(Value::as_u64)
        .and_then(|line| usize::try_from(line).ok())
        .filter(|line| *line >= start_line)
        .unwrap_or(start_line);
    vec![ContextSeedLocation {
        start_line,
        end_line,
    }]
}

fn backend_seed_context_pack(
    root: &Path,
    task: &str,
    token_budget: usize,
    backend_evidence: &AgentRouteBackendEvidence,
    mode: BackendContextMode,
) -> Result<BackendContextAttempt> {
    let store = Store::open(root)?;
    let indexed_files = store.indexed_files()?.into_iter().collect::<BTreeSet<_>>();
    let valid_candidate_files = backend_evidence
        .candidate_files
        .iter()
        .filter(|candidate_file| {
            root.join(candidate_file).is_file() && indexed_files.contains(*candidate_file)
        })
        .cloned()
        .collect::<Vec<_>>();
    let valid_candidate_file_set = valid_candidate_files.iter().collect::<BTreeSet<_>>();
    let indexed_symbols =
        store.symbols_for_files(&valid_candidate_files, store.count_symbols()?)?;
    let mut candidates = backend_evidence
        .candidate_files
        .iter()
        .filter(|candidate_file| valid_candidate_file_set.contains(candidate_file))
        .map(|candidate_file| {
            let mut candidate = backend_evidence
                .candidates
                .iter()
                .find(|candidate| candidate.file == *candidate_file)
                .cloned()
                .unwrap_or_else(|| AgentRouteBackendCandidate {
                    file: candidate_file.clone(),
                    symbol: None,
                    locations: Vec::new(),
                    source: None,
                    score: None,
                    reason: None,
                    evidence: Vec::new(),
                });
            if candidate.symbol.as_ref().is_some_and(|candidate_symbol| {
                !indexed_symbols.iter().any(|symbol| {
                    symbol.file == *candidate_file
                        && backend_symbol_name_matches(
                            candidate_symbol,
                            &symbol.name,
                            &symbol.qualified_name,
                        )
                })
            }) {
                candidate.symbol = None;
            }
            candidate
        })
        .collect::<Vec<_>>();
    let backend_task_keywords = task_keywords(task);
    if !backend_task_prefers_support_files(&backend_task_keywords) {
        candidates.sort_by_key(|candidate| backend_candidate_is_support_file(&candidate.file));
    }

    for primary_index in 0..candidates.len() {
        let candidate_end = match mode {
            BackendContextMode::Fallback => primary_index + 1,
            BackendContextMode::Preferred => candidates.len(),
        };
        let ranked_candidates = &candidates[primary_index..candidate_end];
        let primary_candidate = &ranked_candidates[0];
        let ranked_symbols = match mode {
            BackendContextMode::Fallback => primary_candidate.symbol.clone().into_iter().collect(),
            BackendContextMode::Preferred => ranked_candidates
                .iter()
                .filter_map(|candidate| candidate.symbol.clone())
                .collect(),
        };
        let ranked_files = backend_candidate_context_seed_files(ranked_candidates);
        let context_result = context_pack_value(
            root.to_path_buf(),
            task.to_string(),
            ranked_symbols,
            ranked_files.clone(),
            token_budget,
        );
        let context_result = match context_result {
            Err(error)
                if matches!(mode, BackendContextMode::Preferred)
                    && is_context_pack_invalid_seed_error(&error) =>
            {
                context_pack_value(
                    root.to_path_buf(),
                    task.to_string(),
                    Vec::new(),
                    ranked_files,
                    token_budget,
                )
            }
            result => result,
        };
        match context_result {
            Ok(mut context_pack) if !context_pack.reading_plan.is_empty() => {
                context_pack.seed_strategy = mode.seed_source().to_string();
                for seed in &mut context_pack.selected_seeds {
                    seed.source = mode.seed_source().to_string();
                }
                let selected_files = context_pack
                    .files
                    .iter()
                    .map(|file| file.file.as_str())
                    .collect::<BTreeSet<_>>();
                let selected_candidates = ranked_candidates
                    .iter()
                    .filter(|candidate| selected_files.contains(candidate.file.as_str()))
                    .cloned()
                    .collect::<Vec<_>>();
                for candidate in &selected_candidates {
                    if let Some(symbol) = candidate.symbol.as_ref()
                        && let Some(seed) = context_pack
                            .selected_seeds
                            .iter_mut()
                            .find(|seed| seed.value == candidate.file)
                        && !seed.matched_symbols.contains(symbol)
                    {
                        seed.matched_symbols.push(symbol.clone());
                    }
                }
                annotate_backend_seed_context(
                    &mut context_pack,
                    backend_evidence,
                    &selected_candidates,
                );
                let candidate_dispositions = backend_candidate_dispositions(
                    root,
                    &indexed_files,
                    backend_evidence,
                    &candidates,
                    &selected_candidates,
                    mode,
                );
                return Ok(BackendContextAttempt {
                    context_pack: Some(context_pack),
                    selection: BackendContextSelection {
                        candidates: selected_candidates,
                        candidate_dispositions,
                    },
                });
            }
            Ok(_) => {}
            Err(error) if is_context_pack_invalid_seed_error(&error) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(BackendContextAttempt {
        context_pack: None,
        selection: BackendContextSelection {
            candidates: Vec::new(),
            candidate_dispositions: backend_candidate_dispositions(
                root,
                &indexed_files,
                backend_evidence,
                &candidates,
                &[],
                mode,
            ),
        },
    })
}

fn backend_candidate_dispositions(
    root: &Path,
    indexed_files: &BTreeSet<String>,
    backend_evidence: &AgentRouteBackendEvidence,
    valid_candidates: &[AgentRouteBackendCandidate],
    selected_candidates: &[AgentRouteBackendCandidate],
    mode: BackendContextMode,
) -> Vec<AgentRouteBackendCandidateDisposition> {
    let selected_files = selected_candidates
        .iter()
        .map(|candidate| candidate.file.as_str())
        .collect::<BTreeSet<_>>();

    backend_evidence
        .candidate_files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let original_candidate = backend_evidence
                .candidates
                .iter()
                .find(|candidate| candidate.file == *file);
            let valid_candidate = valid_candidates
                .iter()
                .find(|candidate| candidate.file == *file);
            let context_rank = valid_candidates
                .iter()
                .position(|candidate| candidate.file == *file)
                .map(|position| position + 1);
            let original_context_rank = backend_evidence
                .candidate_files
                .iter()
                .filter(|candidate_file| {
                    root.join(candidate_file).is_file() && indexed_files.contains(*candidate_file)
                })
                .position(|candidate_file| candidate_file == file)
                .map(|position| position + 1);
            let routing_reason = context_rank
                .zip(original_context_rank)
                .filter(|(context_rank, original_context_rank)| {
                    context_rank != original_context_rank
                })
                .map(|(context_rank, original_context_rank)| {
                    if context_rank < original_context_rank {
                        "promoted_over_support_candidate_for_task"
                    } else {
                        "deprioritized_support_candidate_for_task"
                    }
                    .to_string()
                });
            let context_rank = routing_reason.as_ref().map(|_| context_rank.unwrap());
            let symbol = original_candidate.and_then(|candidate| candidate.symbol.clone());
            let symbol_status = symbol.as_ref().map(|_| match valid_candidate {
                Some(candidate) if candidate.symbol.is_some() => "valid",
                Some(_) => "stale",
                None => "not_checked",
            });
            let (context_status, context_reason) = if !root.join(file).is_file() {
                ("omitted", "missing_file")
            } else if !indexed_files.contains(file) {
                ("omitted", "unindexed_file")
            } else if selected_files.contains(file.as_str()) {
                ("selected", "selected_within_token_budget")
            } else {
                match mode {
                    BackendContextMode::Preferred => ("omitted", "token_budget_exhausted"),
                    BackendContextMode::Fallback => ("omitted", "fallback_not_selected"),
                }
            };
            let next_action = match context_reason {
                "selected_within_token_budget" => "read_selected_context",
                "token_budget_exhausted" => "run_backend_candidate_context_pack",
                "fallback_not_selected" => "use_if_fallback_context_insufficient",
                "missing_file" => "refresh_backend_evidence",
                "unindexed_file" => "use_indexed_source_candidate",
                _ => unreachable!("backend candidate context reason is exhaustive"),
            };

            AgentRouteBackendCandidateDisposition {
                file: file.clone(),
                rank: index + 1,
                context_rank,
                routing_reason,
                symbol,
                context_status: context_status.to_string(),
                context_reason: context_reason.to_string(),
                next_action: next_action.to_string(),
                symbol_status: symbol_status.map(str::to_string),
            }
        })
        .collect()
}

fn backend_symbol_name_matches(candidate: &str, name: &str, qualified_name: &str) -> bool {
    candidate == name
        || candidate == qualified_name
        || candidate.ends_with(&format!(".{qualified_name}"))
        || qualified_name.ends_with(&format!(".{candidate}"))
}

fn backend_trace_symbol_match_score(backend_symbol: &str, local_symbol: &Symbol) -> usize {
    if backend_symbol == local_symbol.qualified_name {
        return usize::MAX;
    }

    let backend_parts = backend_symbol_identity_parts(backend_symbol);
    let local_file = Path::new(&local_symbol.file).with_extension("");
    let mut local_parts = backend_symbol_identity_parts(&local_file.to_string_lossy());
    local_parts.extend(backend_symbol_identity_parts(&local_symbol.qualified_name));
    let suffix_parts = backend_parts
        .iter()
        .rev()
        .zip(local_parts.iter().rev())
        .take_while(|(backend, local)| backend == local)
        .count();

    suffix_parts.max(1)
}

fn resolve_backend_trace_symbol(
    store: &Store,
    backend_symbol: &str,
    lookup_name: Option<&str>,
) -> Result<Option<Symbol>> {
    let lookup_name = lookup_name.unwrap_or_else(|| {
        backend_symbol
            .rsplit(['.', ':'])
            .find(|part| !part.is_empty())
            .unwrap_or(backend_symbol)
    });
    let local_symbols =
        store.search_symbols(lookup_name, BACKEND_EVIDENCE_TOOL_RESULT_ITEMS_LIMIT)?;
    Ok(local_symbols
        .into_iter()
        .enumerate()
        .filter(|(_, symbol)| {
            backend_symbol_name_matches(backend_symbol, &symbol.name, &symbol.qualified_name)
        })
        .max_by_key(|(index, symbol)| {
            (
                backend_trace_symbol_match_score(backend_symbol, symbol),
                Reverse(*index),
            )
        })
        .map(|(_, symbol)| symbol))
}

fn backend_symbol_identity_parts(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn backend_candidate_is_support_file(file: &str) -> bool {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    let path = format!("/{normalized}");
    is_low_value_reference_file(&normalized)
        || [
            "/.github/",
            "/benches/",
            "/demo/",
            "/demos/",
            "/docs/",
            "/example/",
            "/examples/",
            "/formula/",
            "/scripts/",
        ]
        .iter()
        .any(|segment| path.contains(segment))
        || normalized.starts_with("formula/")
        || normalized.contains("-smoke.")
        || normalized.contains(".smoke.")
        || normalized.contains("_smoke.")
}

fn backend_task_prefers_support_files(task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "benchmark"
                | "benchmarks"
                | "ci"
                | "demo"
                | "docs"
                | "documentation"
                | "example"
                | "examples"
                | "fixture"
                | "fixtures"
                | "formula"
                | "homebrew"
                | "integration"
                | "package"
                | "packaging"
                | "release"
                | "script"
                | "scripts"
                | "smoke"
                | "spec"
                | "specs"
                | "test"
                | "testing"
                | "tests"
                | "workflow"
        )
    })
}

fn annotate_backend_seed_context(
    context_pack: &mut ContextPack,
    backend_evidence: &AgentRouteBackendEvidence,
    candidates: &[AgentRouteBackendCandidate],
) {
    for candidate in candidates {
        let rank = backend_evidence
            .candidate_files
            .iter()
            .position(|file| file == &candidate.file)
            .map(|index| index + 1)
            .unwrap_or(0);
        let summary = format!(
            "Selected from backend {} candidate rank {rank}: {}.",
            backend_evidence.provider,
            backend_candidate_summary(candidate)
        );
        if let Some(file) = context_pack
            .files
            .iter_mut()
            .find(|file| file.file == candidate.file)
        {
            file.reason.push(' ');
            file.reason.push_str(&summary);
        }
        if let Some(step) = context_pack
            .reading_plan
            .iter_mut()
            .find(|step| step.file == candidate.file)
        {
            step.reason.push(' ');
            step.reason.push_str(&summary);
            step.selection_reason.push(' ');
            step.selection_reason.push_str(&summary);
        }
    }
}

fn backend_candidate_summary(candidate: &AgentRouteBackendCandidate) -> String {
    let mut details = vec![format!("file {}", candidate.file)];
    if let Some(symbol) = candidate.symbol.as_ref() {
        details.push(format!("symbol {symbol}"));
    }
    if let Some(source) = candidate.source.as_ref() {
        details.push(format!("source {source}"));
    }
    if let Some(score) = candidate.score {
        details.push(format!("score {score:.3}"));
    }
    if let Some(reason) = candidate.reason.as_ref() {
        details.push(format!("reason {reason}"));
    }
    if !candidate.evidence.is_empty() {
        details.push(format!("evidence {}", candidate.evidence.join(", ")));
    }
    details.join("; ")
}

fn bounded_optional_string(
    value: Option<String>,
    char_limit: usize,
    truncated_text_fields: &mut usize,
) -> Option<String> {
    value.and_then(|value| {
        let value = bounded_trimmed_string(&value, char_limit, truncated_text_fields);
        (!value.is_empty()).then_some(value)
    })
}

fn normalized_bounded_strings(
    values: Vec<String>,
    item_limit: usize,
    char_limit: usize,
    truncated_text_fields: &mut usize,
) -> (Vec<String>, usize) {
    let mut seen = BTreeSet::new();
    let mut normalized = values
        .into_iter()
        .map(|value| bounded_trimmed_string(&value, char_limit, truncated_text_fields))
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.clone()))
        .collect::<Vec<_>>();
    let omitted = normalized.len().saturating_sub(item_limit);
    normalized.truncate(item_limit);
    (normalized, omitted)
}

fn bounded_trimmed_string(
    value: &str,
    char_limit: usize,
    truncated_text_fields: &mut usize,
) -> String {
    let value = value.trim();
    if value.chars().count() <= char_limit {
        return value.to_string();
    }
    *truncated_text_fields += 1;
    value.chars().take(char_limit).collect()
}

fn backend_normalization_changed(normalization: &AgentRouteBackendNormalization) -> bool {
    normalization.unfetched_tool_result_items > 0
        || normalization.omitted_tool_result_items > 0
        || normalization.omitted_candidates > 0
        || normalization.omitted_candidate_evidence_items > 0
        || normalization.omitted_evidence_sources > 0
        || normalization.omitted_notes > 0
        || normalization.truncated_text_fields > 0
}

fn normalize_backend_candidate_file(root: &Path, file: &str) -> Result<String> {
    let normalized_separators = file.replace('\\', "/");
    let path = Path::new(&normalized_separators);
    let canonical_path;
    let relative = if path.is_absolute() {
        canonical_path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        canonical_path
            .strip_prefix(root)
            .with_context(|| format!("candidate file is outside project root: {file}"))?
    } else {
        path
    };
    let mut normalized = PathBuf::new();
    for component in relative.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            Component::ParentDir => {
                if !normalized.pop() {
                    bail!("candidate file is outside project root: {file}");
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                bail!("candidate file is outside project root: {file}");
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        bail!("backend evidence candidate file must not be empty");
    }
    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if normalized.chars().count() > BACKEND_EVIDENCE_FILE_CHARS_LIMIT {
        bail!(
            "backend evidence candidate file must not exceed {} characters",
            BACKEND_EVIDENCE_FILE_CHARS_LIMIT
        );
    }
    Ok(normalized)
}

fn agent_route_context_reason(context_pack: &ContextPack) -> String {
    let summary = format!(
        "selected {} files, {} ranges, and {} reading-plan steps within the token budget",
        context_pack.files.len(),
        context_pack.budget.selected_ranges,
        context_pack.reading_plan.len()
    );

    match context_pack.reading_plan.first() {
        Some(step) => format!(
            "{summary}; {}; {}; continuation {}",
            agent_route_first_step_summary(step),
            agent_route_omitted_summary(context_pack),
            context_pack.continuation_summary.next_action
        ),
        None => {
            format!(
                "{summary}; context selection is {}; next action {}; {}",
                context_pack.continuation_summary.status,
                context_pack.continuation_summary.next_action,
                context_pack.continuation_summary.message
            )
        }
    }
}

fn agent_route_context_status(context_pack: &ContextPack) -> String {
    if context_pack.reading_plan.is_empty() {
        context_pack.continuation_summary.status.clone()
    } else {
        "complete".to_string()
    }
}

fn agent_route_first_step_summary(step: &ContextReadingStep) -> String {
    format!(
        "read {} first (candidate rank {}) via {}, use {} when deeper evidence is needed",
        step.file, step.selection_rank, step.next_action, step.suggested_tool.tool
    )
}

fn agent_route_omitted_summary(context_pack: &ContextPack) -> String {
    if let Some(candidate) = context_pack.omitted_candidates.first() {
        return format!(
            "first omitted candidate {} (candidate rank {}, reason {}) can be revisited via {} using {} after selected context",
            candidate.file,
            candidate.selection_rank,
            candidate.omission_reason,
            candidate.next_action,
            candidate.suggested_tool.tool
        );
    }

    if context_pack.budget.omitted_files > 0 || context_pack.budget.omitted_ranges > 0 {
        return format!(
            "{} lower-ranked files and {} ranges were omitted without a focused candidate; next action {}",
            context_pack.budget.omitted_files,
            context_pack.budget.omitted_ranges,
            context_pack.continuation_summary.next_action
        );
    }

    "no omitted candidate follow-up is needed before the selected context is read".to_string()
}

fn agent_route_continuation_instruction(context_pack: &ContextPack) -> String {
    let continuation = &context_pack.continuation_summary;
    if let Some(candidate) = context_pack.omitted_candidates.first() {
        return format!(
            "Use continuation_summary only after selected context has been read. First omitted candidate is {} (candidate rank {}, reason {}); next_action {} with suggested tool {}.",
            candidate.file,
            candidate.selection_rank,
            candidate.omission_reason,
            candidate.next_action,
            candidate.suggested_tool.tool
        );
    }

    if continuation.status == "complete" {
        return format!(
            "Use continuation_summary only after selected context has been read. Current continuation status is complete; no follow-up tool is required after selected context. next_action {} means read the selected context first.",
            continuation.next_action
        );
    }

    format!(
        "Use continuation_summary only after selected context has been read. Current continuation status is {} with next_action {}. Message: {}",
        continuation.status, continuation.next_action, continuation.message
    )
}

fn add_index_scope_hint_to_blocked_context(
    context_pack: &mut ContextPack,
    index_scope: &IndexScopeReport,
) {
    if !index_scope.enabled
        || !context_pack.reading_plan.is_empty()
        || !context_pack
            .continuation_summary
            .status
            .starts_with("blocked_")
    {
        return;
    }

    let hint = format!(
        "Index scope is enabled; includes: {}; excludes: {}; walk_roots: {}. If the expected code is outside this scope, update .codeinsight/config.toml and rerun index_project or agent_route with force_index.",
        summarize_index_scope_values(&index_scope.includes),
        summarize_index_scope_values(&index_scope.excludes),
        summarize_index_scope_values(&index_scope.walk_roots),
    );
    if !context_pack
        .continuation_summary
        .message
        .contains("Index scope is enabled")
    {
        context_pack.continuation_summary.message =
            format!("{} {}", context_pack.continuation_summary.message, hint);
    }
    context_pack.continuation_summary.next_action =
        match context_pack.continuation_summary.status.as_str() {
            "blocked_no_context" => "check_index_scope_or_provide_matching_seed".to_string(),
            "blocked_no_seed" => "check_index_scope_or_provide_seed".to_string(),
            _ => context_pack.continuation_summary.next_action.clone(),
        };
}

fn summarize_index_scope_values(values: &[String]) -> String {
    if values.is_empty() {
        "-".to_string()
    } else {
        values.join(", ")
    }
}

fn agent_route_impact_reason(report: &ImpactAnalysisReport) -> String {
    format!(
        "after selected context is read, pre-edit impact check estimated {} impacted files at {} risk, including {} call-related files, {} dependency-related files, {} call paths, and {} dependency paths",
        report.impact_counts.impacted_files,
        report.risk_level,
        report.impact_breakdown.call_related_files,
        report.impact_breakdown.dependency_related_files,
        report.impact_breakdown.call_paths,
        report.impact_breakdown.dependency_paths
    )
}

fn agent_route_skipped_impact_status(context_status: &str) -> &'static str {
    match context_status {
        "blocked_invalid_seed" => "skipped_invalid_seed",
        "blocked_no_context" => "skipped_no_context",
        "blocked_unindexed_task_path" => "skipped_unindexed_task_path",
        _ => "skipped_no_seed",
    }
}

fn agent_route_skipped_impact_reason(impact_status: &str) -> String {
    match impact_status {
        "deferred_by_request" => {
            "deferred for the fast first read and required before edits".to_string()
        }
        "skipped_invalid_seed" => {
            "skipped because the explicit seed file could not be resolved".to_string()
        }
        "skipped_no_context" => {
            "skipped because the explicit seed did not match any readable context".to_string()
        }
        "skipped_unindexed_task_path" => {
            "skipped because the task path seed is not indexed".to_string()
        }
        _ => "skipped because no context file or symbol seed was available".to_string(),
    }
}

fn agent_route_skipped_impact_instruction(impact_status: &str) -> String {
    match impact_status {
        "deferred_by_request" => {
            "Impact analysis was deferred for the fast first read; call the suggested impact_analysis tool after reading selected context and before editing."
                .to_string()
        }
        "skipped_invalid_seed" => {
            "Impact analysis was skipped because the explicit seed file could not be resolved; provide an existing seed file or symbol before editing.".to_string()
        }
        "skipped_no_context" => {
            "Impact analysis was skipped because the explicit seed did not match any readable context; provide a matching seed file or symbol before editing.".to_string()
        }
        "skipped_unindexed_task_path" => {
            "Impact analysis was skipped because the task path seed is not indexed; update the index scope or pass an indexed seed before editing.".to_string()
        }
        _ => "Impact analysis was skipped because no file or symbol seed was available."
            .to_string(),
    }
}

fn agent_route_impact_instruction(report: &ImpactAnalysisReport) -> String {
    let mut instruction = format!(
        "Before editing, review impact_analysis: {} impacted files at {} risk.",
        report.impact_counts.impacted_files, report.risk_level
    );
    if let Some(check) = report.suggested_checks.first() {
        instruction.push_str(" First suggested check: ");
        instruction.push_str(&suggested_check_instruction(check));
        instruction.push('.');
    }
    instruction
}

fn suggested_check_instruction(check: &SuggestedCheck) -> String {
    let reason = check.reason.trim_end_matches('.');
    if let Some(command) = &check.command {
        return format!("run {command} because {reason}");
    }
    if let Some(file) = &check.file {
        return format!("review {file} because {reason}");
    }
    format!("review impact result because {reason}")
}

fn agent_route_impact_suggested_tool(report: &ImpactAnalysisReport) -> ContextSuggestedTool {
    ContextSuggestedTool {
        tool: "impact_analysis".to_string(),
        priority: 80,
        reason: "Open the full impact analysis before editing selected context.".to_string(),
        suggested_arguments: json!({
            "root": &report.root,
            "symbols": &report.seed_symbols,
            "files": &report.seed_files,
            "limit": report.impacted_files.len().max(10),
            "depth": report.depth,
            "format": "full",
            "evidence_limit": report.evidence_limit
        }),
    }
}

fn agent_route_deferred_impact_suggested_tool(
    root: &Path,
    symbols: &[String],
    files: &[String],
    limit: usize,
    depth: usize,
    evidence_limit: usize,
) -> ContextSuggestedTool {
    ContextSuggestedTool {
        tool: "impact_analysis".to_string(),
        priority: 90,
        reason: "Run the deferred impact check after reading selected context and before editing."
            .to_string(),
        suggested_arguments: json!({
            "root": root.display().to_string(),
            "symbols": symbols,
            "files": files,
            "limit": limit,
            "depth": depth,
            "format": "summary",
            "evidence_limit": evidence_limit
        }),
    }
}

fn is_context_pack_no_seed_error(error: &anyhow::Error) -> bool {
    error.to_string() == CONTEXT_PACK_NO_SEED_ERROR
}

fn is_context_pack_invalid_seed_error(error: &anyhow::Error) -> bool {
    let error = error.to_string();
    error.starts_with("failed to resolve seed file:")
        || error.starts_with("seed file is outside project root:")
}

fn empty_context_pack_for_blocked_route(
    task: String,
    requested_token_budget: usize,
    baseline_source_lines: usize,
) -> ContextPack {
    let applied_token_budget = requested_token_budget.max(500);
    let estimated_tokens = estimate_tokens(&task);
    ContextPack {
        task,
        summary: "Context pack could not infer source seed files; provide --file/--symbol or add source files before broad reading.".to_string(),
        seed_strategy: "auto_no_seed".to_string(),
        selected_seeds: Vec::new(),
        reading_plan: Vec::new(),
        semantic_status: ContextSemanticStatus {
            provider: "disabled".to_string(),
            model: "disabled".to_string(),
            provider_configured: false,
            vector_status: "skipped_no_seed".to_string(),
            vector_candidates: 0,
            fallback_candidates: 0,
            selected_vector_ranges: 0,
            selected_fallback_ranges: 0,
            recommendation: "provide a source seed before semantic context expansion".to_string(),
        },
        budget: ContextBudget {
            requested_token_budget,
            applied_token_budget,
            estimated_tokens,
            candidate_files: 0,
            selected_files: 0,
            omitted_files: 0,
            candidate_ranges: 0,
            selected_ranges: 0,
            omitted_ranges: 0,
            truncated: false,
            truncation_reason: "no_seed_available".to_string(),
        },
        read_less: context_read_less(baseline_source_lines, 0),
        continuation_summary: ContextContinuationSummary {
            status: "blocked_no_seed".to_string(),
            message: "No source seed was available for context selection; provide --file/--symbol or add source files.".to_string(),
            next_action: "provide_seed_file_or_symbol".to_string(),
            omitted_candidate_count: 0,
            first_omitted_file: None,
            suggested_tool: None,
        },
        omitted_candidates: Vec::new(),
        files: Vec::new(),
        symbols: Vec::new(),
        references: Vec::new(),
        estimated_tokens,
        truncated: false,
    }
}

fn empty_context_pack_for_invalid_seed_route(
    task: String,
    requested_token_budget: usize,
    baseline_source_lines: usize,
    seed_files: &[String],
    error_message: String,
) -> ContextPack {
    let applied_token_budget = requested_token_budget.max(500);
    let estimated_tokens = estimate_tokens(&task);
    ContextPack {
        task,
        summary: format!(
            "Context pack could not resolve an explicit seed file: {error_message}. Provide an existing --file path under the project root or use --symbol."
        ),
        seed_strategy: "explicit_invalid_seed".to_string(),
        selected_seeds: explicit_context_seeds(&[], seed_files, &BTreeMap::new()),
        reading_plan: Vec::new(),
        semantic_status: ContextSemanticStatus {
            provider: "disabled".to_string(),
            model: "disabled".to_string(),
            provider_configured: false,
            vector_status: "skipped_invalid_seed".to_string(),
            vector_candidates: 0,
            fallback_candidates: 0,
            selected_vector_ranges: 0,
            selected_fallback_ranges: 0,
            recommendation: "provide an existing source seed before semantic context expansion"
                .to_string(),
        },
        budget: ContextBudget {
            requested_token_budget,
            applied_token_budget,
            estimated_tokens,
            candidate_files: 0,
            selected_files: 0,
            omitted_files: 0,
            candidate_ranges: 0,
            selected_ranges: 0,
            omitted_ranges: 0,
            truncated: false,
            truncation_reason: "invalid_seed_file".to_string(),
        },
        read_less: context_read_less(baseline_source_lines, 0),
        continuation_summary: ContextContinuationSummary {
            status: "blocked_invalid_seed".to_string(),
            message: format!(
                "Explicit seed file could not be resolved: {error_message}. Provide an existing --file path under the project root or use --symbol."
            ),
            next_action: "provide_existing_seed_file_or_symbol".to_string(),
            omitted_candidate_count: 0,
            first_omitted_file: None,
            suggested_tool: None,
        },
        omitted_candidates: Vec::new(),
        files: Vec::new(),
        symbols: Vec::new(),
        references: Vec::new(),
        estimated_tokens,
        truncated: false,
    }
}

fn empty_context_pack_for_unindexed_task_path(
    task: String,
    requested_token_budget: usize,
    baseline_source_lines: usize,
    selected_seeds: Vec<ContextSeed>,
) -> ContextPack {
    let applied_token_budget = requested_token_budget.max(500);
    let estimated_tokens = estimate_tokens(&task);
    let task_paths = selected_seeds
        .iter()
        .map(|seed| seed.value.clone())
        .collect::<Vec<_>>();
    ContextPack {
        task,
        summary: format!(
            "Context pack could not use task path seed files because they are not indexed: {}.",
            task_paths.join(", ")
        ),
        seed_strategy: "auto_task_path_unindexed".to_string(),
        selected_seeds,
        reading_plan: Vec::new(),
        semantic_status: ContextSemanticStatus {
            provider: "disabled".to_string(),
            model: "disabled".to_string(),
            provider_configured: false,
            vector_status: "skipped_unindexed_task_path".to_string(),
            vector_candidates: 0,
            fallback_candidates: 0,
            selected_vector_ranges: 0,
            selected_fallback_ranges: 0,
            recommendation: "index the task path or update the configured index scope before semantic context expansion".to_string(),
        },
        budget: ContextBudget {
            requested_token_budget,
            applied_token_budget,
            estimated_tokens,
            candidate_files: 0,
            selected_files: 0,
            omitted_files: 0,
            candidate_ranges: 0,
            selected_ranges: 0,
            omitted_ranges: 0,
            truncated: false,
            truncation_reason: "unindexed_task_path".to_string(),
        },
        read_less: context_read_less(baseline_source_lines, 0),
        continuation_summary: ContextContinuationSummary {
            status: "blocked_unindexed_task_path".to_string(),
            message: format!(
                "Task path seed files are not indexed: {}. Run index_project with a scope that includes them, or pass a different indexed --file/--symbol.",
                task_paths.join(", ")
            ),
            next_action: "index_or_update_scope_for_task_path".to_string(),
            omitted_candidate_count: 0,
            first_omitted_file: None,
            suggested_tool: None,
        },
        omitted_candidates: Vec::new(),
        files: Vec::new(),
        symbols: Vec::new(),
        references: Vec::new(),
        estimated_tokens,
        truncated: false,
    }
}

pub fn init_config_value(root: PathBuf, force: bool) -> Result<ConfigInitReport> {
    let root = root.canonicalize()?;
    let (path, overwritten) = init_project_config(&root, force)?;
    Ok(ConfigInitReport {
        root: root.display().to_string(),
        path: path.display().to_string(),
        created: !overwritten,
        overwritten,
    })
}

pub fn config_status_value(root: PathBuf) -> Result<ConfigStatusReport> {
    let root = root.canonicalize()?;
    let path = root.join(project_config_path());
    let exists = path.exists();
    let detected_test_commands = suggested_test_commands_for_root(&root);

    let (
        loaded,
        parse_error,
        configured_test_commands,
        configured_suggested_checks,
        configured_package_conditions,
        configured_index_includes,
        configured_index_excludes,
    ) = if exists {
        match load_project_config(&root) {
            Ok(Some(config)) => (
                true,
                None,
                config.impact_analysis.test_commands,
                config.impact_analysis.suggested_checks.len(),
                config.javascript.package_conditions,
                config.index.include,
                config.index.exclude,
            ),
            Ok(None) => (
                false,
                None,
                Vec::new(),
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            Err(error) => (
                false,
                Some(error.to_string()),
                Vec::new(),
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
        }
    } else {
        (
            false,
            None,
            Vec::new(),
            0,
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };
    let commands_override_builtin =
        loaded && (!configured_test_commands.is_empty() || configured_suggested_checks > 0);

    Ok(ConfigStatusReport {
        root: root.display().to_string(),
        path: path.display().to_string(),
        exists,
        loaded,
        parse_error,
        configured_test_commands,
        configured_suggested_checks,
        configured_package_conditions,
        configured_index_includes,
        configured_index_excludes,
        detected_test_commands,
        commands_override_builtin,
    })
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

pub fn file_outline_for_locations_value(
    path: PathBuf,
    locations: &[ContextSeedLocation],
) -> Result<Vec<Symbol>> {
    let symbols = file_outline_value(path)?;
    if locations.is_empty() {
        return Ok(symbols);
    }

    Ok(symbols
        .into_iter()
        .filter(|symbol| {
            locations.iter().any(|location| {
                symbol.start_line <= location.end_line && symbol.end_line >= location.start_line
            })
        })
        .collect())
}

pub fn dependency_graph_value(
    root: PathBuf,
    files: Vec<String>,
    languages: Vec<String>,
    kinds: Vec<String>,
    limit: usize,
    offset: usize,
) -> Result<DependencyGraph> {
    let root = root.canonicalize()?;
    let files = files
        .iter()
        .map(|file| normalize_seed_file(&root, file))
        .collect::<Result<Vec<_>>>()?;
    let languages = normalize_dependency_languages(&languages)?;
    let kinds = normalize_dependency_kinds(&kinds)?;
    let store = Store::open(&root)?;
    store.dependency_graph(&root, limit, offset, &files, &languages, &kinds)
}

pub fn impact_analysis_value(
    root: PathBuf,
    seed_symbols: Vec<String>,
    seed_files: Vec<String>,
    limit: usize,
    depth: usize,
    format: String,
    evidence_limit: usize,
) -> Result<ImpactAnalysisReport> {
    let root = root.canonicalize()?;
    let limit = limit.max(1);
    let depth = depth.max(1);
    let format = normalize_impact_format(&format)?;
    let evidence_limit = evidence_limit.max(1);
    let store = Store::open(&root)?;
    let indexed_files = store.indexed_files()?;
    if seed_symbols.is_empty() && seed_files.is_empty() {
        bail!("impact_analysis requires at least one --symbol or --file seed")
    }

    let indexed_file_set = indexed_files.iter().cloned().collect::<BTreeSet<_>>();
    let mut errors = Vec::new();
    let mut normalized_seed_files = Vec::new();
    for file in seed_files {
        match normalize_seed_file(&root, &file) {
            Ok(normalized) if indexed_file_set.contains(&normalized) => {
                normalized_seed_files.push(normalized);
            }
            Ok(normalized) => {
                errors.push(IndexError {
                    file: normalized,
                    stage: "impact_seed_file".to_string(),
                    message: "file is not present in the current index".to_string(),
                });
            }
            Err(error) => {
                errors.push(IndexError {
                    file,
                    stage: "impact_seed_file".to_string(),
                    message: error.to_string(),
                });
            }
        }
    }
    normalized_seed_files.sort();
    normalized_seed_files.dedup();

    let mut impact = BTreeMap::<String, (i32, BTreeSet<String>)>::new();
    for file in &normalized_seed_files {
        add_impact(&mut impact, file, IMPACT_SCORE_SEED_FILE, "seed_file");
    }

    let mut symbols = Vec::new();
    let mut symbol_terms = seed_symbols.iter().cloned().collect::<BTreeSet<_>>();
    for seed in &seed_symbols {
        let matches = store.search_symbols(seed, limit)?;
        for symbol in matches {
            add_impact(
                &mut impact,
                &symbol.file,
                IMPACT_SCORE_SYMBOL_DEFINITION,
                format!("symbol_definition:{}", symbol.qualified_name),
            );
            symbol_terms.insert(symbol.name.clone());
            symbol_terms.insert(symbol.qualified_name.clone());
            symbols.push(symbol);
        }
    }

    let file_symbols = store.symbols_for_files(
        &normalized_seed_files,
        impact_file_symbol_scan_limit(limit, normalized_seed_files.len()),
    )?;
    for symbol in file_symbols {
        add_impact(
            &mut impact,
            &symbol.file,
            IMPACT_SCORE_SEED_FILE_SYMBOL,
            format!("seed_file_symbol:{}", symbol.qualified_name),
        );
        symbol_terms.insert(symbol.name.clone());
        symbol_terms.insert(symbol.qualified_name.clone());
        symbols.push(symbol);
    }
    dedup_symbols(&mut symbols);

    let mut references = Vec::new();
    let mut callers = Vec::new();
    let mut callees = Vec::new();
    for term in symbol_terms.iter().filter(|term| !term.trim().is_empty()) {
        if references.len() < limit {
            let remaining = limit.saturating_sub(references.len());
            references.extend(find_references_value(root.clone(), term, remaining, false)?);
        }
        if callers.len() < limit {
            let remaining = limit.saturating_sub(callers.len());
            callers.extend(store.callers(term, remaining)?);
        }
        if callees.len() < limit {
            let remaining = limit.saturating_sub(callees.len());
            callees.extend(store.callees(term, remaining)?);
        }
    }
    dedup_references(&mut references);
    dedup_calls(&mut callers);
    dedup_calls(&mut callees);

    for reference in &references {
        add_impact(
            &mut impact,
            &reference.file,
            IMPACT_SCORE_REFERENCE,
            format!("reference:{}", reference.context),
        );
    }
    for call in &callers {
        add_impact(
            &mut impact,
            &call.file,
            IMPACT_SCORE_CALLER,
            format!("caller:{}->{}", call.caller, call.callee),
        );
    }
    let mut paths = impact_call_paths(
        &store,
        &symbol_terms,
        &mut callers,
        &mut impact,
        depth,
        limit,
    )?;
    let mut seen_call_paths = paths
        .iter()
        .filter(|path| path.kind == "call")
        .map(|path| (path.from.clone(), path.to.clone(), path.depth, path.line))
        .collect::<BTreeSet<_>>();
    for call in &callees {
        add_impact(
            &mut impact,
            &call.file,
            IMPACT_SCORE_CALLEE_SOURCE,
            format!("callee_source:{}->{}", call.caller, call.callee),
        );
        if let Some(callee_file) = &call.callee_file {
            add_impact(
                &mut impact,
                callee_file,
                IMPACT_SCORE_CALLEE_TARGET,
                format!("callee_target:{}->{}", call.caller, call.callee),
            );
        }
        push_downstream_call_path(&mut paths, &mut seen_call_paths, call, 1, limit);
    }
    dedup_calls(&mut callers);

    let mut dependency_seed_files = normalized_seed_files.clone();
    dependency_seed_files.extend(symbols.iter().map(|symbol| symbol.file.clone()));
    dependency_seed_files.sort();
    dependency_seed_files.dedup();
    let mut dependencies = store.dependencies_touching_files(&dependency_seed_files, limit)?;
    for dependency in &dependencies {
        add_impact(
            &mut impact,
            &dependency.source_file,
            IMPACT_SCORE_DEPENDENCY_SOURCE,
            format!("dependency_source:{}", dependency.target),
        );
        if let Some(resolved_file) = &dependency.resolved_file {
            add_impact(
                &mut impact,
                resolved_file,
                IMPACT_SCORE_DEPENDENCY_TARGET,
                format!("dependency_target:{}", dependency.source_file),
            );
        }
    }
    let type_relation_terms = symbol_terms.iter().cloned().collect::<Vec<_>>();
    let type_relation_importers =
        store.type_relation_importers_for_symbols(&type_relation_terms, limit)?;
    let mut seen_type_relation_paths = BTreeSet::new();
    for dependency in &type_relation_importers {
        if !is_type_relation_dependency(dependency) {
            continue;
        }
        let relation = dependency.imported_symbol.as_deref().unwrap_or("extends");
        let target_file = impact_type_relation_target_file(&symbols, dependency)
            .unwrap_or_else(|| dependency.target.clone());
        add_impact(
            &mut impact,
            &dependency.source_file,
            IMPACT_SCORE_TYPE_RELATION_SOURCE,
            format!(
                "type_relation_source:{}:{}",
                dependency
                    .local_alias
                    .as_deref()
                    .unwrap_or(&dependency.source_file),
                dependency.target
            ),
        );
        push_type_relation_path(
            &mut paths,
            &mut seen_type_relation_paths,
            dependency,
            &target_file,
            relation,
            1,
            limit,
        );
        if dependencies.len() < limit {
            dependencies.push(dependency.clone());
        }
    }
    let mut dependency_paths = impact_dependency_paths(
        &store,
        &dependency_seed_files,
        &mut dependencies,
        &mut impact,
        depth,
        limit,
    )?;
    paths.append(&mut dependency_paths);
    paths.truncate(limit);

    let mut ranked_impacted_files = impact
        .into_iter()
        .map(|(file, (score, reasons))| (file, score, reasons))
        .collect::<Vec<_>>();
    ranked_impacted_files
        .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    ranked_impacted_files.truncate(limit);

    let impact_breakdown = impact_breakdown_from_reason_sets(
        ranked_impacted_files
            .iter()
            .map(|(_file, _score, reasons)| reasons),
        &paths,
        errors.len(),
    );
    let impacted_files = ranked_impacted_files
        .into_iter()
        .map(|(file, score, reasons)| ImpactFile {
            file,
            score,
            reasons: reasons.into_iter().take(8).collect(),
        })
        .collect::<Vec<_>>();

    let risk_level = impact_risk_level(&impacted_files, &paths);
    let impact_counts = ImpactCounts {
        impacted_files: impacted_files.len(),
        paths: paths.len(),
        symbols: symbols.len(),
        references: references.len(),
        callers: callers.len(),
        callees: callees.len(),
        dependencies: dependencies.len(),
        errors: errors.len(),
    };
    let summary = format!(
        "Impact analysis found {} impacted files from {} symbol seeds and {} file seeds, including {} call-related files, {} dependency-related files, {} call paths, and {} dependency paths.",
        impacted_files.len(),
        seed_symbols.len(),
        normalized_seed_files.len(),
        impact_breakdown.call_related_files,
        impact_breakdown.dependency_related_files,
        impact_breakdown.call_paths,
        impact_breakdown.dependency_paths
    );
    let top_reasons = impact_top_reasons(&impacted_files, 8);
    let suggested_checks =
        impact_suggested_checks(&root, &risk_level, &impacted_files, &paths, &errors)?;

    if format == "summary" {
        symbols.truncate(evidence_limit);
        references.truncate(evidence_limit);
        callers.truncate(evidence_limit);
        callees.truncate(evidence_limit);
        dependencies.truncate(evidence_limit);
    }

    Ok(ImpactAnalysisReport {
        root: root.display().to_string(),
        summary,
        risk_level,
        impact_counts,
        impact_breakdown,
        top_reasons,
        suggested_checks,
        depth,
        format,
        evidence_limit,
        seed_symbols,
        seed_files: normalized_seed_files,
        impacted_files,
        paths,
        symbols,
        references,
        callers,
        callees,
        dependencies,
        errors,
    })
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
    let mut candidates = Vec::new();

    for relative_path in files {
        let path = root.join(&relative_path);
        let Ok(source) = fs::read_to_string(path) else {
            continue;
        };

        let mut scanner = ReferenceLineScanner::default();
        for (line_index, line) in source.lines().enumerate() {
            let code_mask = scanner.code_mask(line);
            if !include_definitions && looks_like_definition(line, symbol) {
                continue;
            }

            for column in symbol_columns(line, symbol) {
                if !is_code_reference_column(&code_mask, column, symbol.len()) {
                    continue;
                }

                let reference_kind = classify_reference(line, symbol).to_string();
                let confidence =
                    confidence_for_reference(line, symbol, &relative_path, &reference_kind);
                candidates.push(ReferenceCandidate {
                    score: reference_candidate_score(&relative_path, &reference_kind, confidence),
                    reference: ReferenceMatch {
                        file: relative_path.clone(),
                        line: line_index + 1,
                        column: column + 1,
                        context: line.trim().to_string(),
                        reference_kind,
                        confidence,
                    },
                });
            }
        }
    }

    candidates.sort_by(compare_reference_candidates);
    candidates.truncate(limit.max(1));
    Ok(candidates
        .into_iter()
        .map(|candidate| candidate.reference)
        .collect())
}

pub fn semantic_search_value(
    root: PathBuf,
    query: &str,
    limit: usize,
) -> Result<Vec<SemanticSearchResult>> {
    let root = root.canonicalize()?;
    let provider = embedding::provider_from_env()?;
    let query_embedding = embedding::embed_query(provider.as_ref(), query)?;
    let store = Store::open(&root)?;
    let mut matches = store
        .semantic_embedding_matches(provider.provider_name(), provider.model_name())?
        .into_iter()
        .filter_map(|candidate| semantic_search_result(candidate, &query_embedding.values))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        bail!(
            "semantic search index is empty for provider '{}' model '{}'; run semantic-index with {PROVIDER_ENV}={}. {}",
            provider.provider_name(),
            provider.model_name(),
            provider.provider_name(),
            embedding::provider_help(),
            PROVIDER_ENV = embedding::PROVIDER_ENV
        )
    }

    matches.sort_by(compare_semantic_search_results);
    matches.truncate(limit.max(1));
    Ok(matches)
}

pub fn semantic_index_value(
    root: PathBuf,
    chunk_lines: usize,
    explain: bool,
) -> Result<SemanticIndexReport> {
    let root = root.canonicalize()?;
    let chunk_lines = chunk_lines.max(1);
    let mut store = Store::open(&root)?;
    let files = store.indexed_files()?;
    if files.is_empty() {
        bail!("semantic_index requires an existing project index; run index first")
    }

    let mut chunks = Vec::new();
    let mut errors = Vec::new();
    for file in &files {
        let path = root.join(file);
        match fs::read_to_string(&path) {
            Ok(source) => chunks.extend(semantic_chunks_for_file(file, &source, chunk_lines)),
            Err(error) => errors.push(IndexError {
                file: file.clone(),
                stage: "semantic_read".to_string(),
                message: error.to_string(),
            }),
        }
    }

    let chunk_stats = store.replace_semantic_chunks(&chunks, explain)?;
    let provider = embedding::provider_from_env()?;
    let mut embeddings_generated = 0;
    if provider.is_configured() && chunk_stats.total > 0 {
        let stored_chunks = store
            .semantic_chunks_missing_embeddings(provider.provider_name(), provider.model_name())?;
        embeddings_generated = stored_chunks.len();
        let batch_size = embedding::batch_size_from_env()?;
        let semantic_embeddings =
            semantic_embeddings_for_chunks(provider.as_ref(), &stored_chunks, batch_size)?;
        store.upsert_semantic_embeddings(
            provider.provider_name(),
            provider.model_name(),
            &semantic_embeddings,
        )?;
    }
    let embeddings = if provider.is_configured() {
        store.count_semantic_embeddings_for(provider.provider_name(), provider.model_name())?
    } else {
        0
    };
    let embeddings_reused = if provider.is_configured() {
        embeddings.saturating_sub(embeddings_generated)
    } else {
        0
    };

    Ok(SemanticIndexReport {
        root: root.display().to_string(),
        indexed_files: files.len(),
        chunks: chunk_stats.total,
        chunks_added: chunk_stats.added,
        chunks_updated: chunk_stats.updated,
        chunks_removed: chunk_stats.removed,
        embeddings,
        embeddings_generated,
        embeddings_reused,
        chunk_lines,
        provider: provider.provider_name().to_string(),
        vector_status: if embeddings == 0 {
            "chunks_indexed_without_embeddings".to_string()
        } else {
            "embeddings_indexed".to_string()
        },
        errors,
        changes: explain.then_some(chunk_stats.changes),
    })
}

pub fn embedding_status_value(root: Option<PathBuf>) -> Result<EmbeddingProviderStatus> {
    let config = embedding::provider_config_from_env()?;
    let batch_size = embedding::batch_size_from_env()?;
    let index = match root {
        Some(root) => {
            let root = root.canonicalize()?;
            let store = Store::open(&root)?;
            let chunks = store.count_semantic_chunks()?;
            let embeddings =
                store.count_semantic_embeddings_for(&config.provider_name, &config.model_name)?;
            Some(SemanticIndexStatus {
                root: root.display().to_string(),
                chunks,
                embeddings,
                vector_status: semantic_vector_status(config.configured, chunks, embeddings)
                    .to_string(),
            })
        }
        None => None,
    };

    Ok(EmbeddingProviderStatus {
        provider: config.provider_name,
        model: config.model_name,
        configured: config.configured,
        source: config.source,
        provider_env: embedding::PROVIDER_ENV.to_string(),
        supported_providers: embedding::SUPPORTED_PROVIDER_NAMES
            .iter()
            .map(|provider| (*provider).to_string())
            .collect(),
        batch_size,
        batch_size_env: embedding::BATCH_SIZE_ENV.to_string(),
        help: embedding::provider_help(),
        ollama: config.ollama.map(|ollama| OllamaEmbeddingStatus {
            base_url: ollama.base_url,
            base_url_env: embedding::OLLAMA_BASE_URL_ENV.to_string(),
            model_env: embedding::OLLAMA_MODEL_ENV.to_string(),
            timeout_secs: ollama.timeout_secs,
            timeout_secs_env: embedding::OLLAMA_TIMEOUT_SECS_ENV.to_string(),
        }),
        openai: config.openai.map(|openai| OpenAiEmbeddingStatus {
            base_url: openai.base_url,
            base_url_env: embedding::OPENAI_BASE_URL_ENV.to_string(),
            api_key_env: embedding::OPENAI_API_KEY_ENV.to_string(),
            api_key_configured: openai.api_key_configured,
            model_env: embedding::OPENAI_MODEL_ENV.to_string(),
            timeout_secs: openai.timeout_secs,
            timeout_secs_env: embedding::OPENAI_TIMEOUT_SECS_ENV.to_string(),
        }),
        index,
    })
}

fn semantic_vector_status(configured: bool, chunks: usize, embeddings: usize) -> &'static str {
    if chunks == 0 {
        "semantic_chunks_missing"
    } else if !configured {
        "provider_not_configured"
    } else if embeddings == 0 {
        "embeddings_missing_for_provider"
    } else {
        "embeddings_indexed"
    }
}

fn semantic_embeddings_for_chunks(
    provider: &dyn embedding::EmbeddingProvider,
    chunks: &[SemanticChunk],
    batch_size: usize,
) -> Result<Vec<SemanticEmbeddingInput>> {
    let batch_size = batch_size.max(1);
    let mut semantic_embeddings = Vec::with_capacity(chunks.len());

    for (batch_index, chunk_batch) in chunks.chunks(batch_size).enumerate() {
        let inputs = chunk_batch
            .iter()
            .map(|chunk| chunk.text.clone())
            .collect::<Vec<_>>();
        let embeddings = provider
            .embed(&inputs)
            .with_context(|| format!("embedding provider failed in batch {}", batch_index + 1))?;
        if embeddings.len() != chunk_batch.len() {
            bail!(
                "embedding provider returned {} vectors for {} chunks in batch {}",
                embeddings.len(),
                chunk_batch.len(),
                batch_index + 1
            );
        }

        semantic_embeddings.extend(
            chunk_batch
                .iter()
                .zip(embeddings)
                .map(|(chunk, embedding)| SemanticEmbeddingInput {
                    chunk_id: chunk.id,
                    vector: embedding.values,
                }),
        );
    }

    Ok(semantic_embeddings)
}

fn semantic_search_result(
    candidate: SemanticEmbeddingMatch,
    query_embedding: &[f32],
) -> Option<SemanticSearchResult> {
    let score = cosine_similarity(query_embedding, &candidate.vector)?;
    let excerpt = excerpt_chunk_text(&candidate.chunk);
    Some(SemanticSearchResult {
        file: candidate.chunk.file,
        start_line: candidate.chunk.start_line,
        end_line: candidate.chunk.end_line,
        score,
        excerpt,
    })
}

fn compare_semantic_search_results(
    left: &SemanticSearchResult,
    right: &SemanticSearchResult,
) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.file.cmp(&right.file))
        .then_with(|| left.start_line.cmp(&right.start_line))
        .then_with(|| left.end_line.cmp(&right.end_line))
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f64> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }

    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (left, right) in left.iter().zip(right) {
        let left = *left as f64;
        let right = *right as f64;
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        return None;
    }
    Some(dot / (left_norm.sqrt() * right_norm.sqrt()))
}

fn excerpt_chunk_text(chunk: &SemanticChunk) -> String {
    chunk
        .text
        .lines()
        .enumerate()
        .map(|(index, line)| format!("{:>4}: {}", chunk.start_line + index, line))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn context_pack_value(
    root: PathBuf,
    task: String,
    seed_symbols: Vec<String>,
    mut seed_files: Vec<String>,
    token_budget: usize,
) -> Result<ContextPack> {
    let root = root.canonicalize()?;
    let budget = token_budget.max(500);
    let task_keywords = task_keywords(&task);
    let auto_seeded = seed_symbols.is_empty() && seed_files.is_empty();
    let store = Store::open(&root)?;
    let mut seed_strategy = "explicit".to_string();
    let mut selected_seeds = Vec::new();
    let mut task_path_locations = BTreeMap::new();
    if auto_seeded {
        let auto_selection = auto_context_seed_files(&store, &root, &task, &task_keywords)?;
        seed_strategy = auto_selection.strategy;
        seed_files = auto_selection.files;
        selected_seeds = auto_selection.seeds;
        task_path_locations = auto_selection.task_path_locations;
        if seed_files.is_empty() {
            if seed_strategy == "auto_task_path_unindexed" {
                let baseline_source_lines = store.overview(&root)?.total_lines;
                return Ok(empty_context_pack_for_unindexed_task_path(
                    task,
                    token_budget,
                    baseline_source_lines,
                    selected_seeds,
                ));
            }
            bail!(CONTEXT_PACK_NO_SEED_ERROR);
        }
    }

    let mut symbols = Vec::new();
    let mut references = Vec::new();
    if auto_seeded {
        seed_files = seed_files
            .iter()
            .map(|file| normalize_seed_file(&root, file))
            .collect::<Result<Vec<_>>>()?;
    } else {
        let (normalized_files, explicit_locations) =
            normalize_explicit_seed_files(&root, &seed_files)?;
        seed_files = normalized_files;
        task_path_locations = explicit_locations;
        selected_seeds = explicit_context_seeds(&seed_symbols, &seed_files, &task_path_locations);
    }
    let seed_file_set = seed_files.iter().cloned().collect::<BTreeSet<_>>();
    let seed_file_order = seed_files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let scoring_policy = ContextScoringPolicy {
        prefer_low_value_files: context_prefers_low_value_files(&task_keywords, &seed_files),
        prefer_agent_first_read_source_files: auto_seed_agent_first_read_task(&task_keywords)
            && !auto_seed_agent_first_read_evidence_task(&task_keywords),
        prefer_indexing_pipeline_source_files: auto_seed_indexing_pipeline_task(&task_keywords),
        prefer_data_persistence_source_files: auto_seed_data_persistence_task(&task_keywords),
        prefer_semantic_context_source_files: auto_seed_semantic_context_task(&task_keywords),
        prefer_semantic_context_orchestration_files:
            auto_seed_semantic_context_prefers_orchestration(&task_keywords),
        prefer_dependency_graph_source_files: auto_seed_dependency_graph_task(&task_keywords),
        prefer_project_overview_source_files: auto_seed_project_overview_task(&task_keywords),
        prefer_symbol_search_source_files: auto_seed_symbol_search_task(&task_keywords),
        prefer_reference_search_source_files: auto_seed_reference_search_task(&task_keywords),
        prefer_call_graph_traversal_source_files: auto_seed_call_graph_traversal_task(
            &task_keywords,
        ),
        prefer_file_parsing_source_files: auto_seed_file_parsing_task(&task_keywords),
        prefer_binding_validation_source_files: auto_seed_binding_validation_task(&task_keywords),
        prefer_import_resolution_source_files: auto_seed_import_resolution_task(&task_keywords),
    };

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
        for range in seed_file_ranges(
            &root,
            file,
            &symbols,
            &task_keywords,
            &seed_symbols,
            task_path_locations
                .get(file)
                .map(Vec::as_slice)
                .unwrap_or_default(),
        ) {
            push_context_range(
                &mut ranges_by_file,
                file.clone(),
                range.start_line,
                range.end_line,
                range.reason,
                &range.source,
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
            "symbol_definition",
            context_score_for_file(
                &symbol.file,
                CONTEXT_SCORE_SYMBOL_DEFINITION + symbol_task_boost(symbol, &task_keywords),
                &scoring_policy,
            ),
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
            "reference",
            context_score_for_file(
                &reference.file,
                reference_score(reference) + reference_task_boost(reference, &task_keywords),
                &scoring_policy,
            ),
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

    for seed in &caller_graph_seeds {
        for call in store.callers(seed, 20)? {
            push_context_range(
                &mut ranges_by_file,
                call.file.clone(),
                call.line.saturating_sub(2).max(1),
                call.line + 2,
                format!("Call graph caller of {} via {}", call.callee, call.caller),
                "call_graph",
                context_score_for_file(
                    &call.file,
                    CONTEXT_SCORE_CALL_GRAPH + call_task_boost(&call, &task_keywords),
                    &scoring_policy,
                ),
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
                callee_file.clone(),
                1,
                40,
                format!("Call graph target of {} via {}", call.caller, call.callee),
                "call_graph",
                context_score_for_file(
                    &callee_file,
                    CONTEXT_SCORE_CALL_GRAPH + call_task_boost(&call, &task_keywords),
                    &scoring_policy,
                ),
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
                resolved_file.clone(),
                1,
                40,
                format!(
                    "Local dependency of {} via {}",
                    dependency.source_file, dependency.target
                ),
                "dependency",
                context_score_for_file(&resolved_file, score, &scoring_policy),
            );
        }
    }
    let selected_file_set = selected_files.iter().cloned().collect::<BTreeSet<_>>();
    let mut seen_type_relations = BTreeSet::new();
    for dependency in store
        .dependencies_touching_files(&selected_files, CONTEXT_TYPE_RELATION_DEPENDENCY_LIMIT)?
    {
        if !selected_file_set.contains(&dependency.source_file)
            || !is_type_relation_dependency(&dependency)
        {
            continue;
        }
        for symbol in context_type_relation_symbols(&store, &dependency)? {
            if symbol.file == dependency.source_file {
                continue;
            }
            let key = (
                dependency.source_file.clone(),
                dependency.target.clone(),
                symbol.file.clone(),
                symbol.qualified_name.clone(),
            );
            if !seen_type_relations.insert(key) {
                continue;
            }
            push_context_range(
                &mut ranges_by_file,
                symbol.file.clone(),
                symbol.start_line.saturating_sub(2).max(1),
                capped_symbol_end_line(&symbol) + 2,
                format!(
                    "Type relation target of {} via {} {}",
                    dependency
                        .local_alias
                        .as_deref()
                        .unwrap_or(&dependency.source_file),
                    dependency.kind.replace('_', " "),
                    dependency.target
                ),
                "type_relation",
                context_score_for_file(
                    &symbol.file,
                    CONTEXT_SCORE_TYPE_RELATION
                        + dependency_task_boost(&dependency, &task_keywords)
                        + symbol_task_boost(&symbol, &task_keywords),
                    &scoring_policy,
                ),
            );
        }
    }
    let type_relation_terms = context_type_relation_terms(&seed_symbols, &symbols);
    for dependency in store.type_relation_importers_for_symbols(
        &type_relation_terms,
        CONTEXT_TYPE_RELATION_DEPENDENCY_LIMIT,
    )? {
        if !is_type_relation_dependency(&dependency)
            || selected_file_set.contains(&dependency.source_file)
        {
            continue;
        }
        for symbol in context_type_relation_source_symbols(&store, &dependency)? {
            let key = (
                dependency.target.clone(),
                dependency.source_file.clone(),
                symbol.file.clone(),
                symbol.qualified_name.clone(),
            );
            if !seen_type_relations.insert(key) {
                continue;
            }
            push_context_range(
                &mut ranges_by_file,
                symbol.file.clone(),
                symbol.start_line.saturating_sub(2).max(1),
                capped_symbol_end_line(&symbol) + 2,
                format!(
                    "Type relation source of {} via {} {}",
                    dependency.target,
                    dependency
                        .imported_symbol
                        .as_deref()
                        .unwrap_or("implements"),
                    symbol.qualified_name
                ),
                "type_relation",
                context_score_for_file(
                    &symbol.file,
                    CONTEXT_SCORE_TYPE_RELATION
                        + dependency_task_boost(&dependency, &task_keywords)
                        + symbol_task_boost(&symbol, &task_keywords),
                    &scoring_policy,
                ),
            );
        }
    }
    let mut semantic_status = semantic_vector_context_matches(&store, &task, 20);
    let vector_matches = std::mem::take(&mut semantic_status.matches);
    for result in vector_matches {
        push_context_range(
            &mut ranges_by_file,
            result.file,
            result.start_line,
            result.end_line,
            format!(
                "Semantic vector match for task with score {:.3} near lines {}-{}",
                result.score, result.start_line, result.end_line
            ),
            "semantic",
            CONTEXT_SCORE_SEMANTIC_VECTOR + semantic_vector_score_boost(result.score),
        );
    }
    let fallback_chunks = store
        .semantic_chunks_matching(&semantic_ranking_terms(&task_keywords, &seed_symbols), 20)?;
    semantic_status.status.fallback_candidates = fallback_chunks.len();
    for chunk in fallback_chunks {
        push_context_range(
            &mut ranges_by_file,
            chunk.file.clone(),
            chunk.start_line,
            chunk.end_line,
            format!(
                "Semantic chunk match for task near lines {}-{}",
                chunk.start_line, chunk.end_line
            ),
            "semantic",
            CONTEXT_SCORE_SEMANTIC_CHUNK
                + semantic_chunk_task_boost(&chunk, &task_keywords)
                + semantic_chunk_density_boost(&chunk),
        );
    }
    semantic_status.status.recommendation =
        context_semantic_recommendation(&semantic_status.status);

    if scoring_policy.prefer_agent_first_read_source_files {
        ranges_by_file.retain(|file, _| {
            seed_file_set.contains(file) || !context_agent_first_read_support_file(file)
        });
    }

    let mut candidates = ranges_by_file
        .into_iter()
        .map(|(file, ranges)| {
            let total_score = ranges.iter().map(|range| range.score).sum();
            let mut ranges = merge_ranges(ranges);
            ranges.sort_by(compare_context_ranges_for_budget);
            let max_score = ranges.iter().map(|range| range.score).max().unwrap_or(0);
            let source_mix_score = context_range_source_mix_score(&ranges);
            let recent_edit_score = context_file_recent_edit_score(&root.join(&file));
            ContextFileCandidate {
                seed_order: seed_file_order.get(&file).copied(),
                file,
                ranges,
                max_score,
                source_mix_score,
                recent_edit_score,
                total_score,
            }
        })
        .collect::<Vec<_>>();
    candidates.sort_by(compare_context_file_candidates);
    let candidate_files = candidates.len();
    let candidate_ranges = candidates
        .iter()
        .map(|candidate| candidate.ranges.len())
        .sum::<usize>();

    let mut estimated_tokens = estimate_tokens(&task);
    let mut files = Vec::new();
    let mut truncated = false;

    for (candidate_index, candidate) in candidates.iter().enumerate() {
        let path = root.join(&candidate.file);
        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let lines = source.lines().collect::<Vec<_>>();
        let mut context_ranges = Vec::new();
        let mut selected_line_ranges = Vec::new();
        let mut selected_max_score = 0;
        let mut selected_source: Option<String> = None;
        let mut selected_reason: Option<String> = None;
        let mut selected_source_priority = 0;
        let mut selected_source_score = 0;

        for range in &candidate.ranges {
            let uncovered_segments = uncovered_segments(
                range.start_line,
                range.end_line.min(lines.len().max(1)),
                &selected_line_ranges,
            );

            for (start_line, mut end_line) in uncovered_segments {
                let mut excerpt = excerpt_lines(&lines, start_line, end_line);
                let mut range_tokens = estimate_tokens(&excerpt);
                if estimated_tokens + range_tokens > budget {
                    truncated = true;
                    if range.score >= CONTEXT_SCORE_SEED_FILE {
                        let remaining_budget = budget.saturating_sub(estimated_tokens);
                        if let Some((fitted_end_line, fitted_excerpt, fitted_tokens)) =
                            fit_context_range_to_budget(
                                &lines,
                                start_line,
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
                let source_priority = context_source_priority(range.source.as_str());
                if selected_source.is_none()
                    || source_priority > selected_source_priority
                    || (source_priority == selected_source_priority
                        && range.score > selected_source_score)
                {
                    selected_source = Some(range.source.clone());
                    selected_reason = Some(range.reason.clone());
                    selected_source_priority = source_priority;
                    selected_source_score = range.score;
                }
                selected_max_score = selected_max_score.max(range.score);
                selected_line_ranges.push((start_line, end_line));
                context_ranges.push(ContextRange {
                    start_line,
                    end_line,
                    source: range.source.clone(),
                    score: range.score,
                    importance: importance_for_score(range.score).to_string(),
                    reason: range.reason.clone(),
                    excerpt,
                });
            }
        }

        if !context_ranges.is_empty() {
            let source = selected_source.unwrap_or_else(|| "unknown".to_string());
            context_ranges.sort_by_key(|range| (range.start_line, range.end_line));
            files.push(ContextFile {
                file: candidate.file.clone(),
                source: source.clone(),
                score: selected_max_score,
                selection_rank: candidate_index + 1,
                reason: format!(
                    "Selected for {} relevance via {}: {}; {}",
                    importance_for_score(selected_max_score),
                    source,
                    selected_reason
                        .unwrap_or_else(|| "selected range matched the task".to_string()),
                    context_range_source_mix(&context_ranges)
                ),
                source_mix: context_range_source_counts(&context_ranges),
                ranges: context_ranges,
            });
        }
    }

    let summary = if auto_seeded {
        format!(
            "Context pack for task using auto-selected seed files: {}.",
            seed_files.join(", ")
        )
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

    semantic_status.status.selected_vector_ranges =
        count_selected_ranges_with_reason(&files, "Semantic vector match");
    semantic_status.status.selected_fallback_ranges =
        count_selected_ranges_with_reason(&files, "Semantic chunk match");
    semantic_status.status.recommendation =
        context_semantic_recommendation(&semantic_status.status);
    let reading_plan = context_reading_plan(&root, &task, &files, &selected_seeds);
    let selected_files = files.len();
    let selected_ranges = files.iter().map(|file| file.ranges.len()).sum::<usize>();
    let selected_source_lines = context_selected_source_lines(&files);
    let baseline_source_lines = store.overview(&root)?.total_lines;
    let read_less = context_read_less(baseline_source_lines, selected_source_lines);
    let omitted_candidates = context_omitted_candidates(
        &root,
        &task,
        &candidates,
        &files,
        truncated,
        CONTEXT_OMITTED_CANDIDATE_LIMIT,
    );
    let no_context_for_explicit_seed = !auto_seeded && files.is_empty();
    let mut budget_summary = ContextBudget {
        requested_token_budget: token_budget,
        applied_token_budget: budget,
        estimated_tokens,
        candidate_files,
        selected_files,
        omitted_files: candidate_files.saturating_sub(selected_files),
        candidate_ranges,
        selected_ranges,
        omitted_ranges: candidate_ranges.saturating_sub(selected_ranges),
        truncated,
        truncation_reason: context_budget_truncation_reason(
            token_budget,
            budget,
            truncated,
            candidate_files,
            selected_files,
            candidate_ranges,
            selected_ranges,
        ),
    };
    if no_context_for_explicit_seed {
        budget_summary.truncation_reason = "no_context_for_explicit_seed".to_string();
    }
    let continuation_summary = if no_context_for_explicit_seed {
        context_no_context_continuation_summary(&seed_symbols, &seed_files)
    } else {
        context_continuation_summary(&budget_summary, &omitted_candidates)
    };
    retain_selected_context_metadata(&files, &mut symbols, &mut references);

    Ok(ContextPack {
        task,
        summary,
        seed_strategy,
        selected_seeds,
        reading_plan,
        semantic_status: semantic_status.status,
        budget: budget_summary,
        read_less,
        continuation_summary,
        omitted_candidates,
        files,
        symbols,
        references,
        estimated_tokens,
        truncated,
    })
}

fn retain_selected_context_metadata(
    files: &[ContextFile],
    symbols: &mut Vec<Symbol>,
    references: &mut Vec<ReferenceMatch>,
) {
    let selected_ranges = files
        .iter()
        .map(|file| {
            (
                file.file.as_str(),
                file.ranges
                    .iter()
                    .map(|range| (range.start_line, range.end_line))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut seen_symbols = BTreeSet::new();
    symbols.retain(|symbol| {
        selected_ranges
            .get(symbol.file.as_str())
            .is_some_and(|ranges| {
                ranges.iter().any(|(start_line, end_line)| {
                    symbol.start_line <= *end_line && symbol.end_line >= *start_line
                })
            })
            && seen_symbols.insert((
                symbol.file.clone(),
                symbol.start_line,
                symbol.end_line,
                symbol.qualified_name.clone(),
            ))
    });

    let mut seen_references = BTreeSet::new();
    references.retain(|reference| {
        selected_ranges
            .get(reference.file.as_str())
            .is_some_and(|ranges| {
                ranges.iter().any(|(start_line, end_line)| {
                    reference.line >= *start_line && reference.line <= *end_line
                })
            })
            && seen_references.insert((
                reference.file.clone(),
                reference.line,
                reference.column,
                reference.reference_kind.clone(),
            ))
    });
}

fn context_selected_source_lines(files: &[ContextFile]) -> usize {
    files
        .iter()
        .flat_map(|file| &file.ranges)
        .map(|range| range.end_line.saturating_sub(range.start_line) + 1)
        .sum()
}

fn context_read_less(
    baseline_source_lines: usize,
    selected_source_lines: usize,
) -> ContextReadLess {
    let source_lines_avoided = baseline_source_lines.saturating_sub(selected_source_lines);
    let line_reduction = if baseline_source_lines == 0 {
        "n/a".to_string()
    } else {
        let reduction =
            (1.0 - (selected_source_lines as f64 / baseline_source_lines as f64)) * 100.0;
        format!("{:.1}%", reduction.max(0.0))
    };
    let read_less_ratio = if baseline_source_lines == 0 || selected_source_lines == 0 {
        "n/a".to_string()
    } else {
        format!(
            "{:.1}x",
            baseline_source_lines as f64 / selected_source_lines as f64
        )
    };

    ContextReadLess {
        baseline_source_lines,
        selected_source_lines,
        source_lines_avoided,
        line_reduction,
        read_less_ratio,
    }
}

fn context_budget_truncation_reason(
    requested_token_budget: usize,
    applied_token_budget: usize,
    truncated: bool,
    candidate_files: usize,
    selected_files: usize,
    candidate_ranges: usize,
    selected_ranges: usize,
) -> String {
    if requested_token_budget < applied_token_budget {
        return "minimum_budget_applied".to_string();
    }
    if truncated {
        return "token_budget_exhausted".to_string();
    }
    if selected_files < candidate_files || selected_ranges < candidate_ranges {
        return "candidate_selection_omitted_lower_ranked_context".to_string();
    }
    "none".to_string()
}

fn context_omitted_candidates(
    root: &Path,
    task: &str,
    candidates: &[ContextFileCandidate],
    files: &[ContextFile],
    budget_exhausted: bool,
    limit: usize,
) -> Vec<ContextOmittedCandidate> {
    let selected_files = files
        .iter()
        .map(|file| file.file.as_str())
        .collect::<BTreeSet<_>>();
    let omission_reason = context_omission_reason(budget_exhausted);
    candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| !selected_files.contains(candidate.file.as_str()))
        .take(limit)
        .filter_map(|(index, candidate)| {
            context_omitted_candidate(root, task, candidate, index + 1, omission_reason)
        })
        .collect()
}

fn context_omission_reason(budget_exhausted: bool) -> &'static str {
    if budget_exhausted {
        "token_budget_exhausted"
    } else {
        "lower_ranked_candidate_after_budget_selection"
    }
}

fn context_continuation_summary(
    budget: &ContextBudget,
    omitted_candidates: &[ContextOmittedCandidate],
) -> ContextContinuationSummary {
    let minimum_budget_note = context_minimum_budget_note(budget);
    if let Some(candidate) = omitted_candidates.first() {
        return ContextContinuationSummary {
            status: "omitted_candidates_available".to_string(),
            message: format!(
                "{}{} selected files fit the context budget; {} candidate files were omitted. Continue with {} if more context is needed.",
                minimum_budget_note, budget.selected_files, budget.omitted_files, candidate.file
            ),
            next_action: "run_omitted_candidate_context_pack".to_string(),
            omitted_candidate_count: omitted_candidates.len(),
            first_omitted_file: Some(candidate.file.clone()),
            suggested_tool: Some(candidate.suggested_tool.clone()),
        };
    }

    if budget.truncated {
        return ContextContinuationSummary {
            status: "token_budget_exhausted".to_string(),
            message: format!(
                "{}{} selected files fit the context budget, but some ranges were truncated. Increase token_budget or narrow the task for deeper context.",
                minimum_budget_note, budget.selected_files
            ),
            next_action: "increase_token_budget_or_narrow_task".to_string(),
            omitted_candidate_count: 0,
            first_omitted_file: None,
            suggested_tool: None,
        };
    }

    if budget.truncation_reason == "minimum_budget_applied" {
        return ContextContinuationSummary {
            status: "minimum_budget_applied".to_string(),
            message: format!(
                "The requested token budget was below the minimum, so {} tokens were applied.",
                budget.applied_token_budget
            ),
            next_action: "read_selected_context".to_string(),
            omitted_candidate_count: 0,
            first_omitted_file: None,
            suggested_tool: None,
        };
    }

    if budget.omitted_files > 0 || budget.omitted_ranges > 0 {
        return ContextContinuationSummary {
            status: "lower_ranked_context_omitted".to_string(),
            message: format!(
                "{}{} lower-ranked files and {} ranges were omitted; use a narrower seed if those signals are needed.",
                minimum_budget_note, budget.omitted_files, budget.omitted_ranges
            ),
            next_action: "narrow_task_or_seed".to_string(),
            omitted_candidate_count: 0,
            first_omitted_file: None,
            suggested_tool: None,
        };
    }

    ContextContinuationSummary {
        status: "complete".to_string(),
        message: "Selected context fits the applied token budget; follow the reading_plan first."
            .to_string(),
        next_action: "read_selected_context".to_string(),
        omitted_candidate_count: 0,
        first_omitted_file: None,
        suggested_tool: None,
    }
}

fn context_no_context_continuation_summary(
    seed_symbols: &[String],
    seed_files: &[String],
) -> ContextContinuationSummary {
    let seed_summary = if seed_symbols.is_empty() {
        format!("seed files: {}", seed_files.join(", "))
    } else if seed_files.is_empty() {
        format!("seed symbols: {}", seed_symbols.join(", "))
    } else {
        format!(
            "seed symbols: {}; seed files: {}",
            seed_symbols.join(", "),
            seed_files.join(", ")
        )
    };
    ContextContinuationSummary {
        status: "blocked_no_context".to_string(),
        message: format!(
            "Explicit {seed_summary} did not match any readable context; provide a matching --file or --symbol."
        ),
        next_action: "provide_matching_seed_file_or_symbol".to_string(),
        omitted_candidate_count: 0,
        first_omitted_file: None,
        suggested_tool: None,
    }
}

fn context_minimum_budget_note(budget: &ContextBudget) -> String {
    if budget.requested_token_budget < budget.applied_token_budget {
        format!(
            "Requested token budget {} was below the minimum, so {} tokens were applied. ",
            budget.requested_token_budget, budget.applied_token_budget
        )
    } else {
        String::new()
    }
}

fn context_omitted_candidate(
    root: &Path,
    task: &str,
    candidate: &ContextFileCandidate,
    selection_rank: usize,
    omission_reason: &str,
) -> Option<ContextOmittedCandidate> {
    let primary_range = candidate.ranges.first()?;
    let ranges = candidate
        .ranges
        .iter()
        .take(CONTEXT_OMITTED_CANDIDATE_RANGE_LIMIT)
        .map(|range| ContextReadingRange {
            start_line: range.start_line,
            end_line: range.end_line,
            source: range.source.clone(),
            importance: importance_for_score(range.score).to_string(),
        })
        .collect::<Vec<_>>();
    Some(ContextOmittedCandidate {
        file: candidate.file.clone(),
        source: primary_range.source.clone(),
        score: candidate.max_score,
        selection_rank,
        omission_reason: omission_reason.to_string(),
        next_action: "run_omitted_candidate_context_pack".to_string(),
        reason: format!(
            "Omitted from selected context because {}; candidate rank {} by score; top reason: {}",
            omission_reason, selection_rank, primary_range.reason
        ),
        ranges,
        suggested_tool: ContextSuggestedTool {
            tool: "context_pack".to_string(),
            priority: 60,
            reason: "Rebuild a focused context pack around this omitted candidate.".to_string(),
            suggested_arguments: json!({
                "root": root.display().to_string(),
                "task": task,
                "files": [candidate.file.clone()],
                "token_budget": 4000
            }),
        },
    })
}

fn context_reading_plan(
    root: &Path,
    task: &str,
    files: &[ContextFile],
    selected_seeds: &[ContextSeed],
) -> Vec<ContextReadingStep> {
    files
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, file)| {
            let next_action = context_reading_next_action(file).to_string();
            let question = context_reading_question(file, task);
            let requested_locations = context_requested_locations(selected_seeds, &file.file);
            let suggested_tool =
                context_reading_suggested_tool(root, task, file, &requested_locations);
            let ranges = file
                .ranges
                .iter()
                .take(4)
                .map(|range| ContextReadingRange {
                    start_line: range.start_line,
                    end_line: range.end_line,
                    source: range.source.clone(),
                    importance: range.importance.clone(),
                })
                .collect::<Vec<_>>();
            ContextReadingStep {
                order: index + 1,
                file: file.file.clone(),
                selection_rank: file.selection_rank,
                requested_locations,
                focus: context_reading_focus(file, task),
                next_action,
                question: question.clone(),
                reason: context_reading_reason(&question, &suggested_tool, file),
                suggested_tool,
                selection_reason: file.reason.clone(),
                source: file.source.clone(),
                score: file.score,
                source_mix: context_range_source_counts(&file.ranges),
                ranges,
            }
        })
        .collect()
}

fn context_requested_locations(
    selected_seeds: &[ContextSeed],
    file: &str,
) -> Vec<ContextSeedLocation> {
    let mut seen = BTreeSet::new();
    selected_seeds
        .iter()
        .filter(|seed| seed.kind == "file" && seed.value == file)
        .flat_map(|seed| seed.locations.iter())
        .filter(|location| seen.insert((location.start_line, location.end_line)))
        .cloned()
        .collect()
}

fn context_reading_reason(
    question: &str,
    suggested_tool: &ContextSuggestedTool,
    file: &ContextFile,
) -> String {
    let selection_reason = if file.reason.contains("evidence mix") {
        file.reason.clone()
    } else {
        format!(
            "{}; {}",
            file.reason,
            context_range_source_mix(&file.ranges)
        )
    };

    format!(
        "Read this step to answer: {question} If deeper evidence is needed, call {}. Selection reason: {}",
        suggested_tool.tool, selection_reason
    )
}

fn context_reading_suggested_tool(
    root: &Path,
    task: &str,
    file: &ContextFile,
    requested_locations: &[ContextSeedLocation],
) -> ContextSuggestedTool {
    let root_arg = root.display().to_string();
    if !requested_locations.is_empty() {
        return ContextSuggestedTool {
            tool: "file_outline".to_string(),
            priority: 5,
            reason: "Inspect symbols overlapping the explicitly requested line ranges before expanding to broader relationships."
                .to_string(),
            suggested_arguments: json!({
                "path": root.join(&file.file).display().to_string(),
                "locations": requested_locations
            }),
        };
    }

    match context_reading_next_action(file) {
        "inspect_seed_file" | "inspect_symbol_definition" => ContextSuggestedTool {
            tool: "file_outline".to_string(),
            priority: 10,
            reason: "Inspect the selected file's symbol outline before reading deeper ranges."
                .to_string(),
            suggested_arguments: json!({
                "path": root.join(&file.file).display().to_string()
            }),
        },
        "inspect_type_relation" => ContextSuggestedTool {
            tool: "dependency_graph".to_string(),
            priority: 35,
            reason: "Inspect inheritance or interface relationships around selected type context."
                .to_string(),
            suggested_arguments: json!({
                "root": root_arg,
                "files": [file.file.clone()],
                "kinds": ["base_type"],
                "limit": 100
            }),
        },
        "follow_call_graph" | "inspect_references" => ContextSuggestedTool {
            tool: "impact_analysis".to_string(),
            priority: 30,
            reason: "Expand from this file through references, calls, and dependency signals."
                .to_string(),
            suggested_arguments: json!({
                "root": root_arg,
                "files": [file.file.clone()],
                "limit": 20,
                "depth": 2,
                "format": "summary",
                "evidence_limit": 5
            }),
        },
        "review_semantic_matches" => ContextSuggestedTool {
            tool: "context_pack".to_string(),
            priority: 50,
            reason: "Rebuild a focused context pack around this semantically related file."
                .to_string(),
            suggested_arguments: json!({
                "root": root_arg,
                "task": task,
                "files": [file.file.clone()],
                "token_budget": 4000
            }),
        },
        "inspect_dependency" => ContextSuggestedTool {
            tool: "dependency_graph".to_string(),
            priority: 40,
            reason: "Inspect module and package relationships around selected dependency context."
                .to_string(),
            suggested_arguments: json!({
                "root": root_arg,
                "files": [file.file.clone()],
                "limit": 100
            }),
        },
        _ => ContextSuggestedTool {
            tool: "context_pack".to_string(),
            priority: 50,
            reason: "Rebuild context focused on this selected file.".to_string(),
            suggested_arguments: json!({
                "root": root_arg,
                "task": task,
                "files": [file.file.clone()],
                "token_budget": 4000
            }),
        },
    }
}

fn context_reading_focus(file: &ContextFile, task: &str) -> String {
    let sources = context_reading_sources(file);
    let signals = ContextTaskSignals::from_task(task);
    if sources.contains("seed_file") {
        context_seed_file_focus(signals)
    } else if sources.contains("symbol_definition") {
        context_symbol_definition_focus(signals)
    } else if sources.contains("type_relation") {
        context_type_relation_focus(signals)
    } else if sources.contains("call_graph") {
        context_call_graph_focus(signals)
    } else if sources.contains("reference") {
        context_reference_focus(signals)
    } else if sources.contains("semantic") {
        context_semantic_focus(signals)
    } else if sources.contains("dependency") {
        context_dependency_focus(signals)
    } else {
        "Review selected ranges for task-relevant context.".to_string()
    }
}

fn context_seed_file_focus(signals: ContextTaskSignals) -> String {
    if signals.current_reading_step_contract {
        "Start with seed file current_reading_step mirroring, reading-plan fields, and client handoff."
            .to_string()
    } else if signals.agent_first_read {
        "Start with seed file context routing, first-read handoff, and read-less evidence."
            .to_string()
    } else if signals.blocked_no_seed_route {
        "Start with seed file blocked no-seed routing, empty-state contract, and client handoff."
            .to_string()
    } else if signals.recommended_next_tools_contract {
        "Start with seed file recommended next tools ordering, arguments, and client contract."
            .to_string()
    } else if signals.project_entrypoint_ranking {
        "Start with seed file project overview entrypoint ranking, framework detection, and scoring."
            .to_string()
    } else if signals.budget_continuation {
        "Start with seed file token budget accounting, omitted candidates, and continuation contract."
            .to_string()
    } else if signals.impact_suggested_checks {
        "Start with seed file impact suggested checks, focused commands, and review gates."
            .to_string()
    } else if signals.mcp_tool_schema_validation {
        "Start with seed file MCP tool schema validation, argument binding, and error shaping."
            .to_string()
    } else if signals.config_status_reporting {
        "Start with seed file config status loading, parse diagnostics, and structured reporting."
            .to_string()
    } else if signals.semantic_index_explain {
        "Start with seed file semantic index explain output, chunk change reporting, and provider status."
            .to_string()
    } else if signals.semantic_provider_fallback {
        "Start with seed file semantic provider fallback, disabled-provider handling, and search readiness."
            .to_string()
    } else if signals.test_coverage {
        "Start with seed file test, spec, or regression coverage.".to_string()
    } else if signals.response_headers {
        "Start with seed file response headers, status metadata, or Content-Type boundaries."
            .to_string()
    } else if signals.response_cookies {
        "Start with seed file response cookies, Set-Cookie headers, or cookie option boundaries."
            .to_string()
    } else if signals.route_parameters {
        "Start with seed file route parameters, path variables, or wildcard extraction boundaries."
            .to_string()
    } else if signals.url_building {
        "Start with seed file URL building, reverse routing, or route path joining boundaries."
            .to_string()
    } else if signals.route_grouping {
        "Start with seed file mounted routers, blueprints, route groups, or nested route boundaries."
            .to_string()
    } else if signals.route_miss_handling {
        "Start with seed file route miss, 404/405, not-found, or method-not-allowed boundaries."
            .to_string()
    } else if signals.http_method_routing {
        "Start with seed file HTTP method routing, verb registration, or dispatch boundaries."
            .to_string()
    } else if signals.route_dispatch {
        "Start with seed file route registration, matching, or handler dispatch boundaries."
            .to_string()
    } else if signals.http_state_headers && !signals.auth_session && !signals.security_safety {
        "Start with seed file cookies, headers, or HTTP state boundaries.".to_string()
    } else if signals.request_body_parsing {
        "Start with seed file request body parsing, payload binding, or form-data boundaries."
            .to_string()
    } else if signals.request_query_params {
        "Start with seed file query string, request args, or URL parameter boundaries.".to_string()
    } else if signals.response_redirect {
        "Start with seed file response redirect, status code, or Location header boundaries."
            .to_string()
    } else if signals.static_file_serving {
        "Start with seed file static file, asset, or filesystem serving boundaries.".to_string()
    } else if signals.response_rendering {
        "Start with seed file response rendering, templates, or output format boundaries."
            .to_string()
    } else if signals.auth_session {
        "Start with seed file authentication and session boundaries.".to_string()
    } else if signals.network_http {
        "Start with seed file network client, proxy, redirect, or transport boundaries.".to_string()
    } else if signals.tls_certificate {
        "Start with seed file TLS, SSL, certificate, or verification boundaries.".to_string()
    } else if signals.reference_search {
        "Start with seed file reference search, usage classification, and definition filtering."
            .to_string()
    } else if signals.call_graph_traversal {
        "Start with seed file call graph extraction, caller/callee traversal, and path shaping."
            .to_string()
    } else if signals.symbol_search {
        "Start with seed file symbol lookup, matching, ranking, and result shaping.".to_string()
    } else if signals.import_resolution {
        "Start with seed file import parsing, alias resolution, package mapping, and local target resolution."
            .to_string()
    } else if signals.project_overview {
        "Start with seed file project summary, entrypoint detection, directory roles, and recommended next tools."
            .to_string()
    } else if signals.indexing_pipeline {
        "Start with seed file project indexing, source scanning, parsing, and graph extraction."
            .to_string()
    } else if signals.dependency_graph {
        "Start with seed file dependency graph extraction, local edge resolution, and graph output."
            .to_string()
    } else if signals.embedding_provider_status {
        "Start with seed file embedding provider status, diagnostics, and reporting boundaries."
            .to_string()
    } else if signals.semantic_context_orchestration {
        "Start with seed file semantic search orchestration, chunk fallback, and embedding context flow."
            .to_string()
    } else if signals.file_parsing_language {
        "Start with seed file parsing, AST extraction, or language support boundaries.".to_string()
    } else if signals.validation_binding {
        "Start with seed file validation, schema, binding, or serialization boundaries.".to_string()
    } else if signals.feature_flags {
        "Start with seed file feature flag, rollout, toggle, or experiment boundaries.".to_string()
    } else if signals.configuration {
        "Start with seed file configuration defaults and inputs.".to_string()
    } else if signals.startup {
        "Start with seed file startup and initialization flow.".to_string()
    } else if signals.error_recovery {
        "Start with seed file error handling, retry, and recovery boundaries.".to_string()
    } else if signals.middleware {
        "Start with seed file middleware and handler boundaries.".to_string()
    } else if signals.request_lifecycle {
        "Start with seed file request lifecycle, dispatch, and response finalization flow."
            .to_string()
    } else if signals.runtime_lifecycle {
        "Start with seed file runtime execution, script runner, or rerun lifecycle boundaries."
            .to_string()
    } else if signals.file_upload {
        "Start with seed file uploaded file storage, retrieval, and cleanup boundaries.".to_string()
    } else if signals.websocket_connection {
        "Start with seed file WebSocket connection, session, or message lifecycle boundaries."
            .to_string()
    } else if signals.performance_cache {
        "Start with seed file cache, performance, latency, or optimization boundaries.".to_string()
    } else if signals.observability_logging {
        "Start with seed file logging, telemetry, metrics, or tracing boundaries.".to_string()
    } else if signals.dependency_injection {
        "Start with seed file dependency injection, dependency resolution, or parameter injection boundaries."
            .to_string()
    } else if signals.security_safety {
        "Start with seed file security, secrets, sanitization, or vulnerability boundaries."
            .to_string()
    } else if signals.billing_payment {
        "Start with seed file billing, payment, checkout, or subscription boundaries.".to_string()
    } else if signals.frontend_ui {
        "Start with seed file frontend UI, component, or page boundaries.".to_string()
    } else if signals.background_jobs {
        "Start with seed file background jobs, queues, or worker boundaries.".to_string()
    } else if signals.api_handler {
        "Start with seed file API handler, controller, or endpoint boundaries.".to_string()
    } else if signals.documentation {
        "Start with seed file documentation, guide, or usage notes.".to_string()
    } else if signals.data_persistence {
        "Start with seed file data persistence and storage boundaries.".to_string()
    } else if signals.impact_flow {
        "Start with seed file calls, callees, and impact paths.".to_string()
    } else {
        "Start with seed file context and primary symbols.".to_string()
    }
}

fn context_symbol_definition_focus(signals: ContextTaskSignals) -> String {
    if signals.current_reading_step_contract {
        "Read symbol definitions that mirror current_reading_step from reading_plan and shape client handoff."
            .to_string()
    } else if signals.blocked_no_seed_route {
        "Read symbol definitions that implement blocked no-seed routing and empty-state handoff."
            .to_string()
    } else if signals.recommended_next_tools_contract {
        "Read symbol definitions that build recommended next tool entries, priorities, and arguments."
            .to_string()
    } else if signals.project_entrypoint_ranking {
        "Read symbol definitions that rank entrypoints, detect frameworks, or score project overview candidates."
            .to_string()
    } else if signals.budget_continuation {
        "Read symbol definitions that compute token budgets, omitted candidates, or continuation summaries."
            .to_string()
    } else if signals.impact_suggested_checks {
        "Read symbol definitions that choose impact suggested checks, focused commands, or review gates."
            .to_string()
    } else if signals.mcp_tool_schema_validation {
        "Read symbol definitions that validate MCP tool arguments, bind schemas, or shape protocol errors."
            .to_string()
    } else if signals.config_status_reporting {
        "Read symbol definitions that load config status, preserve parse diagnostics, or shape status reports."
            .to_string()
    } else if signals.semantic_index_explain {
        "Read symbol definitions that explain semantic index chunks, provider status, or embedding changes."
            .to_string()
    } else if signals.semantic_provider_fallback {
        "Read symbol definitions that choose semantic provider fallback, disabled-provider behavior, or readiness errors."
            .to_string()
    } else if signals.test_coverage {
        "Read symbol definitions that establish test coverage or regression behavior.".to_string()
    } else if signals.response_headers {
        "Read symbol definitions that establish response headers, status metadata, or Content-Type behavior."
            .to_string()
    } else if signals.response_cookies {
        "Read symbol definitions that establish response cookie, Set-Cookie, or cookie option behavior."
            .to_string()
    } else if signals.route_parameters {
        "Read symbol definitions that establish route parameter, path variable, or wildcard behavior."
            .to_string()
    } else if signals.url_building {
        "Read symbol definitions that establish URL building, reverse routing, or route path joining behavior."
            .to_string()
    } else if signals.route_grouping {
        "Read symbol definitions that establish mounted router, blueprint, route group, or nested route behavior."
            .to_string()
    } else if signals.route_miss_handling {
        "Read symbol definitions that establish route miss, 404/405, not-found, or method-not-allowed behavior."
            .to_string()
    } else if signals.http_method_routing {
        "Read symbol definitions that establish HTTP method routing, verb registration, or dispatch behavior."
            .to_string()
    } else if signals.request_body_parsing {
        "Read symbol definitions that establish request body parsing or payload binding behavior."
            .to_string()
    } else if signals.request_query_params {
        "Read symbol definitions that establish query string or request parameter behavior."
            .to_string()
    } else if signals.response_redirect {
        "Read symbol definitions that establish response redirect, status code, or Location header behavior."
            .to_string()
    } else if signals.static_file_serving {
        "Read symbol definitions that establish static file, asset, or filesystem serving behavior."
            .to_string()
    } else if signals.response_rendering {
        "Read symbol definitions that establish response rendering or output format behavior."
            .to_string()
    } else if signals.auth_session {
        "Read symbol definitions that establish authentication or session behavior.".to_string()
    } else if signals.network_http {
        "Read symbol definitions that establish network client, proxy, redirect, or transport behavior."
            .to_string()
    } else if signals.tls_certificate {
        "Read symbol definitions that establish TLS, SSL, certificate, or verification behavior."
            .to_string()
    } else if signals.reference_search {
        "Read symbol definitions that implement reference search, usage classification, or definition filtering."
            .to_string()
    } else if signals.call_graph_traversal {
        "Read symbol definitions that implement call extraction, caller/callee traversal, or call path shaping."
            .to_string()
    } else if signals.symbol_search {
        "Read symbol definitions that implement symbol lookup, matching, ranking, or result shaping."
            .to_string()
    } else if signals.import_resolution {
        "Read symbol definitions that implement import parsing, alias/package resolution, or local target mapping."
            .to_string()
    } else if signals.project_overview {
        "Read symbol definitions that build project summaries, entrypoint detection, directory roles, or next-tool recommendations."
            .to_string()
    } else if signals.indexing_pipeline {
        "Read symbol definitions that implement source scanning, parsing, symbol extraction, dependency extraction, or index writes."
            .to_string()
    } else if signals.dependency_graph {
        "Read symbol definitions that implement dependency graph extraction, local edge resolution, filtering, or graph output."
            .to_string()
    } else if signals.embedding_provider_status {
        "Read symbol definitions that implement embedding provider status, diagnostics, or reporting behavior."
            .to_string()
    } else if signals.semantic_context_orchestration {
        "Read symbol definitions that implement semantic search orchestration, chunk selection, fallback, or embedding context flow."
            .to_string()
    } else if signals.validation_binding {
        "Read symbol definitions that establish validation, schema, binding, or serialization behavior."
            .to_string()
    } else if signals.feature_flags {
        "Read symbol definitions that establish feature flag, rollout, toggle, or experiment behavior."
            .to_string()
    } else if signals.configuration {
        "Read symbol definitions that establish configuration behavior.".to_string()
    } else if signals.startup {
        "Read symbol definitions that establish startup behavior.".to_string()
    } else if signals.middleware {
        "Read symbol definitions that establish middleware boundaries.".to_string()
    } else if signals.request_lifecycle {
        "Read symbol definitions that establish request lifecycle or response finalization behavior."
            .to_string()
    } else if signals.runtime_lifecycle {
        "Read symbol definitions that establish runtime execution, script runner, or rerun lifecycle behavior."
            .to_string()
    } else if signals.file_upload {
        "Read symbol definitions that establish uploaded file storage, retrieval, or cleanup behavior."
            .to_string()
    } else if signals.websocket_connection {
        "Read symbol definitions that establish WebSocket connection, session, or message lifecycle behavior."
            .to_string()
    } else if signals.performance_cache {
        "Read symbol definitions that establish cache, performance, or optimization behavior."
            .to_string()
    } else if signals.observability_logging {
        "Read symbol definitions that establish logging, telemetry, metrics, or tracing behavior."
            .to_string()
    } else if signals.dependency_injection {
        "Read symbol definitions that establish dependency injection, dependency resolution, or parameter injection behavior."
            .to_string()
    } else if signals.security_safety {
        "Read symbol definitions that establish security, sanitization, or vulnerability behavior."
            .to_string()
    } else if signals.billing_payment {
        "Read symbol definitions that establish billing, payment, or subscription behavior."
            .to_string()
    } else if signals.frontend_ui {
        "Read symbol definitions that establish frontend component or page behavior.".to_string()
    } else if signals.background_jobs {
        "Read symbol definitions that establish background job or worker behavior.".to_string()
    } else if signals.api_handler {
        "Read symbol definitions that establish API handler or controller behavior.".to_string()
    } else if signals.documentation {
        "Read definitions or examples that documentation describes.".to_string()
    } else if signals.data_persistence {
        "Read symbol definitions that establish database or storage behavior.".to_string()
    } else if signals.error_recovery {
        "Read symbol definitions that establish error handling or recovery behavior.".to_string()
    } else if signals.impact_flow {
        "Read symbol definitions that anchor call and impact paths.".to_string()
    } else {
        "Read symbol definitions that anchor the requested task.".to_string()
    }
}

fn context_type_relation_focus(signals: ContextTaskSignals) -> String {
    if signals.auth_session {
        "Check inherited contracts or base behavior that can affect authentication and session boundaries."
            .to_string()
    } else if signals.api_handler {
        "Check inherited controller or interface contracts that shape API handler behavior."
            .to_string()
    } else if signals.security_safety {
        "Check inherited contracts or base behavior that can affect security boundaries."
            .to_string()
    } else if signals.runtime_lifecycle {
        "Check inherited contracts or base behavior that can affect runtime lifecycle boundaries."
            .to_string()
    } else if signals.file_upload {
        "Check inherited contracts or base behavior that can affect uploaded file boundaries."
            .to_string()
    } else if signals.websocket_connection {
        "Check inherited contracts or base behavior that can affect WebSocket session boundaries."
            .to_string()
    } else if signals.dependency_injection {
        "Check inherited contracts or base behavior that can affect dependency injection boundaries."
            .to_string()
    } else if signals.impact_flow {
        "Check base types or interfaces that can widen the impact path.".to_string()
    } else {
        "Check base types or interfaces that shape this selected type.".to_string()
    }
}

fn context_call_graph_focus(signals: ContextTaskSignals) -> String {
    if signals.test_coverage {
        "Follow call graph evidence from tests, specs, or regression coverage.".to_string()
    } else if signals.response_headers {
        "Follow call graph evidence for response headers, status metadata, or Content-Type flow."
            .to_string()
    } else if signals.response_cookies {
        "Follow call graph evidence for response cookies, Set-Cookie headers, or cookie options."
            .to_string()
    } else if signals.route_parameters {
        "Follow call graph evidence for route parameters, path variables, or wildcard extraction."
            .to_string()
    } else if signals.url_building {
        "Follow call graph evidence for URL building, reverse routing, or route path joining."
            .to_string()
    } else if signals.route_grouping {
        "Follow call graph evidence for mounted routers, blueprints, route groups, or nested route dispatch."
            .to_string()
    } else if signals.route_miss_handling {
        "Follow call graph evidence for route misses, 404/405 responses, or method-not-allowed fallbacks."
            .to_string()
    } else if signals.http_method_routing {
        "Follow call graph evidence for HTTP method routing, verb registration, or dispatch."
            .to_string()
    } else if signals.request_body_parsing {
        "Follow call graph evidence for request body parsing, payload binding, or form-data flow."
            .to_string()
    } else if signals.request_query_params {
        "Follow call graph evidence for query strings, request args, or URL parameter flow."
            .to_string()
    } else if signals.response_redirect {
        "Follow call graph evidence for redirect responses, status codes, or Location headers."
            .to_string()
    } else if signals.static_file_serving {
        "Follow call graph evidence for static file, asset, or filesystem serving behavior."
            .to_string()
    } else if signals.response_rendering {
        "Follow call graph evidence for response rendering, templates, or output formats."
            .to_string()
    } else if signals.auth_session {
        "Follow call graph evidence for authentication and session flow.".to_string()
    } else if signals.network_http {
        "Follow call graph evidence for network client requests, proxies, redirects, or transport flow."
            .to_string()
    } else if signals.tls_certificate {
        "Follow call graph evidence for TLS verification, certificates, CA bundles, or SSL context flow."
            .to_string()
    } else if signals.reference_search {
        "Follow call graph evidence for reference search dispatch, classification, and filtering."
            .to_string()
    } else if signals.call_graph_traversal {
        "Follow call graph evidence for call extraction, caller/callee traversal, and path shaping."
            .to_string()
    } else if signals.validation_binding {
        "Follow call graph evidence for validation, binding, parsing, or serialization flow."
            .to_string()
    } else if signals.feature_flags {
        "Follow call graph evidence for feature flag evaluation, rollout, toggle, or experiment flow."
            .to_string()
    } else if signals.configuration {
        "Follow call graph evidence for configuration propagation.".to_string()
    } else if signals.startup {
        "Follow call graph evidence for startup and initialization order.".to_string()
    } else if signals.middleware {
        "Follow call graph evidence for middleware and handler boundaries.".to_string()
    } else if signals.request_lifecycle {
        "Follow call graph evidence for request dispatch, hooks, and response finalization."
            .to_string()
    } else if signals.runtime_lifecycle {
        "Follow call graph evidence for runtime execution, script runner, rerun, or shutdown flow."
            .to_string()
    } else if signals.file_upload {
        "Follow call graph evidence for uploaded file storage, retrieval, cleanup, or exposure."
            .to_string()
    } else if signals.websocket_connection {
        "Follow call graph evidence for WebSocket opening, session handoff, message flow, or closure."
            .to_string()
    } else if signals.performance_cache {
        "Follow call graph evidence for cache lookups, latency, or optimization flow.".to_string()
    } else if signals.observability_logging {
        "Follow call graph evidence for logs, metrics, telemetry, or trace spans.".to_string()
    } else if signals.dependency_injection {
        "Follow call graph evidence for dependency injection, dependency resolution, or parameter injection flow."
            .to_string()
    } else if signals.security_safety {
        "Follow call graph evidence for security checks, sanitization, or vulnerability flow."
            .to_string()
    } else if signals.billing_payment {
        "Follow call graph evidence for checkout, billing, payment, or subscription flow."
            .to_string()
    } else if signals.frontend_ui {
        "Follow call graph evidence for frontend rendering or component flow.".to_string()
    } else if signals.background_jobs {
        "Follow call graph evidence for queued, scheduled, or background work.".to_string()
    } else if signals.api_handler {
        "Follow call graph evidence for API request, response, or controller flow.".to_string()
    } else if signals.documentation {
        "Follow call graph evidence that supports documented usage.".to_string()
    } else if signals.project_overview {
        "Follow call graph evidence for project summary assembly, entrypoint detection, and next-tool recommendations."
            .to_string()
    } else if signals.indexing_pipeline {
        "Follow call graph evidence for project indexing, source scanning, parsing, and index writes."
            .to_string()
    } else if signals.dependency_graph {
        "Follow call graph evidence for dependency graph extraction, local edge resolution, and graph output."
            .to_string()
    } else if signals.embedding_provider_status {
        "Follow call graph evidence for embedding provider status, diagnostics, and reporting flow."
            .to_string()
    } else if signals.semantic_context_orchestration {
        "Follow call graph evidence for semantic search orchestration, chunk fallback, and embedding context flow."
            .to_string()
    } else if signals.data_persistence {
        "Follow call graph evidence for database, repository, or storage flow.".to_string()
    } else if signals.error_recovery {
        "Follow call graph evidence for error propagation, retries, and recovery.".to_string()
    } else if signals.impact_flow {
        "Follow call graph evidence for callers, callees, and impact paths.".to_string()
    } else {
        "Follow static call graph evidence around the seed flow.".to_string()
    }
}

fn context_reference_focus(signals: ContextTaskSignals) -> String {
    if signals.test_coverage {
        "Inspect references that exercise behavior in tests, specs, or regression cases."
            .to_string()
    } else if signals.response_headers {
        "Inspect references that set response headers, status metadata, or Content-Type values."
            .to_string()
    } else if signals.response_cookies {
        "Inspect references that set response cookies, Set-Cookie headers, or cookie options."
            .to_string()
    } else if signals.route_parameters {
        "Inspect references that capture, attach, or read route parameters and path variables."
            .to_string()
    } else if signals.url_building {
        "Inspect references that build URLs, reverse routes, or join route paths.".to_string()
    } else if signals.route_grouping {
        "Inspect references that mount routers, register blueprints, create route groups, or attach nested routes."
            .to_string()
    } else if signals.route_miss_handling {
        "Inspect references that configure not-found handlers, method-not-allowed handlers, or route miss fallbacks."
            .to_string()
    } else if signals.http_method_routing {
        "Inspect references that register HTTP verbs, match methods, or dispatch handlers."
            .to_string()
    } else if signals.request_body_parsing {
        "Inspect references that parse request bodies, bind payloads, or read form data."
            .to_string()
    } else if signals.request_query_params {
        "Inspect references that read query strings, request args, or URL parameters.".to_string()
    } else if signals.response_redirect {
        "Inspect references that issue redirect responses or set redirect locations.".to_string()
    } else if signals.static_file_serving {
        "Inspect references that register, serve, or configure static files and assets.".to_string()
    } else if signals.response_rendering {
        "Inspect references that render responses, templates, or output formats.".to_string()
    } else if signals.auth_session {
        "Inspect references that consume authentication or session state.".to_string()
    } else if signals.network_http {
        "Inspect references that send requests, select proxies, follow redirects, or configure transports."
            .to_string()
    } else if signals.tls_certificate {
        "Inspect references that verify TLS certificates, configure SSL, or pass CA/cert inputs."
            .to_string()
    } else if signals.reference_search {
        "Inspect references that exercise reference search, classification, or definition filtering."
            .to_string()
    } else if signals.call_graph_traversal {
        "Inspect references that exercise caller/callee lookup, call extraction, or path shaping."
            .to_string()
    } else if signals.validation_binding {
        "Inspect references that validate inputs, bind payloads, parse schemas, or serialize data."
            .to_string()
    } else if signals.feature_flags {
        "Inspect references that evaluate, override, or consume feature flags and rollout state."
            .to_string()
    } else if signals.configuration {
        "Inspect references that read or pass configuration values.".to_string()
    } else if signals.startup {
        "Inspect references that register or trigger startup behavior.".to_string()
    } else if signals.middleware {
        "Inspect references that attach or call middleware boundaries.".to_string()
    } else if signals.request_lifecycle {
        "Inspect references that enter, hook into, or finalize request lifecycle flow.".to_string()
    } else if signals.runtime_lifecycle {
        "Inspect references that execute scripts, coordinate reruns, or transition runtime lifecycle state."
            .to_string()
    } else if signals.file_upload {
        "Inspect references that store, retrieve, clean up, or expose uploaded files.".to_string()
    } else if signals.websocket_connection {
        "Inspect references that open, track, hand off, or close WebSocket connections.".to_string()
    } else if signals.performance_cache {
        "Inspect references that read, write, invalidate, or optimize cached work.".to_string()
    } else if signals.observability_logging {
        "Inspect references that emit, record, or propagate logs, metrics, telemetry, or traces."
            .to_string()
    } else if signals.dependency_injection {
        "Inspect references that declare, solve, inject, or consume dependencies.".to_string()
    } else if signals.security_safety {
        "Inspect references that validate security, sanitize input, handle secrets, or guard vulnerabilities.".to_string()
    } else if signals.billing_payment {
        "Inspect references that create checkout, invoice, payment, or subscription flow."
            .to_string()
    } else if signals.frontend_ui {
        "Inspect references that render, mount, or compose frontend UI.".to_string()
    } else if signals.background_jobs {
        "Inspect references that enqueue, schedule, or run background work.".to_string()
    } else if signals.api_handler {
        "Inspect references that register, route, or invoke API handlers.".to_string()
    } else if signals.documentation {
        "Inspect references that connect documented usage to implementation.".to_string()
    } else if signals.data_persistence {
        "Inspect references that read, write, or persist data.".to_string()
    } else if signals.error_recovery {
        "Inspect references that catch, wrap, retry, or recover from failures.".to_string()
    } else if signals.impact_flow {
        "Inspect references that show production usage and impact paths.".to_string()
    } else {
        "Inspect references that show how the seed is used.".to_string()
    }
}

fn context_semantic_focus(signals: ContextTaskSignals) -> String {
    if signals.test_coverage {
        "Review semantic matches for test, spec, or regression coverage.".to_string()
    } else if signals.response_headers {
        "Review semantic matches for response headers, status metadata, or Content-Type behavior."
            .to_string()
    } else if signals.response_cookies {
        "Review semantic matches for response cookies, Set-Cookie headers, or cookie options."
            .to_string()
    } else if signals.route_parameters {
        "Review semantic matches for route parameters, path variables, or wildcard extraction."
            .to_string()
    } else if signals.url_building {
        "Review semantic matches for URL building, reverse routing, or route path joining."
            .to_string()
    } else if signals.route_grouping {
        "Review semantic matches for mounted routers, blueprints, route groups, or nested route structure."
            .to_string()
    } else if signals.route_miss_handling {
        "Review semantic matches for route misses, not-found handlers, method-not-allowed handlers, or 404/405 responses."
            .to_string()
    } else if signals.http_method_routing {
        "Review semantic matches for HTTP method routing, verb registration, or dispatch."
            .to_string()
    } else if signals.request_body_parsing {
        "Review semantic matches for request body parsing, payload binding, or form-data behavior."
            .to_string()
    } else if signals.request_query_params {
        "Review semantic matches for query strings, request args, or URL parameters.".to_string()
    } else if signals.response_redirect {
        "Review semantic matches for redirect responses, status codes, or Location headers."
            .to_string()
    } else if signals.static_file_serving {
        "Review semantic matches for static file, asset, or filesystem serving behavior."
            .to_string()
    } else if signals.response_rendering {
        "Review semantic matches for response rendering, templates, or output formats.".to_string()
    } else if signals.auth_session {
        "Review semantic matches for authentication, cookie, or session behavior.".to_string()
    } else if signals.network_http {
        "Review semantic matches for network clients, proxies, redirects, adapters, or transports."
            .to_string()
    } else if signals.tls_certificate {
        "Review semantic matches for TLS, SSL, certificates, CA bundles, or verification."
            .to_string()
    } else if signals.embedding_provider_status {
        "Review semantic matches for embedding provider status, diagnostics, or reporting."
            .to_string()
    } else if signals.reference_search {
        "Review semantic matches for reference search, usage classification, or definition filtering."
            .to_string()
    } else if signals.call_graph_traversal {
        "Review semantic matches for call graph extraction, caller/callee traversal, or path shaping."
            .to_string()
    } else if signals.validation_binding {
        "Review semantic matches for validation, schemas, bindings, parsers, or serializers."
            .to_string()
    } else if signals.feature_flags {
        "Review semantic matches for feature flags, rollouts, toggles, variants, or experiments."
            .to_string()
    } else if signals.configuration {
        "Review semantic matches for configuration and environment behavior.".to_string()
    } else if signals.startup {
        "Review semantic matches for startup and initialization behavior.".to_string()
    } else if signals.middleware {
        "Review semantic matches for middleware or handler behavior.".to_string()
    } else if signals.request_lifecycle {
        "Review semantic matches for request lifecycle, dispatch, hooks, or response finalization."
            .to_string()
    } else if signals.runtime_lifecycle {
        "Review semantic matches for runtime execution, script runner lifecycle, reruns, or shutdown."
            .to_string()
    } else if signals.file_upload {
        "Review semantic matches for uploaded file storage, retrieval, cleanup, or exposure."
            .to_string()
    } else if signals.websocket_connection {
        "Review semantic matches for WebSocket connections, sessions, messages, or close handling."
            .to_string()
    } else if signals.performance_cache {
        "Review semantic matches for cache behavior, performance, latency, or optimization."
            .to_string()
    } else if signals.observability_logging {
        "Review semantic matches for logging, telemetry, metrics, tracing, or monitoring."
            .to_string()
    } else if signals.dependency_injection {
        "Review semantic matches for dependency injection, dependency resolution, or parameter injection."
            .to_string()
    } else if signals.security_safety {
        "Review semantic matches for security, sanitization, secrets, or vulnerabilities."
            .to_string()
    } else if signals.billing_payment {
        "Review semantic matches for billing, payment, checkout, invoices, or subscriptions."
            .to_string()
    } else if signals.frontend_ui {
        "Review semantic matches for frontend UI, pages, forms, or components.".to_string()
    } else if signals.background_jobs {
        "Review semantic matches for background jobs, queues, workers, or schedulers.".to_string()
    } else if signals.api_handler {
        "Review semantic matches for API handlers, controllers, or endpoints.".to_string()
    } else if signals.documentation {
        "Review semantic matches for documentation, guides, or usage examples.".to_string()
    } else if signals.data_persistence {
        "Review semantic matches for database, repository, or storage behavior.".to_string()
    } else if signals.error_recovery {
        "Review semantic matches for error handling, retries, or recovery behavior.".to_string()
    } else {
        "Review semantic matches related to the task wording.".to_string()
    }
}

fn context_dependency_focus(signals: ContextTaskSignals) -> String {
    if signals.test_coverage {
        "Check local dependencies that support test setup, fixtures, or assertions.".to_string()
    } else if signals.response_headers {
        "Check local dependencies that supply response header, status metadata, or Content-Type behavior."
            .to_string()
    } else if signals.response_cookies {
        "Check local dependencies that supply response cookie, Set-Cookie, or cookie option behavior."
            .to_string()
    } else if signals.route_parameters {
        "Check local dependencies that supply route parameter, path variable, or wildcard behavior."
            .to_string()
    } else if signals.url_building {
        "Check local dependencies that supply URL building, reverse routing, or route path joining behavior."
            .to_string()
    } else if signals.route_grouping {
        "Check local dependencies that supply mounted router, blueprint, route group, or nested route behavior."
            .to_string()
    } else if signals.route_miss_handling {
        "Check local dependencies that supply route miss, not-found, method-not-allowed, or final handler behavior."
            .to_string()
    } else if signals.http_method_routing {
        "Check local dependencies that supply HTTP method routing, verb registration, or dispatch behavior."
            .to_string()
    } else if signals.request_body_parsing {
        "Check local dependencies that supply request body parsing, payload binding, or form-data behavior.".to_string()
    } else if signals.request_query_params {
        "Check local dependencies that supply query string parsing or request parameter behavior."
            .to_string()
    } else if signals.response_redirect {
        "Check local dependencies that supply redirect response or Location header behavior."
            .to_string()
    } else if signals.static_file_serving {
        "Check local dependencies that supply static file, asset, or filesystem serving."
            .to_string()
    } else if signals.response_rendering {
        "Check local dependencies that shape response rendering or output formats.".to_string()
    } else if signals.auth_session {
        "Check local dependencies that affect authentication or session boundaries.".to_string()
    } else if signals.network_http {
        "Check local dependencies that shape network client, proxy, redirect, or transport behavior."
            .to_string()
    } else if signals.tls_certificate {
        "Check local dependencies that shape TLS verification, SSL context, certificates, or CA bundles."
            .to_string()
    } else if signals.reference_search {
        "Check local dependencies that supply reference search, usage classification, or definition filtering."
            .to_string()
    } else if signals.call_graph_traversal {
        "Check local dependencies that supply call extraction, caller/callee traversal, or path shaping."
            .to_string()
    } else if signals.validation_binding {
        "Check local dependencies that shape validation, binding, parsing, or serialization behavior."
            .to_string()
    } else if signals.feature_flags {
        "Check local dependencies that shape feature flag, rollout, toggle, or experiment behavior."
            .to_string()
    } else if signals.configuration {
        "Check local dependencies that supply configuration behavior.".to_string()
    } else if signals.startup {
        "Check local dependencies that participate in startup behavior.".to_string()
    } else if signals.middleware {
        "Check local dependencies that shape middleware or handler dispatch.".to_string()
    } else if signals.request_lifecycle {
        "Check local dependencies that shape request dispatch, hooks, or response finalization."
            .to_string()
    } else if signals.runtime_lifecycle {
        "Check local dependencies that shape runtime execution, script runner, or rerun lifecycle behavior."
            .to_string()
    } else if signals.file_upload {
        "Check local dependencies that shape uploaded file storage, retrieval, or cleanup behavior."
            .to_string()
    } else if signals.websocket_connection {
        "Check local dependencies that shape WebSocket connection, session, or message lifecycle behavior."
            .to_string()
    } else if signals.performance_cache {
        "Check local dependencies that shape cache, performance, or optimization behavior."
            .to_string()
    } else if signals.observability_logging {
        "Check local dependencies that shape logging, metrics, telemetry, or tracing behavior."
            .to_string()
    } else if signals.dependency_injection {
        "Check local dependencies that shape dependency injection, dependency resolution, or parameter injection behavior."
            .to_string()
    } else if signals.security_safety {
        "Check local dependencies that shape security, sanitization, or vulnerability handling."
            .to_string()
    } else if signals.billing_payment {
        "Check local dependencies that shape billing, payment, or subscription dispatch."
            .to_string()
    } else if signals.frontend_ui {
        "Check local dependencies that shape frontend rendering or component composition."
            .to_string()
    } else if signals.background_jobs {
        "Check local dependencies that shape queue, worker, or scheduler dispatch.".to_string()
    } else if signals.api_handler {
        "Check local dependencies that shape API handler or controller dispatch.".to_string()
    } else if signals.documentation {
        "Check local dependencies referenced by documentation or examples.".to_string()
    } else if signals.project_overview {
        "Check local dependencies that supply project summary, entrypoint detection, directory role, or next-tool data."
            .to_string()
    } else if signals.indexing_pipeline {
        "Check local dependencies that supply source scanning, parsing, symbol extraction, dependency extraction, or index storage."
            .to_string()
    } else if signals.dependency_graph {
        "Check local dependencies that supply dependency edges, local resolution, filtering, or graph output."
            .to_string()
    } else if signals.embedding_provider_status {
        "Check local dependencies that supply embedding provider status, diagnostics, or reporting data."
            .to_string()
    } else if signals.semantic_context_orchestration {
        "Check local dependencies that supply semantic chunks, embeddings, fallback ranges, or context assembly."
            .to_string()
    } else if signals.data_persistence {
        "Check local dependencies that supply database or storage behavior.".to_string()
    } else if signals.error_recovery {
        "Check local dependencies that shape failure handling or recovery behavior.".to_string()
    } else {
        "Check local dependency context that supports selected files.".to_string()
    }
}

fn context_reading_next_action(file: &ContextFile) -> &'static str {
    let sources = context_reading_sources(file);
    if sources.contains("seed_file") {
        "inspect_seed_file"
    } else if sources.contains("symbol_definition") {
        "inspect_symbol_definition"
    } else if sources.contains("type_relation") {
        "inspect_type_relation"
    } else if sources.contains("call_graph") {
        "follow_call_graph"
    } else if sources.contains("reference") {
        "inspect_references"
    } else if sources.contains("semantic") {
        "review_semantic_matches"
    } else if sources.contains("dependency") {
        "inspect_dependency"
    } else {
        "review_selected_ranges"
    }
}

fn context_reading_question(file: &ContextFile, task: &str) -> String {
    match context_reading_next_action(file) {
        "inspect_seed_file" => context_seed_file_question(task),
        "inspect_symbol_definition" => context_symbol_definition_question(task),
        "inspect_type_relation" => context_type_relation_question(task),
        "follow_call_graph" => context_call_graph_question(task),
        "inspect_references" => context_reference_question(task),
        "review_semantic_matches" => context_semantic_question(task),
        "inspect_dependency" => context_dependency_question(task),
        _ => "What task-relevant context is present in these selected ranges?".to_string(),
    }
}

fn context_seed_file_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.current_reading_step_contract {
        "Where is current_reading_step mirrored from reading_plan[0], and how is that handoff exposed to clients?"
            .to_string()
    } else if signals.agent_first_read {
        "Which seed selection, reading-plan handoff, or read-less evidence controls the agent first-read workflow here?"
            .to_string()
    } else if signals.blocked_no_seed_route {
        "Where does the no-seed path become a blocked route, continuation status, or client-facing next action here?"
            .to_string()
    } else if signals.recommended_next_tools_contract {
        "Where are recommended next tools selected, ordered, justified, or shaped into client-ready arguments here?"
            .to_string()
    } else if signals.project_entrypoint_ranking {
        "Where are project overview entrypoints detected, ranked, scored, or filtered here?"
            .to_string()
    } else if signals.budget_continuation {
        "Where are token budgets applied, omitted candidates recorded, and continuation next actions decided here?"
            .to_string()
    } else if signals.impact_suggested_checks {
        "Where are impact suggested checks selected, focused commands built, or review gates added here?"
            .to_string()
    } else if signals.mcp_tool_schema_validation {
        "Where are MCP tool arguments validated, schemas bound, invalid shapes rejected, or protocol errors shaped here?"
            .to_string()
    } else if signals.config_status_reporting {
        "Where is config status loaded, parse errors preserved, defaults detected, or status output shaped here?"
            .to_string()
    } else if signals.semantic_index_explain {
        "Where does semantic index explain report chunk changes, provider readiness, or embedding status here?"
            .to_string()
    } else if signals.semantic_provider_fallback {
        "Where is a disabled semantic provider detected, reported, or turned into fallback/readiness behavior here?"
            .to_string()
    } else if signals.test_coverage {
        "Which behavior, assertions, fixtures, or regression cases are covered here?".to_string()
    } else if signals.response_headers {
        "Where are response headers set, status metadata written, or Content-Type values selected here?"
            .to_string()
    } else if signals.response_cookies {
        "Where are response cookies created, Set-Cookie headers appended, or cookie options applied here?"
            .to_string()
    } else if signals.route_parameters {
        "Where are route parameters captured, attached to requests, or passed into handlers here?"
            .to_string()
    } else if signals.url_building {
        "Where are URLs built, routes reversed, or route paths joined here?".to_string()
    } else if signals.route_grouping {
        "Where are routers mounted, blueprints registered, route groups created, or nested routes attached here?"
            .to_string()
    } else if signals.route_miss_handling {
        "Where are route misses, 404/405 responses, not-found handlers, or method-not-allowed fallbacks decided here?"
            .to_string()
    } else if signals.http_method_routing {
        "Where are HTTP methods registered, verbs matched, or handlers dispatched here?".to_string()
    } else if signals.route_dispatch {
        "Where are routes registered, matched, and dispatched to handlers here?".to_string()
    } else if signals.http_state_headers && !signals.auth_session && !signals.security_safety {
        "Where are cookies, headers, or HTTP state containers handled here?".to_string()
    } else if signals.request_body_parsing {
        "Where are request bodies parsed, payloads bound, content types selected, or form data read here?".to_string()
    } else if signals.request_query_params {
        "Where are query strings parsed, request args read, or URL parameters exposed here?"
            .to_string()
    } else if signals.response_redirect {
        "Where are redirect responses built, status codes selected, or Location headers set here?"
            .to_string()
    } else if signals.static_file_serving {
        "Where are static files, assets, filesystem roots, or file responses served here?"
            .to_string()
    } else if signals.response_rendering {
        "Where are responses rendered, templates selected, or output formats produced here?"
            .to_string()
    } else if signals.auth_session {
        "Where are authentication decisions, credentials, or session boundaries handled here?"
            .to_string()
    } else if signals.network_http {
        "Where are network requests, proxies, redirects, adapters, or transports handled here?"
            .to_string()
    } else if signals.tls_certificate {
        "Where are TLS certificates, SSL settings, CA bundles, or verification decisions handled here?"
            .to_string()
    } else if signals.reference_search {
        "Where are references found, usage kinds classified, definitions filtered, or results shaped here?"
            .to_string()
    } else if signals.call_graph_traversal {
        "Where are calls extracted, callers or callees traversed, paths bounded, or results shaped here?"
            .to_string()
    } else if signals.symbol_search {
        "Where are symbol queries matched, ranked, limited, or formatted into search results here?"
            .to_string()
    } else if signals.import_resolution {
        "Where are imports parsed, aliases or package metadata applied, and local targets resolved here?"
            .to_string()
    } else if signals.project_overview {
        "Where are project summaries, entrypoint candidates, directory roles, or recommended next tools assembled here?"
            .to_string()
    } else if signals.indexing_pipeline {
        "Where are files scanned, languages parsed, symbols extracted, dependencies captured, or index records written here?"
            .to_string()
    } else if signals.dependency_graph {
        "Where are dependency edges extracted, resolved, filtered, or formatted into graph output here?"
            .to_string()
    } else if signals.embedding_provider_status {
        "Where is embedding provider status detected, diagnosed, normalized, or reported here?"
            .to_string()
    } else if signals.semantic_context_orchestration {
        "Where are semantic searches routed, chunks selected, embedding fallback applied, or context results assembled here?"
            .to_string()
    } else if signals.file_parsing_language {
        "Where are source files parsed, languages detected, ASTs built, or symbols extracted here?"
            .to_string()
    } else if signals.validation_binding {
        "Where are inputs validated, payloads bound, schemas applied, or data serialized here?"
            .to_string()
    } else if signals.feature_flags {
        "Where are feature flags, rollouts, toggles, variants, or experiments evaluated here?"
            .to_string()
    } else if signals.configuration {
        "Which configuration options, defaults, or environment inputs control the requested behavior?".to_string()
    } else if signals.startup {
        "What startup entrypoint or initialization sequence creates the requested flow?".to_string()
    } else if signals.error_recovery {
        "Where are errors, retries, timeouts, or recovery decisions handled here?".to_string()
    } else if signals.middleware {
        "Which middleware or handler boundaries shape the requested flow here?".to_string()
    } else if signals.request_lifecycle {
        "Where do request lifecycle hooks, dispatch, and response finalization happen here?"
            .to_string()
    } else if signals.runtime_lifecycle {
        "Where does the runtime execute scripts, coordinate reruns, or transition lifecycle state here?"
            .to_string()
    } else if signals.file_upload {
        "Where are uploaded files stored, retrieved, cleaned up, or exposed to callers here?"
            .to_string()
    } else if signals.websocket_connection {
        "Where are WebSocket connections opened, tracked, handed to sessions, or closed here?"
            .to_string()
    } else if signals.performance_cache {
        "Where are cache reads, invalidation, latency, or optimization decisions handled here?"
            .to_string()
    } else if signals.observability_logging {
        "Where are logs, metrics, telemetry, or trace spans emitted here?".to_string()
    } else if signals.dependency_injection {
        "Where are dependencies declared, resolved, injected, or passed into callables here?"
            .to_string()
    } else if signals.security_safety {
        "Where are security checks, secrets, sanitization, or vulnerability boundaries handled here?"
            .to_string()
    } else if signals.billing_payment {
        "Where are billing, payment, checkout, invoice, or subscription decisions handled here?"
            .to_string()
    } else if signals.frontend_ui {
        "Which frontend component, page, screen, form, or layout behavior is handled here?"
            .to_string()
    } else if signals.background_jobs {
        "Where are background jobs, queues, workers, or scheduled runs handled here?".to_string()
    } else if signals.api_handler {
        "Where are API requests, responses, handlers, or controller boundaries handled here?"
            .to_string()
    } else if signals.documentation {
        "What setup, usage, examples, or documented workflow should the agent follow here?"
            .to_string()
    } else if signals.data_persistence {
        "Where are database access, persistence decisions, or storage boundaries handled here?"
            .to_string()
    } else if signals.impact_flow {
        "Which local callers, callees, or impact paths in this seed file explain the requested flow?".to_string()
    } else {
        "What entrypoints, exported symbols, or setup code define the main flow here?".to_string()
    }
}

fn context_symbol_definition_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.current_reading_step_contract {
        "What current_reading_step mirror, reading_plan field, or client handoff behavior does this definition establish?"
            .to_string()
    } else if signals.blocked_no_seed_route {
        "What blocked no-seed route status, continuation behavior, or client handoff does this definition establish?"
            .to_string()
    } else if signals.recommended_next_tools_contract {
        "What recommended next tool ordering, priority, reason, or argument contract does this definition establish?"
            .to_string()
    } else if signals.project_entrypoint_ranking {
        "What entrypoint detection, ranking, framework scoring, or overview filtering behavior does this definition establish?"
            .to_string()
    } else if signals.budget_continuation {
        "What token budget, omitted-candidate, truncation, or continuation behavior does this definition establish?"
            .to_string()
    } else if signals.impact_suggested_checks {
        "What impact suggested check, focused command, or review gate behavior does this definition establish?"
            .to_string()
    } else if signals.mcp_tool_schema_validation {
        "What MCP argument validation, schema binding, invalid-shape rejection, or protocol error behavior does this definition establish?"
            .to_string()
    } else if signals.config_status_reporting {
        "What config status loading, parse diagnostic, default detection, or report-shaping behavior does this definition establish?"
            .to_string()
    } else if signals.semantic_index_explain {
        "What semantic index explain, chunk-change, provider-readiness, or embedding-status behavior does this definition establish?"
            .to_string()
    } else if signals.semantic_provider_fallback {
        "What disabled-provider detection, semantic fallback, or readiness-error behavior does this definition establish?"
            .to_string()
    } else if signals.test_coverage {
        "What test behavior, assertion, fixture, or regression case does this definition establish?"
            .to_string()
    } else if signals.response_headers {
        "What response header, status metadata, or Content-Type behavior does this definition establish?".to_string()
    } else if signals.response_cookies {
        "What response cookie, Set-Cookie, or cookie option behavior does this definition establish?"
            .to_string()
    } else if signals.route_parameters {
        "What route parameter, path variable, or wildcard behavior does this definition establish?"
            .to_string()
    } else if signals.url_building {
        "What URL building, reverse routing, or route path joining behavior does this definition establish?"
            .to_string()
    } else if signals.route_grouping {
        "What mounted router, blueprint, route group, prefix, or nested route behavior does this definition establish?"
            .to_string()
    } else if signals.route_miss_handling {
        "What route miss, 404/405, not-found, or method-not-allowed behavior does this definition establish?"
            .to_string()
    } else if signals.http_method_routing {
        "What HTTP method routing, verb registration, or dispatch behavior does this definition establish?"
            .to_string()
    } else if signals.request_body_parsing {
        "What request body parsing, payload binding, content-type, or form-data behavior does this definition establish?".to_string()
    } else if signals.request_query_params {
        "What query string, request arg, or URL parameter behavior does this definition establish?"
            .to_string()
    } else if signals.response_redirect {
        "What redirect response, status code, or Location header behavior does this definition establish?".to_string()
    } else if signals.static_file_serving {
        "What static file, asset, filesystem root, or file response behavior does this definition establish?".to_string()
    } else if signals.response_rendering {
        "What response rendering, template selection, or output format behavior does this definition establish?".to_string()
    } else if signals.auth_session {
        "What authentication decisions, credentials, or session boundaries does this definition establish?".to_string()
    } else if signals.network_http {
        "What network client, proxy, redirect, adapter, or transport behavior does this definition establish?".to_string()
    } else if signals.tls_certificate {
        "What TLS, SSL, certificate, CA bundle, or verification behavior does this definition establish?".to_string()
    } else if signals.reference_search {
        "What reference search, usage classification, definition filtering, or result-shaping behavior does this definition establish?".to_string()
    } else if signals.call_graph_traversal {
        "What call extraction, caller/callee traversal, path bounding, or result-shaping behavior does this definition establish?".to_string()
    } else if signals.symbol_search {
        "What symbol lookup, matching, ranking, limit, or result-shaping behavior does this definition establish?".to_string()
    } else if signals.import_resolution {
        "What import parsing, alias resolution, package metadata, or local target mapping behavior does this definition establish?".to_string()
    } else if signals.project_overview {
        "What project summary, entrypoint candidates, directory role, or next-tool recommendation behavior does this definition establish?".to_string()
    } else if signals.indexing_pipeline {
        "What source scanning, parsing, symbol extraction, dependency extraction, or index-write behavior does this definition establish?".to_string()
    } else if signals.dependency_graph {
        "What dependency graph extraction, local edge resolution, filtering, or graph output behavior does this definition establish?".to_string()
    } else if signals.embedding_provider_status {
        "What embedding provider status, diagnostic, normalization, or reporting behavior does this definition establish?".to_string()
    } else if signals.semantic_context_orchestration {
        "What semantic search orchestration, chunk selection, fallback, or embedding context behavior does this definition establish?".to_string()
    } else if signals.validation_binding {
        "What validation, schema, binding, parser, or serialization behavior does this definition establish?".to_string()
    } else if signals.feature_flags {
        "What feature flag, rollout, toggle, variant, or experiment behavior does this definition establish?".to_string()
    } else if signals.configuration {
        "What configuration defaults, inputs, or environment behavior does this definition establish?".to_string()
    } else if signals.startup {
        "What startup or initialization role does this definition establish?".to_string()
    } else if signals.middleware {
        "What middleware or handler boundary does this definition establish?".to_string()
    } else if signals.request_lifecycle {
        "What request lifecycle, dispatch, or response finalization behavior does this definition establish?".to_string()
    } else if signals.runtime_lifecycle {
        "What runtime execution, script runner, rerun, or shutdown behavior does this definition establish?".to_string()
    } else if signals.file_upload {
        "What uploaded file storage, retrieval, cleanup, or exposure behavior does this definition establish?".to_string()
    } else if signals.websocket_connection {
        "What WebSocket connection, session, message, or close behavior does this definition establish?".to_string()
    } else if signals.performance_cache {
        "What cache, performance, latency, or optimization behavior does this definition establish?"
            .to_string()
    } else if signals.observability_logging {
        "What logging, telemetry, metrics, or tracing behavior does this definition establish?"
            .to_string()
    } else if signals.dependency_injection {
        "What dependency injection, dependency resolution, or parameter injection behavior does this definition establish?".to_string()
    } else if signals.security_safety {
        "What security check, secret handling, sanitization, or vulnerability behavior does this definition establish?".to_string()
    } else if signals.billing_payment {
        "What billing, payment, checkout, invoice, or subscription behavior does this definition establish?".to_string()
    } else if signals.frontend_ui {
        "What frontend component, page, screen, form, or layout behavior does this definition establish?".to_string()
    } else if signals.background_jobs {
        "What background job, queue, worker, or scheduler behavior does this definition establish?"
            .to_string()
    } else if signals.api_handler {
        "What API handler, request, response, or controller boundary does this definition establish?"
            .to_string()
    } else if signals.documentation {
        "What documented setup, usage, or example behavior does this definition support?"
            .to_string()
    } else if signals.data_persistence {
        "What database access, persistence decision, or storage boundary does this definition establish?".to_string()
    } else if signals.error_recovery {
        "What error handling, retry, timeout, or recovery decision does this definition establish?"
            .to_string()
    } else if signals.impact_flow {
        "What callers, callees, or impact paths does this definition anchor?".to_string()
    } else {
        "What behavior or contract does this definition establish for the task?".to_string()
    }
}

fn context_type_relation_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.auth_session {
        "Which base types or interfaces can change authentication decisions or session boundaries?"
            .to_string()
    } else if signals.api_handler {
        "Which base controller or interface contract shapes this API handler behavior?".to_string()
    } else if signals.security_safety {
        "Which inherited contract or base behavior affects this security boundary?".to_string()
    } else if signals.runtime_lifecycle {
        "Which inherited contract or base behavior affects this runtime lifecycle boundary?"
            .to_string()
    } else if signals.file_upload {
        "Which inherited contract or base behavior affects this uploaded file boundary?".to_string()
    } else if signals.websocket_connection {
        "Which inherited contract or base behavior affects this WebSocket session boundary?"
            .to_string()
    } else if signals.dependency_injection {
        "Which inherited contract or base behavior affects this dependency injection boundary?"
            .to_string()
    } else if signals.impact_flow {
        "Which base types or interfaces widen the caller, callee, or impact path?".to_string()
    } else {
        "Which inherited contract or base type behavior is required to understand this type?"
            .to_string()
    }
}

fn context_call_graph_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.test_coverage {
        "Which callers or callees exercise behavior through tests, specs, or regression cases?"
            .to_string()
    } else if signals.response_headers {
        "Which callers or callees set response headers, write status metadata, or choose Content-Type values?".to_string()
    } else if signals.response_cookies {
        "Which callers or callees create response cookies, append Set-Cookie headers, or apply cookie options?".to_string()
    } else if signals.route_parameters {
        "Which callers or callees capture route parameters, attach path variables, or pass them into handlers?".to_string()
    } else if signals.url_building {
        "Which callers or callees build URLs, reverse routes, or join route paths?".to_string()
    } else if signals.route_grouping {
        "Which callers or callees mount routers, register blueprints, create groups, or attach nested handlers?"
            .to_string()
    } else if signals.route_miss_handling {
        "Which callers or callees decide route misses, 404/405 responses, or method-not-allowed fallbacks?"
            .to_string()
    } else if signals.http_method_routing {
        "Which callers or callees register HTTP methods, match verbs, or dispatch handlers?"
            .to_string()
    } else if signals.request_body_parsing {
        "Which callers or callees parse request bodies, select content-type binders, or read form data?".to_string()
    } else if signals.request_query_params {
        "Which callers or callees parse query strings, read request args, or expose URL parameters?"
            .to_string()
    } else if signals.response_redirect {
        "Which callers or callees issue redirects, select redirect status codes, or set Location headers?".to_string()
    } else if signals.static_file_serving {
        "Which callers or callees register static routes, open filesystem roots, or serve file responses?".to_string()
    } else if signals.response_rendering {
        "Which callers or callees select response renderers, templates, or output formats?"
            .to_string()
    } else if signals.auth_session {
        "Which callers or callees carry authentication decisions, credentials, or session state through this flow?".to_string()
    } else if signals.network_http {
        "Which callers or callees send requests, select proxies, follow redirects, or configure transports?".to_string()
    } else if signals.tls_certificate {
        "Which callers or callees verify TLS certificates, configure SSL, or pass CA/cert inputs?"
            .to_string()
    } else if signals.reference_search {
        "Which callers or callees find references, classify usage kinds, filter definitions, or shape reference results?"
            .to_string()
    } else if signals.call_graph_traversal {
        "Which callers or callees extract calls, traverse caller/callee edges, bound paths, or shape call graph results?"
            .to_string()
    } else if signals.validation_binding {
        "Which callers or callees validate inputs, bind payloads, parse schemas, or serialize data?"
            .to_string()
    } else if signals.feature_flags {
        "Which callers or callees evaluate, override, or consume feature flags and rollout state?"
            .to_string()
    } else if signals.configuration {
        "Which callers or callees read, transform, or propagate configuration in this flow?"
            .to_string()
    } else if signals.startup {
        "Which callers or callees order startup, bootstrap, or initialization work in this flow?"
            .to_string()
    } else if signals.middleware {
        "Which callers or callees enter, wrap, or exit middleware and handler boundaries?"
            .to_string()
    } else if signals.request_lifecycle {
        "Which callers or callees move requests through dispatch, hooks, and response finalization?"
            .to_string()
    } else if signals.runtime_lifecycle {
        "Which callers or callees execute scripts, coordinate reruns, or transition runtime lifecycle state?"
            .to_string()
    } else if signals.file_upload {
        "Which callers or callees store, retrieve, clean up, or expose uploaded files?".to_string()
    } else if signals.websocket_connection {
        "Which callers or callees open, track, hand off, message, or close WebSocket connections?"
            .to_string()
    } else if signals.performance_cache {
        "Which callers or callees read, write, invalidate, or optimize cached work?".to_string()
    } else if signals.observability_logging {
        "Which callers or callees emit logs, record metrics, or propagate telemetry and traces?"
            .to_string()
    } else if signals.dependency_injection {
        "Which callers or callees declare, resolve, inject, or consume dependencies?".to_string()
    } else if signals.security_safety {
        "Which callers or callees enforce security checks, sanitize data, or handle secrets?"
            .to_string()
    } else if signals.billing_payment {
        "Which callers or callees create checkout, invoice, payment, or subscription flow?"
            .to_string()
    } else if signals.frontend_ui {
        "Which callers or callees render, mount, or compose frontend UI?".to_string()
    } else if signals.background_jobs {
        "Which callers or callees enqueue, schedule, or execute background work?".to_string()
    } else if signals.api_handler {
        "Which callers or callees route API requests through handlers or controllers?".to_string()
    } else if signals.documentation {
        "Which callers or callees implement the documented workflow or usage example?".to_string()
    } else if signals.project_overview {
        "Which callers or callees assemble project summaries, entrypoint candidates, directory roles, or recommended next tools?"
            .to_string()
    } else if signals.indexing_pipeline {
        "Which callers or callees scan files, parse languages, extract symbols, capture dependencies, or write index records?"
            .to_string()
    } else if signals.dependency_graph {
        "Which callers or callees extract dependency edges, resolve local targets, filter graph data, or format graph output?"
            .to_string()
    } else if signals.embedding_provider_status {
        "Which callers or callees detect provider status, assemble diagnostics, normalize settings, or report embedding readiness?"
            .to_string()
    } else if signals.semantic_context_orchestration {
        "Which callers or callees route semantic search, select chunks, apply fallback, or assemble context results?"
            .to_string()
    } else if signals.data_persistence {
        "Which callers or callees read, write, or persist data through this flow?".to_string()
    } else if signals.error_recovery {
        "Which callers or callees propagate errors, trigger retries, or recover from failures?"
            .to_string()
    } else if signals.impact_flow {
        "Which callers, callees, or impact paths explain how control moves through this flow?"
            .to_string()
    } else {
        "Which callers or callees explain how control moves through this flow?".to_string()
    }
}

fn context_dependency_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.test_coverage {
        "What imported local dependency behavior supplies test setup, fixtures, or assertions?"
            .to_string()
    } else if signals.response_headers {
        "What imported local dependency behavior supplies response headers, status metadata, or Content-Type values?".to_string()
    } else if signals.response_cookies {
        "What imported local dependency behavior supplies response cookies, Set-Cookie headers, or cookie options?".to_string()
    } else if signals.route_parameters {
        "What imported local dependency behavior supplies route parameter capture or path variable dispatch?".to_string()
    } else if signals.url_building {
        "What imported local dependency behavior supplies URL building or route path joining?"
            .to_string()
    } else if signals.route_grouping {
        "What imported local dependency behavior supplies router mounting, blueprint registration, route groups, or prefixes?"
            .to_string()
    } else if signals.route_miss_handling {
        "What imported local dependency behavior supplies not-found, method-not-allowed, 404/405, or final handler behavior?"
            .to_string()
    } else if signals.http_method_routing {
        "What imported local dependency behavior supplies HTTP method routing or verb dispatch?"
            .to_string()
    } else if signals.request_body_parsing {
        "What imported local dependency behavior supplies body parsers, payload binders, or form-data handling?".to_string()
    } else if signals.request_query_params {
        "What imported local dependency behavior supplies query parsing or request parameter access?".to_string()
    } else if signals.response_redirect {
        "What imported local dependency behavior supplies redirect responses, status codes, or Location headers?".to_string()
    } else if signals.static_file_serving {
        "What imported local dependency behavior supplies static files, asset roots, or file responses?".to_string()
    } else if signals.response_rendering {
        "What imported local dependency behavior shapes response rendering or output formats?"
            .to_string()
    } else if signals.auth_session {
        "What imported local dependency behavior affects authentication or session boundaries here?"
            .to_string()
    } else if signals.network_http {
        "What imported local dependency behavior supplies network client, proxy, redirect, adapter, or transport behavior?".to_string()
    } else if signals.tls_certificate {
        "What imported local dependency behavior supplies TLS verification, SSL context, certificates, or CA bundles?".to_string()
    } else if signals.reference_search {
        "What imported local dependency behavior supplies reference search, usage classification, definition filtering, or result shaping?".to_string()
    } else if signals.call_graph_traversal {
        "What imported local dependency behavior supplies call extraction, caller/callee traversal, path bounding, or result shaping?".to_string()
    } else if signals.validation_binding {
        "What imported local dependency behavior supplies validation, binding, parsing, or serialization?".to_string()
    } else if signals.feature_flags {
        "What imported local dependency behavior supplies feature flag, rollout, toggle, or experiment state?".to_string()
    } else if signals.configuration {
        "What imported local dependency behavior supplies configuration defaults, inputs, or environment handling?".to_string()
    } else if signals.startup {
        "What imported local dependency behavior participates in startup or initialization?"
            .to_string()
    } else if signals.middleware {
        "What imported local dependency behavior shapes middleware or handler dispatch?".to_string()
    } else if signals.request_lifecycle {
        "What imported local dependency behavior shapes request lifecycle, dispatch, or response finalization?".to_string()
    } else if signals.runtime_lifecycle {
        "What imported local dependency behavior shapes runtime execution, script runner, or rerun lifecycle?".to_string()
    } else if signals.file_upload {
        "What imported local dependency behavior shapes uploaded file storage, retrieval, or cleanup?".to_string()
    } else if signals.websocket_connection {
        "What imported local dependency behavior shapes WebSocket connection, session, or message lifecycle?".to_string()
    } else if signals.performance_cache {
        "What imported local dependency behavior shapes cache, latency, or optimization flow?"
            .to_string()
    } else if signals.observability_logging {
        "What imported local dependency behavior shapes logging, metrics, telemetry, or tracing?"
            .to_string()
    } else if signals.dependency_injection {
        "What imported local dependency behavior shapes dependency injection or dependency resolution?".to_string()
    } else if signals.security_safety {
        "What imported local dependency behavior shapes security checks, sanitization, or secrets?"
            .to_string()
    } else if signals.billing_payment {
        "What imported local dependency behavior shapes billing, payment, or subscription flow?"
            .to_string()
    } else if signals.frontend_ui {
        "What imported local dependency behavior shapes frontend rendering or component composition?"
            .to_string()
    } else if signals.background_jobs {
        "What imported local dependency behavior shapes queue, worker, or scheduler dispatch?"
            .to_string()
    } else if signals.api_handler {
        "What imported local dependency behavior shapes API handler or controller dispatch?"
            .to_string()
    } else if signals.documentation {
        "What imported local dependency behavior supports the documented workflow or examples?"
            .to_string()
    } else if signals.project_overview {
        "What imported local dependency behavior supplies project summaries, entrypoint candidates, directory roles, or next-tool recommendations?"
            .to_string()
    } else if signals.indexing_pipeline {
        "What imported local dependency behavior supplies source scanning, parsing, symbol extraction, dependency extraction, or index storage?"
            .to_string()
    } else if signals.dependency_graph {
        "What imported local dependency behavior supplies dependency edges, local target resolution, filtering, or graph output?"
            .to_string()
    } else if signals.embedding_provider_status {
        "What imported local dependency behavior supplies embedding provider status, diagnostics, settings, or reporting data?"
            .to_string()
    } else if signals.semantic_context_orchestration {
        "What imported local dependency behavior supplies semantic chunks, embeddings, fallback ranges, or context assembly?"
            .to_string()
    } else if signals.data_persistence {
        "What imported local dependency behavior supplies database, repository, or storage access?"
            .to_string()
    } else if signals.error_recovery {
        "What imported local dependency behavior supplies error handling, retry, or timeout behavior?"
            .to_string()
    } else {
        "What imported local dependency behavior is required to understand this file?".to_string()
    }
}

fn context_reference_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.test_coverage {
        "Which references exercise behavior through tests, specs, fixtures, or regression cases?"
            .to_string()
    } else if signals.response_headers {
        "Which references set response headers, update status metadata, or select Content-Type values?".to_string()
    } else if signals.response_cookies {
        "Which references create response cookies, append Set-Cookie headers, or apply cookie options?".to_string()
    } else if signals.route_parameters {
        "Which references capture, attach, or read route parameters and path variables?".to_string()
    } else if signals.url_building {
        "Which references build URLs, reverse routes, or join route paths?".to_string()
    } else if signals.route_grouping {
        "Which references mount routers, register blueprints, create route groups, or attach nested routes?"
            .to_string()
    } else if signals.route_miss_handling {
        "Which references configure not-found handlers, method-not-allowed handlers, or 404/405 fallbacks?"
            .to_string()
    } else if signals.http_method_routing {
        "Which references register HTTP methods, match verbs, or dispatch handlers?".to_string()
    } else if signals.request_body_parsing {
        "Which references parse request bodies, bind payloads, choose content types, or read form data?".to_string()
    } else if signals.request_query_params {
        "Which references read query strings, request args, or URL parameters?".to_string()
    } else if signals.response_redirect {
        "Which references issue redirect responses, choose redirect status codes, or set Location headers?".to_string()
    } else if signals.static_file_serving {
        "Which references register static routes, configure asset roots, or serve file responses?"
            .to_string()
    } else if signals.response_rendering {
        "Which references render responses, select templates, or produce output formats?"
            .to_string()
    } else if signals.auth_session {
        "Which references consume authentication decisions, credentials, or session state?"
            .to_string()
    } else if signals.network_http {
        "Which references send requests, select proxies, follow redirects, or configure transports?"
            .to_string()
    } else if signals.tls_certificate {
        "Which references verify TLS certificates, configure SSL, or pass CA/cert inputs?"
            .to_string()
    } else if signals.reference_search {
        "Which references show reference search, usage classification, definition filtering, or result shaping?"
            .to_string()
    } else if signals.call_graph_traversal {
        "Which references show caller/callee lookup, call extraction, traversal bounds, or path shaping?"
            .to_string()
    } else if signals.validation_binding {
        "Which references validate inputs, bind payloads, parse schemas, or serialize data?"
            .to_string()
    } else if signals.feature_flags {
        "Which references evaluate, override, or consume feature flags, variants, or rollout state?"
            .to_string()
    } else if signals.configuration {
        "Which references read, override, or pass configuration values?".to_string()
    } else if signals.startup {
        "Which references register or trigger startup and initialization behavior?".to_string()
    } else if signals.middleware {
        "Which references attach, order, or call middleware and handler boundaries?".to_string()
    } else if signals.request_lifecycle {
        "Which references enter, hook into, or finalize the request lifecycle?".to_string()
    } else if signals.runtime_lifecycle {
        "Which references execute scripts, coordinate reruns, or transition runtime lifecycle state?"
            .to_string()
    } else if signals.file_upload {
        "Which references store, retrieve, clean up, or expose uploaded files?".to_string()
    } else if signals.websocket_connection {
        "Which references open, track, hand off, message, or close WebSocket connections?"
            .to_string()
    } else if signals.performance_cache {
        "Which references read, write, invalidate, measure, or optimize cache behavior?".to_string()
    } else if signals.observability_logging {
        "Which references emit logs, record metrics, attach spans, or propagate telemetry?"
            .to_string()
    } else if signals.dependency_injection {
        "Which references declare, resolve, inject, or consume dependencies?".to_string()
    } else if signals.security_safety {
        "Which references enforce security checks, sanitize input, handle secrets, or guard vulnerabilities?".to_string()
    } else if signals.billing_payment {
        "Which references create, update, charge, invoice, or cancel billing flows?".to_string()
    } else if signals.frontend_ui {
        "Which references render, mount, compose, or style frontend UI?".to_string()
    } else if signals.background_jobs {
        "Which references enqueue, schedule, trigger, or run background workers?".to_string()
    } else if signals.api_handler {
        "Which references register, route, or invoke API handlers and controllers?".to_string()
    } else if signals.documentation {
        "Which references connect documentation, examples, or guides to implementation?".to_string()
    } else if signals.data_persistence {
        "Which references read, write, or persist data through this boundary?".to_string()
    } else if signals.error_recovery {
        "Which references catch, wrap, retry, timeout, or recover from failures?".to_string()
    } else if signals.impact_flow {
        "Which references show production usage or impact paths for this seed?".to_string()
    } else {
        "How is the seed symbol used by nearby production code?".to_string()
    }
}

fn context_semantic_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.test_coverage {
        "Which semantic matches describe tests, specs, fixtures, or regression coverage?"
            .to_string()
    } else if signals.response_headers {
        "Which semantic matches describe response headers, status metadata, or Content-Type behavior?"
            .to_string()
    } else if signals.response_cookies {
        "Which semantic matches describe response cookies, Set-Cookie headers, or cookie options?"
            .to_string()
    } else if signals.route_parameters {
        "Which semantic matches describe route parameters, path variables, or wildcard extraction?"
            .to_string()
    } else if signals.url_building {
        "Which semantic matches describe URL building, reverse routing, or route path joining?"
            .to_string()
    } else if signals.route_grouping {
        "Which semantic matches describe router mounting, blueprint registration, route groups, or nested routes?"
            .to_string()
    } else if signals.route_miss_handling {
        "Which semantic matches describe route misses, 404/405 responses, not-found handlers, or method-not-allowed fallbacks?"
            .to_string()
    } else if signals.http_method_routing {
        "Which semantic matches describe HTTP method routing, verb registration, or dispatch?"
            .to_string()
    } else if signals.request_body_parsing {
        "Which semantic matches describe request body parsing, payload binding, or form data?"
            .to_string()
    } else if signals.request_query_params {
        "Which semantic matches describe query string parsing or request parameter access?"
            .to_string()
    } else if signals.response_redirect {
        "Which semantic matches describe redirect responses, status codes, or Location headers?"
            .to_string()
    } else if signals.static_file_serving {
        "Which semantic matches describe static file serving, asset roots, or file responses?"
            .to_string()
    } else if signals.response_rendering {
        "Which semantic matches describe response rendering, templates, or output formats?"
            .to_string()
    } else if signals.auth_session {
        "Which semantic matches describe authentication, credential, cookie, or session behavior?"
            .to_string()
    } else if signals.network_http {
        "Which semantic matches describe network clients, proxies, redirects, adapters, or transports?"
            .to_string()
    } else if signals.tls_certificate {
        "Which semantic matches describe TLS, SSL, certificates, CA bundles, or verification?"
            .to_string()
    } else if signals.embedding_provider_status {
        "Which semantic matches describe embedding provider status, diagnostics, readiness, or reporting?"
            .to_string()
    } else if signals.reference_search {
        "Which semantic matches describe reference search, usage classification, or definition filtering?"
            .to_string()
    } else if signals.call_graph_traversal {
        "Which semantic matches describe call graph extraction, caller/callee traversal, or path shaping?"
            .to_string()
    } else if signals.validation_binding {
        "Which semantic matches describe validation, schemas, bindings, parsers, or serializers?"
            .to_string()
    } else if signals.feature_flags {
        "Which semantic matches describe feature flags, rollouts, toggles, variants, or experiments?"
            .to_string()
    } else if signals.configuration {
        "Which semantic matches describe configuration defaults, inputs, or environment behavior?"
            .to_string()
    } else if signals.startup {
        "Which semantic matches describe startup, bootstrap, or initialization behavior?"
            .to_string()
    } else if signals.middleware {
        "Which semantic matches describe middleware or handler boundary behavior?".to_string()
    } else if signals.request_lifecycle {
        "Which semantic matches describe request lifecycle hooks, dispatch, or response finalization?"
            .to_string()
    } else if signals.runtime_lifecycle {
        "Which semantic matches describe runtime execution, script runner lifecycle, reruns, or shutdown?"
            .to_string()
    } else if signals.file_upload {
        "Which semantic matches describe uploaded file storage, retrieval, cleanup, or exposure?"
            .to_string()
    } else if signals.websocket_connection {
        "Which semantic matches describe WebSocket connections, sessions, messages, or close handling?"
            .to_string()
    } else if signals.performance_cache {
        "Which semantic matches describe cache behavior, performance, latency, or optimization?"
            .to_string()
    } else if signals.observability_logging {
        "Which semantic matches describe logs, metrics, telemetry, tracing, or monitoring?"
            .to_string()
    } else if signals.dependency_injection {
        "Which semantic matches describe dependency injection, dependency resolution, or parameter injection?"
            .to_string()
    } else if signals.security_safety {
        "Which semantic matches describe security, sanitization, secrets, or vulnerabilities?"
            .to_string()
    } else if signals.billing_payment {
        "Which semantic matches describe billing, payments, checkout, invoices, or subscriptions?"
            .to_string()
    } else if signals.frontend_ui {
        "Which semantic matches describe frontend UI, components, pages, forms, or layout?"
            .to_string()
    } else if signals.background_jobs {
        "Which semantic matches describe background jobs, queues, workers, or scheduled runs?"
            .to_string()
    } else if signals.api_handler {
        "Which semantic matches describe API handlers, controllers, endpoints, or request flow?"
            .to_string()
    } else if signals.documentation {
        "Which semantic matches describe documentation, guides, examples, or usage workflows?"
            .to_string()
    } else if signals.data_persistence {
        "Which semantic matches describe database, repository, or storage behavior?".to_string()
    } else if signals.error_recovery {
        "Which semantic matches describe error handling, retry, timeout, or recovery behavior?"
            .to_string()
    } else {
        "Which task terms are reflected in this semantically related code?".to_string()
    }
}

#[derive(Debug, Clone, Copy)]
struct ContextTaskSignals {
    agent_first_read: bool,
    current_reading_step_contract: bool,
    blocked_no_seed_route: bool,
    recommended_next_tools_contract: bool,
    project_entrypoint_ranking: bool,
    budget_continuation: bool,
    impact_suggested_checks: bool,
    mcp_tool_schema_validation: bool,
    config_status_reporting: bool,
    semantic_index_explain: bool,
    semantic_provider_fallback: bool,
    impact_flow: bool,
    auth_session: bool,
    network_http: bool,
    tls_certificate: bool,
    symbol_search: bool,
    import_resolution: bool,
    project_overview: bool,
    indexing_pipeline: bool,
    dependency_graph: bool,
    semantic_context_orchestration: bool,
    embedding_provider_status: bool,
    reference_search: bool,
    call_graph_traversal: bool,
    file_parsing_language: bool,
    validation_binding: bool,
    feature_flags: bool,
    configuration: bool,
    startup: bool,
    middleware: bool,
    performance_cache: bool,
    observability_logging: bool,
    http_state_headers: bool,
    request_body_parsing: bool,
    request_query_params: bool,
    response_headers: bool,
    response_cookies: bool,
    route_parameters: bool,
    url_building: bool,
    route_grouping: bool,
    route_miss_handling: bool,
    http_method_routing: bool,
    route_dispatch: bool,
    response_redirect: bool,
    static_file_serving: bool,
    response_rendering: bool,
    dependency_injection: bool,
    security_safety: bool,
    billing_payment: bool,
    frontend_ui: bool,
    background_jobs: bool,
    request_lifecycle: bool,
    runtime_lifecycle: bool,
    file_upload: bool,
    websocket_connection: bool,
    api_handler: bool,
    documentation: bool,
    data_persistence: bool,
    error_recovery: bool,
    test_coverage: bool,
}

impl ContextTaskSignals {
    fn from_task(task: &str) -> Self {
        let keywords = task_keywords(task);
        let blocked_no_seed_route = auto_seed_blocked_no_seed_route_task(&keywords);
        let recommended_next_tools_contract =
            auto_seed_recommended_next_tools_contract_task(&keywords);
        let project_entrypoint_ranking = auto_seed_project_entrypoint_ranking_task(&keywords);
        let budget_continuation = auto_seed_budget_continuation_task(&keywords);
        let impact_suggested_checks = auto_seed_impact_suggested_checks_task(&keywords);
        let mcp_tool_schema_validation = auto_seed_mcp_tool_schema_validation_task(&keywords);
        let config_status_reporting = auto_seed_config_status_reporting_task(&keywords);
        let semantic_index_explain = auto_seed_semantic_index_explain_task(&keywords);
        let current_reading_step_contract = auto_seed_current_reading_step_contract_task(&keywords);
        let semantic_provider_fallback = auto_seed_semantic_provider_fallback_task(&keywords);
        let agent_first_read = current_reading_step_contract
            || context_text_mentions(
                task,
                &[
                    "agent",
                    "ai agent",
                    "coding agent",
                    "first read",
                    "first-read",
                    "first reading",
                    "first-read workflow",
                    "reading plan",
                    "reading_plan",
                    "execution plan",
                    "execution_plan",
                    "suggested tool",
                    "suggested_tool",
                    "omitted candidate",
                    "omitted candidates",
                    "context pack",
                    "context_pack",
                    "context router",
                    "context routing",
                    "route quality",
                    "routing quality",
                    "adoption evidence",
                    "read less",
                    "read-less",
                    "source line reduction",
                    "line reduction",
                    "selection rank",
                    "selection reason",
                ],
            ) && !context_text_mentions(
                task,
                &[
                    "express",
                    "gin",
                    "rails",
                    "django",
                    "flask",
                    "http method",
                    "route parameter",
                    "route group",
                    "404",
                    "405",
                ],
            );

        let request_lifecycle =
            context_text_mentions(task, &["request", "requests", "response", "responses"])
                && context_text_mentions(
                    task,
                    &[
                        "lifecycle",
                        "before",
                        "after",
                        "dispatch",
                        "handling",
                        "handle",
                        "handler",
                        "handlers",
                    ],
                );
        let runtime_lifecycle =
            context_text_mentions(
                task,
                &[
                    "script runner",
                    "script lifecycle",
                    "runner lifecycle",
                    "runtime lifecycle",
                    "execution lifecycle",
                ],
            ) || (context_text_mentions(task, &["script", "runner", "runtime", "execution"])
                && context_text_mentions(
                    task,
                    &["lifecycle", "rerun", "reruns", "run loop", "shutdown"],
                ));
        let file_upload =
            context_text_mentions(
                task,
                &[
                    "file upload",
                    "file uploader",
                    "file uploads",
                    "uploaded file",
                    "uploaded files",
                    "upload manager",
                    "uploaded file manager",
                ],
            ) || (context_text_mentions(task, &["upload", "uploaded", "uploader", "uploads"])
                && context_text_mentions(task, &["file", "files", "manager", "storage"]));
        let websocket_connection =
            context_text_mentions(
                task,
                &[
                    "websocket",
                    "websockets",
                    "web socket",
                    "web sockets",
                    "socket connection",
                    "socket connections",
                ],
            ) || (context_text_mentions(task, &["socket", "sockets", "connection", "connections"])
                && context_text_mentions(task, &["session", "sessions", "manager"]));
        let dependency_injection =
            context_text_mentions(task, &["dependency injection", "dependency resolver"])
                || (context_text_mentions(task, &["dependency", "dependencies", "depends"])
                    && context_text_mentions(
                        task,
                        &["injection", "inject", "injected", "resolver", "resolution"],
                    ));
        let file_parsing_language =
            context_text_mentions(
                task,
                &[
                    "file parsing",
                    "source parsing",
                    "code parsing",
                    "parse file",
                    "parse files",
                    "ast",
                    "abstract syntax tree",
                    "tree-sitter",
                    "language support",
                    "language detection",
                ],
            ) || (context_text_mentions(task, &["parse", "parser", "parsing"])
                && context_text_mentions(task, &["language", "languages", "support", "supported"]));

        let http_state_headers = context_text_mentions(
            task,
            &[
                "cookie",
                "cookies",
                "cookie jar",
                "cookiejar",
                "header",
                "headers",
                "case insensitive",
                "case-insensitive",
                "http state",
            ],
        );
        let response_rendering = context_text_mentions(task, &["render", "renderer", "rendering"])
            && context_text_mentions(
                task,
                &[
                    "response",
                    "responses",
                    "http",
                    "handler",
                    "output",
                    "outputs",
                    "template",
                    "templates",
                ],
            )
            && !context_text_mentions(
                task,
                &[
                    "frontend",
                    "front-end",
                    "ui",
                    "component",
                    "components",
                    "page",
                    "pages",
                    "screen",
                    "screens",
                ],
            );
        let static_file_serving =
            context_text_mentions(
                task,
                &[
                    "static file",
                    "static files",
                    "static asset",
                    "static assets",
                    "asset serving",
                    "assets serving",
                    "file serving",
                    "filesystem serving",
                    "file system serving",
                    "send file",
                    "sendfile",
                    "send static",
                ],
            ) || (context_text_mentions(task, &["static", "asset", "assets"])
                && context_text_mentions(
                    task,
                    &[
                        "file",
                        "files",
                        "folder",
                        "folders",
                        "directory",
                        "directories",
                        "serve",
                        "serving",
                        "filesystem",
                        "file system",
                    ],
                ));
        let request_body_parsing =
            context_text_mentions(
                task,
                &[
                    "request body",
                    "request bodies",
                    "body parsing",
                    "body parser",
                    "body parsers",
                    "payload binding",
                    "payload parsing",
                    "form data",
                    "form-data",
                    "multipart form",
                    "content type binding",
                    "content-type binding",
                ],
            ) || (context_text_mentions(task, &["body", "payload", "form", "multipart"])
                && context_text_mentions(
                    task,
                    &[
                        "parse",
                        "parser",
                        "parsing",
                        "bind",
                        "binding",
                        "bindings",
                        "decode",
                        "decoder",
                        "content type",
                        "content-type",
                    ],
                ));
        let request_query_params = context_text_mentions(
            task,
            &[
                "query string",
                "query strings",
                "query parameter",
                "query parameters",
                "query param",
                "query params",
                "request args",
                "request arguments",
                "url parameter",
                "url parameters",
                "url params",
            ],
        ) || (context_text_mentions(task, &["query", "queries"])
            && context_text_mentions(
                task,
                &[
                    "parameter",
                    "parameters",
                    "param",
                    "params",
                    "args",
                    "arguments",
                    "parse",
                    "parser",
                    "parsing",
                    "url",
                    "request",
                ],
            )
            && !context_text_mentions(
                task,
                &[
                    "database",
                    "sql",
                    "graphql",
                    "search index",
                    "semantic search",
                ],
            ));
        let route_parameters = (context_text_mentions(
            task,
            &[
                "route parameter",
                "route parameters",
                "route param",
                "route params",
                "path parameter",
                "path parameters",
                "path param",
                "path params",
                "path variable",
                "path variables",
                "url variable",
                "url variables",
                "route variable",
                "route variables",
                "view args",
                "view_args",
                "wildcard route",
                "wildcard routes",
                "wildcard parameter",
                "wildcard parameters",
            ],
        ) || (context_text_mentions(
            task,
            &[
                "route",
                "routes",
                "router",
                "routing",
                "path",
                "paths",
                "url rule",
                "url rules",
            ],
        ) && context_text_mentions(
            task,
            &[
                "param",
                "params",
                "parameter",
                "parameters",
                "variable",
                "variables",
                "wildcard",
                "wildcards",
                "view args",
                "view_args",
            ],
        ))) && !context_text_mentions(
            task,
            &[
                "query string",
                "query strings",
                "query parameter",
                "query parameters",
                "query param",
                "query params",
                "request args",
                "request arguments",
                "database",
                "sql",
                "graphql",
            ],
        );
        let url_building = context_text_mentions(
            task,
            &[
                "url_for",
                "url for",
                "url building",
                "url builder",
                "url builders",
                "url generation",
                "url generator",
                "url generators",
                "build url",
                "build urls",
                "build a url",
                "build the url",
                "generate url",
                "generate urls",
                "reverse route",
                "reverse routes",
                "reverse routing",
                "route url",
                "route urls",
                "route path joining",
                "path joining",
                "path join",
                "join path",
                "join paths",
                "absolute path",
                "base path",
            ],
        ) || (context_text_mentions(task, &["url", "urls", "path", "paths"])
            && context_text_mentions(
                task,
                &[
                    "build",
                    "builds",
                    "builder",
                    "building",
                    "generate",
                    "generates",
                    "generation",
                    "generator",
                    "reverse",
                    "reversing",
                    "join",
                    "joins",
                    "joining",
                    "absolute",
                    "base",
                ],
            )
            && !context_text_mentions(
                task,
                &[
                    "query string",
                    "query strings",
                    "query parameter",
                    "query parameters",
                    "query param",
                    "query params",
                    "request args",
                    "request arguments",
                    "route parameter",
                    "route parameters",
                    "path parameter",
                    "path parameters",
                    "path variable",
                    "path variables",
                    "static file",
                    "static files",
                    "filesystem",
                    "file upload",
                    "database",
                    "sql",
                    "graphql",
                ],
            ));
        let route_grouping = context_text_mentions(
            task,
            &[
                "route group",
                "route groups",
                "router group",
                "router groups",
                "route grouping",
                "group routing",
                "mounted router",
                "mounted routers",
                "mounted app",
                "mounted application",
                "subrouter",
                "sub-router",
                "sub router",
                "child router",
                "nested router",
                "nested route",
                "nested routes",
                "blueprint routing",
                "blueprint registration",
                "register blueprint",
                "register_blueprint",
                "route mounting",
                "router mounting",
            ],
        ) || (context_text_mentions(
            task,
            &[
                "blueprint",
                "blueprints",
                "register_blueprint",
                "mounted",
                "mount",
                "mounts",
                "mounting",
                "subrouter",
                "nested",
                "group",
                "groups",
            ],
        ) && context_text_mentions(
            task,
            &[
                "route",
                "routes",
                "router",
                "routing",
                "app",
                "application",
                "middleware",
                "handler",
                "handlers",
                "prefix",
                "prefixes",
            ],
        ));
        let route_miss_handling =
            context_text_mentions(
                task,
                &[
                    "not found",
                    "not-found",
                    "notfound",
                    "no route",
                    "noroute",
                    "no method",
                    "nomethod",
                    "method not allowed",
                    "method-not-allowed",
                    "not allowed",
                    "404",
                    "405",
                    "final handler",
                    "finalhandler",
                    "route miss",
                    "route misses",
                    "routing exception",
                    "routing exceptions",
                ],
            ) || (context_text_mentions(
                task,
                &[
                    "404",
                    "405",
                    "notfound",
                    "noroute",
                    "nomethod",
                    "miss",
                    "missing",
                    "fallback",
                    "fallthrough",
                    "finalhandler",
                    "final",
                    "exception",
                ],
            ) && context_text_mentions(
                task,
                &[
                    "route", "routes", "router", "routing", "method", "methods", "handler",
                    "handlers", "http", "request", "response",
                ],
            ) && !context_text_mentions(task, &["template", "templates"]));
        let http_method_routing = context_text_mentions(
            task,
            &[
                "http method",
                "http methods",
                "request method",
                "request methods",
                "method routing",
                "method dispatch",
                "method registration",
                "verb routing",
                "verb dispatch",
                "verb registration",
                "get post",
                "get/post",
                "post put",
                "options head",
            ],
        ) || (context_text_mentions(
            task,
            &["method", "methods", "verb", "verbs"],
        ) && context_text_mentions(
            task,
            &[
                "http",
                "https",
                "request",
                "route",
                "routes",
                "router",
                "routing",
                "dispatch",
                "register",
                "registration",
                "handler",
                "handlers",
                "get",
                "post",
                "put",
                "delete",
                "patch",
                "options",
                "head",
            ],
        ) && !context_text_mentions(
            task,
            &[
                "class method",
                "object method",
                "method call",
                "method calls",
                "network client",
                "proxy",
                "proxies",
                "adapter",
                "adapters",
                "transport",
                "transports",
            ],
        ));
        let generic_route_dispatch =
            context_text_mentions(
                task,
                &[
                    "application routing",
                    "app routing",
                    "engine routing",
                    "router behavior",
                    "routing behavior",
                    "route dispatch",
                    "request routing",
                    "route matching",
                    "route registration",
                    "routing flow",
                ],
            ) || (context_text_mentions(task, &["route", "routes", "router", "routing"])
                && context_text_mentions(
                    task,
                    &[
                        "app",
                        "application",
                        "engine",
                        "handler",
                        "handlers",
                        "dispatch",
                        "match",
                        "matching",
                        "register",
                        "registration",
                        "behavior",
                        "flow",
                    ],
                ));
        let response_headers = context_text_mentions(
            task,
            &[
                "response header",
                "response headers",
                "http response header",
                "http response headers",
                "set response header",
                "set response headers",
                "response metadata",
                "content-type header",
                "content type header",
            ],
        ) || (context_text_mentions(
            task,
            &[
                "header",
                "headers",
                "content type",
                "content-type",
                "contenttype",
            ],
        ) && context_text_mentions(
            task,
            &[
                "response",
                "responses",
                "status",
                "status code",
                "status codes",
                "set",
                "write",
                "writes",
                "send",
                "sends",
                "server",
                "handler",
            ],
        ) && !context_text_mentions(
            task,
            &[
                "request header",
                "request headers",
                "request",
                "requests",
                "client",
                "network client",
                "proxy",
                "proxies",
                "adapter",
                "adapters",
                "transport",
                "transports",
                "binding",
                "bind",
            ],
        ));
        let response_cookies = context_text_mentions(
            task,
            &[
                "response cookie",
                "response cookies",
                "set cookie",
                "set cookies",
                "set-cookie",
                "set-cookie header",
                "set-cookie headers",
                "cookie response",
                "cookie responses",
            ],
        ) || (context_text_mentions(task, &["cookie", "cookies"])
            && context_text_mentions(
                task,
                &[
                    "response",
                    "responses",
                    "set",
                    "sets",
                    "send",
                    "sends",
                    "server",
                    "handler",
                    "option",
                    "options",
                    "header",
                    "headers",
                ],
            )
            && !context_text_mentions(
                task,
                &[
                    "cookie jar",
                    "cookiejar",
                    "jar",
                    "client",
                    "network client",
                    "request cookie",
                    "request cookies",
                    "requests",
                    "case insensitive",
                    "case-insensitive",
                ],
            ));
        let response_redirect = context_text_mentions(
            task,
            &[
                "redirect response",
                "redirect responses",
                "response redirect",
                "response redirects",
                "http redirect",
                "http redirects",
                "location header",
                "location headers",
            ],
        ) || (context_text_mentions(
            task,
            &["redirect", "redirects", "redirection"],
        ) && context_text_mentions(
            task,
            &[
                "response",
                "responses",
                "status",
                "status code",
                "status codes",
                "location",
                "location header",
                "handler",
                "server",
            ],
        ) && !context_text_mentions(
            task,
            &[
                "client",
                "network client",
                "proxy",
                "proxies",
                "adapter",
                "adapters",
                "transport",
                "transports",
            ],
        ));
        let http_client_session = (context_text_mentions(
            task,
            &[
                "requests session",
                "request session",
                "session request",
                "session request flow",
                "client session",
                "http session",
            ],
        ) || (context_text_mentions(task, &["session"])
            && context_text_mentions(
                task,
                &["request flow", "requests", "http client", "network client"],
            )))
            && context_text_mentions(
                task,
                &[
                    "request",
                    "requests",
                    "http",
                    "client",
                    "network",
                    "adapter",
                    "transport",
                    "redirect",
                    "proxy",
                    "flow",
                ],
            )
            && !context_text_mentions(
                task,
                &[
                    "auth",
                    "authentication",
                    "authorization",
                    "login",
                    "signin",
                    "credential",
                    "credentials",
                    "token",
                    "oauth",
                    "jwt",
                    "cookie",
                    "cookies",
                    "security",
                ],
            );

        Self {
            agent_first_read,
            current_reading_step_contract,
            blocked_no_seed_route,
            recommended_next_tools_contract,
            project_entrypoint_ranking,
            budget_continuation,
            impact_suggested_checks,
            mcp_tool_schema_validation,
            config_status_reporting,
            semantic_index_explain,
            semantic_provider_fallback,
            impact_flow: context_text_mentions(
                task,
                &["impact", "caller", "callee", "call path", "call paths"],
            ),
            auth_session: context_text_mentions(
                task,
                &[
                    "auth",
                    "authz",
                    "authentication",
                    "authenticate",
                    "authorization",
                    "authorize",
                    "access control",
                    "access-control",
                    "acl",
                    "rbac",
                    "login",
                    "signin",
                    "permission",
                    "permissions",
                    "session",
                    "credential",
                    "credentials",
                    "token",
                    "tokens",
                    "oauth",
                    "jwt",
                ],
            ) && !http_client_session
                && !budget_continuation,
            network_http: http_client_session
                || context_text_mentions(
                    task,
                    &[
                        "network",
                        "http",
                        "https",
                        "http client",
                        "network client",
                        "proxy",
                        "proxies",
                        "redirect",
                        "redirects",
                        "transport",
                        "transports",
                        "adapter",
                        "adapters",
                    ],
                ),
            tls_certificate: context_text_mentions(
                task,
                &[
                    "tls",
                    "ssl",
                    "certificate",
                    "certificates",
                    "cert",
                    "certs",
                    "ca bundle",
                    "ca bundles",
                    "verify",
                    "verification",
                    "trust",
                    "trusted",
                    "truststore",
                    "ssl context",
                ],
            ),
            symbol_search: auto_seed_symbol_search_task(&keywords),
            import_resolution: auto_seed_import_resolution_task(&keywords),
            project_overview: auto_seed_project_overview_task(&keywords),
            indexing_pipeline: auto_seed_indexing_pipeline_task(&keywords),
            dependency_graph: auto_seed_dependency_graph_task(&keywords),
            semantic_context_orchestration: auto_seed_semantic_context_task(&keywords)
                && auto_seed_semantic_context_prefers_orchestration(&keywords),
            embedding_provider_status: auto_seed_embedding_provider_status_task(&keywords),
            reference_search: auto_seed_reference_search_task(&keywords),
            call_graph_traversal: auto_seed_call_graph_traversal_task(&keywords),
            file_parsing_language,
            validation_binding: context_text_mentions(
                task,
                &[
                    "validation",
                    "validate",
                    "validated",
                    "validator",
                    "validators",
                    "schema",
                    "schemas",
                    "binding",
                    "bindings",
                    "bind",
                    "parser",
                    "parsers",
                    "parse",
                    "parsing",
                    "json",
                    "serialize",
                    "serializer",
                    "serializers",
                    "serialization",
                    "deserialize",
                    "deserializer",
                    "deserializers",
                    "deserialization",
                    "marshal",
                    "unmarshal",
                ],
            ) && !file_parsing_language,
            feature_flags: context_text_mentions(
                task,
                &[
                    "feature flag",
                    "feature flags",
                    "feature-flag",
                    "feature-flags",
                    "flag",
                    "flags",
                    "toggle",
                    "toggles",
                    "rollout",
                    "rollouts",
                    "experiment",
                    "experiments",
                    "variant",
                    "variants",
                ],
            ),
            configuration: context_text_mentions(
                task,
                &["config", "configuration", "setting", "settings"],
            ),
            startup: context_text_mentions(task, &["startup", "bootstrap", "boot"]),
            middleware: context_text_mentions(task, &["middleware"]),
            performance_cache: context_text_mentions(
                task,
                &[
                    "cache",
                    "caches",
                    "cached",
                    "caching",
                    "performance",
                    "perf",
                    "latency",
                    "slow",
                    "slowness",
                    "optimize",
                    "optimization",
                    "optimise",
                    "optimisation",
                ],
            ),
            observability_logging: context_text_mentions(
                task,
                &[
                    "observability",
                    "observe",
                    "telemetry",
                    "logging",
                    "log",
                    "logs",
                    "logger",
                    "metric",
                    "metrics",
                    "trace",
                    "traces",
                    "tracing",
                    "span",
                    "spans",
                    "monitor",
                    "monitoring",
                    "instrumentation",
                ],
            ),
            http_state_headers,
            request_body_parsing,
            request_query_params,
            response_headers,
            response_cookies,
            route_parameters,
            url_building,
            route_grouping,
            route_miss_handling,
            http_method_routing,
            route_dispatch: generic_route_dispatch
                && !route_parameters
                && !url_building
                && !route_grouping
                && !route_miss_handling
                && !http_method_routing
                && !static_file_serving,
            response_redirect,
            static_file_serving,
            response_rendering,
            dependency_injection,
            security_safety: context_text_mentions(
                task,
                &[
                    "security",
                    "secure",
                    "vulnerability",
                    "vulnerabilities",
                    "vuln",
                    "vulns",
                    "secret",
                    "secrets",
                    "encrypt",
                    "encryption",
                    "decrypt",
                    "decryption",
                    "csrf",
                    "xss",
                    "injection",
                    "sanitize",
                    "sanitization",
                    "sanitise",
                    "sanitisation",
                ],
            ) && !dependency_injection,
            billing_payment: context_text_mentions(
                task,
                &[
                    "billing",
                    "bill",
                    "payment",
                    "payments",
                    "checkout",
                    "subscription",
                    "subscriptions",
                    "subscribe",
                    "invoice",
                    "invoices",
                    "pricing",
                    "price",
                    "stripe",
                ],
            ),
            frontend_ui: context_text_mentions(
                task,
                &[
                    "frontend",
                    "front-end",
                    "ui",
                    "component",
                    "components",
                    "page",
                    "pages",
                    "screen",
                    "screens",
                    "form",
                    "forms",
                    "layout",
                    "layouts",
                    "style",
                    "styles",
                    "css",
                ],
            ),
            background_jobs: context_text_mentions(
                task,
                &[
                    "background",
                    "job",
                    "jobs",
                    "queue",
                    "queues",
                    "worker",
                    "workers",
                    "scheduler",
                    "schedulers",
                    "schedule",
                    "scheduled",
                    "cron",
                    "async",
                    "asynchronous",
                ],
            ),
            request_lifecycle,
            runtime_lifecycle,
            file_upload,
            websocket_connection,
            api_handler: context_text_mentions(
                task,
                &[
                    "api",
                    "endpoint",
                    "endpoints",
                    "handler",
                    "handlers",
                    "controller",
                    "controllers",
                    "request",
                    "requests",
                    "response",
                    "responses",
                    "action",
                    "actions",
                ],
            ),
            documentation: context_text_mentions(
                task,
                &[
                    "doc",
                    "docs",
                    "documentation",
                    "readme",
                    "guide",
                    "guides",
                    "usage",
                    "tutorial",
                    "tutorials",
                    "example",
                    "examples",
                ],
            ),
            data_persistence: context_text_mentions(
                task,
                &[
                    "database",
                    "db",
                    "persistence",
                    "persist",
                    "storage",
                    "repository",
                    "query",
                    "queries",
                    "sql",
                ],
            ),
            error_recovery: context_text_mentions(
                task,
                &[
                    "error",
                    "errors",
                    "exception",
                    "exceptions",
                    "failure",
                    "failures",
                    "retry",
                    "retries",
                    "timeout",
                    "timeouts",
                    "debug",
                    "bug",
                    "fallback",
                    "recovery",
                    "recover",
                ],
            ),
            test_coverage: context_text_mentions(
                task,
                &[
                    "test",
                    "tests",
                    "testing",
                    "spec",
                    "specs",
                    "coverage",
                    "regression",
                    "unit",
                    "integration",
                    "e2e",
                ],
            ),
        }
    }
}

fn context_text_mentions(text: &str, terms: &[&str]) -> bool {
    let text_tokens = ascii_word_tokens(text);
    terms.iter().any(|term| {
        let term_tokens = ascii_word_tokens(term);
        !term_tokens.is_empty()
            && text_tokens
                .windows(term_tokens.len())
                .any(|window| window == term_tokens.as_slice())
    })
}

fn ascii_word_tokens(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|token| !token.is_empty())
        .collect()
}

fn context_reading_sources(file: &ContextFile) -> BTreeSet<&str> {
    file.ranges
        .iter()
        .map(|range| range.source.as_str())
        .collect::<BTreeSet<_>>()
}

fn context_range_source_counts(ranges: &[ContextRange]) -> Vec<ContextSourceCount> {
    let mut counts = BTreeMap::<&str, usize>::new();
    for range in ranges {
        *counts.entry(range.source.as_str()).or_default() += 1;
    }

    let mut mix = Vec::new();
    for source in [
        "seed_file",
        "symbol_definition",
        "type_relation",
        "call_graph",
        "reference",
        "dependency",
        "semantic",
    ] {
        if let Some(count) = counts.get(source) {
            mix.push(ContextSourceCount {
                source: context_source_label(source).to_string(),
                count: *count,
            });
        }
    }
    mix
}

fn context_range_source_mix(ranges: &[ContextRange]) -> String {
    let counts = context_range_source_counts(ranges);
    let mut parts = Vec::new();
    for count in counts {
        parts.push(format!("{} x{}", count.source, count.count));
    }

    if parts.is_empty() {
        "evidence mix unavailable".to_string()
    } else {
        format!("evidence mix: {}", parts.join(", "))
    }
}

fn context_range_source_mix_score(ranges: &[ContextCandidateRange]) -> i32 {
    ranges
        .iter()
        .map(|range| match range.source.as_str() {
            "seed_file" => 12,
            "symbol_definition" => 10,
            "type_relation" => 9,
            "call_graph" => 8,
            "reference" => 6,
            "dependency" => 5,
            "semantic" => 3,
            _ => 1,
        })
        .sum()
}

fn context_file_recent_edit_score(path: &Path) -> i32 {
    use std::time::SystemTime;

    let Ok(metadata) = fs::metadata(path) else {
        return 0;
    };
    let Ok(modified) = metadata.modified() else {
        return 0;
    };
    let Ok(age) = SystemTime::now().duration_since(modified) else {
        return 0;
    };

    let age_secs = age.as_secs();
    if age_secs <= 3 * 24 * 60 * 60 {
        12
    } else if age_secs <= 14 * 24 * 60 * 60 {
        8
    } else if age_secs <= 60 * 24 * 60 * 60 {
        4
    } else {
        0
    }
}

fn context_source_label(source: &str) -> &'static str {
    match source {
        "seed_file" => "seed file",
        "symbol_definition" => "symbol definition",
        "type_relation" => "type relation",
        "call_graph" => "call graph",
        "reference" => "reference",
        "dependency" => "dependency",
        "semantic" => "semantic",
        _ => "selected",
    }
}

fn context_source_priority(source: &str) -> i32 {
    match source {
        "seed_file" => 6,
        "symbol_definition" => 5,
        "type_relation" => 4,
        "call_graph" => 4,
        "reference" => 3,
        "dependency" => 2,
        "semantic" => 1,
        _ => 0,
    }
}

fn is_type_relation_dependency(dependency: &Dependency) -> bool {
    matches!(dependency.kind.as_str(), "base_type")
}

fn context_type_relation_terms(seed_symbols: &[String], symbols: &[Symbol]) -> Vec<String> {
    let mut terms = seed_symbols
        .iter()
        .filter(|symbol| !symbol.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    for symbol in symbols {
        if matches!(
            symbol.kind,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Struct
        ) {
            terms.insert(symbol.name.clone());
            terms.insert(symbol.qualified_name.clone());
        }
    }
    terms.into_iter().collect()
}

fn context_type_relation_symbols(store: &Store, dependency: &Dependency) -> Result<Vec<Symbol>> {
    let mut symbols = Vec::new();
    for query in type_relation_target_queries(&dependency.target) {
        symbols.extend(store.search_symbols(&query, 8)?);
    }
    dedup_symbols(&mut symbols);
    symbols.retain(|symbol| {
        matches!(
            symbol.kind,
            SymbolKind::Class | SymbolKind::Interface | SymbolKind::Struct
        ) && symbol_matches_type_relation_target(symbol, &dependency.target)
    });
    symbols.sort_by(|left, right| {
        type_relation_symbol_rank(left, dependency)
            .cmp(&type_relation_symbol_rank(right, dependency))
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.start_line.cmp(&right.start_line))
    });
    symbols.truncate(4);
    Ok(symbols)
}

fn context_type_relation_source_symbols(
    store: &Store,
    dependency: &Dependency,
) -> Result<Vec<Symbol>> {
    let Some(local_alias) = dependency.local_alias.as_deref() else {
        return Ok(Vec::new());
    };
    let mut symbols = store.search_symbols(local_alias, 8)?;
    symbols.retain(|symbol| {
        symbol.file == dependency.source_file
            && matches!(
                symbol.kind,
                SymbolKind::Class | SymbolKind::Interface | SymbolKind::Struct
            )
            && (symbol.name == local_alias || symbol.qualified_name == local_alias)
    });
    symbols.sort_by_key(|symbol| {
        (
            symbol.start_line,
            symbol.end_line,
            symbol.qualified_name.len(),
        )
    });
    symbols.truncate(4);
    Ok(symbols)
}

fn type_relation_target_queries(target: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let target = target.trim();
    if !target.is_empty() {
        queries.push(target.to_string());
    }
    if let Some(simple) = simple_type_relation_target(target)
        && !queries.iter().any(|query| query == simple)
    {
        queries.push(simple.to_string());
    }
    queries
}

fn simple_type_relation_target(target: &str) -> Option<&str> {
    let trimmed = target.trim();
    let without_generic = trimmed.split(['<', '[']).next().unwrap_or(trimmed).trim();
    let simple = without_generic
        .rsplit(['.', ':', '\\'])
        .next()
        .unwrap_or(without_generic)
        .trim();
    (!simple.is_empty()).then_some(simple)
}

fn symbol_matches_type_relation_target(symbol: &Symbol, target: &str) -> bool {
    let simple = simple_type_relation_target(target).unwrap_or(target);
    symbol.name == simple
        || symbol.qualified_name == target
        || symbol.qualified_name == simple
        || symbol
            .qualified_name
            .strip_suffix(simple)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn type_relation_symbol_rank(symbol: &Symbol, dependency: &Dependency) -> (i32, usize) {
    let simple = simple_type_relation_target(&dependency.target).unwrap_or(&dependency.target);
    let exact_name_rank = if symbol.name == simple { 0 } else { 1 };
    let source_dir = Path::new(&dependency.source_file)
        .parent()
        .and_then(|path| path.to_str())
        .filter(|path| !path.is_empty());
    let locality_rank = if symbol.file == dependency.source_file {
        2
    } else if source_dir.is_some_and(|path| symbol.file.starts_with(path)) {
        0
    } else {
        1
    };
    (exact_name_rank + locality_rank, symbol.qualified_name.len())
}

#[derive(Debug)]
struct SemanticVectorContextResult {
    status: ContextSemanticStatus,
    matches: Vec<SemanticSearchResult>,
}

fn semantic_vector_context_matches(
    store: &Store,
    task: &str,
    limit: usize,
) -> SemanticVectorContextResult {
    let mut status = ContextSemanticStatus {
        provider: "disabled".to_string(),
        model: "disabled".to_string(),
        provider_configured: false,
        vector_status: "provider_not_configured".to_string(),
        vector_candidates: 0,
        fallback_candidates: 0,
        selected_vector_ranges: 0,
        selected_fallback_ranges: 0,
        recommendation: String::new(),
    };
    if limit == 0 {
        status.vector_status = "vector_limit_zero".to_string();
        status.recommendation = context_semantic_recommendation(&status);
        return SemanticVectorContextResult {
            status,
            matches: Vec::new(),
        };
    }
    let provider = match embedding::provider_from_env() {
        Ok(provider) => provider,
        Err(error) => {
            status.provider = "unknown".to_string();
            status.model = "unknown".to_string();
            status.vector_status = format!("provider_error: {error}");
            status.recommendation = context_semantic_recommendation(&status);
            return SemanticVectorContextResult {
                status,
                matches: Vec::new(),
            };
        }
    };
    status.provider = provider.provider_name().to_string();
    status.model = provider.model_name().to_string();
    status.provider_configured = provider.is_configured();
    if !provider.is_configured() {
        status.vector_status = "provider_not_configured".to_string();
        status.recommendation = context_semantic_recommendation(&status);
        return SemanticVectorContextResult {
            status,
            matches: Vec::new(),
        };
    }
    let candidates =
        match store.semantic_embedding_matches(provider.provider_name(), provider.model_name()) {
            Ok(candidates) => candidates,
            Err(error) => {
                status.vector_status = format!("index_error: {error}");
                status.recommendation = context_semantic_recommendation(&status);
                return SemanticVectorContextResult {
                    status,
                    matches: Vec::new(),
                };
            }
        };
    status.vector_candidates = candidates.len();
    if candidates.is_empty() {
        status.vector_status = "embeddings_missing_for_provider".to_string();
        status.recommendation = context_semantic_recommendation(&status);
        return SemanticVectorContextResult {
            status,
            matches: Vec::new(),
        };
    };
    let query_embedding = match embedding::embed_query(provider.as_ref(), task) {
        Ok(query_embedding) => query_embedding,
        Err(error) => {
            status.vector_status = format!("provider_error: {error}");
            status.recommendation = context_semantic_recommendation(&status);
            return SemanticVectorContextResult {
                status,
                matches: Vec::new(),
            };
        }
    };

    let mut matches = candidates
        .into_iter()
        .filter_map(|candidate| semantic_search_result(candidate, &query_embedding.values))
        .collect::<Vec<_>>();
    matches.sort_by(compare_semantic_search_results);
    matches.truncate(limit);
    status.vector_status = if matches.is_empty() {
        "vector_matches_empty".to_string()
    } else {
        "vector_matches_available".to_string()
    };
    status.recommendation = context_semantic_recommendation(&status);
    SemanticVectorContextResult { status, matches }
}

fn count_selected_ranges_with_reason(files: &[ContextFile], needle: &str) -> usize {
    files
        .iter()
        .flat_map(|file| &file.ranges)
        .filter(|range| range.reason.contains(needle))
        .count()
}

fn context_semantic_recommendation(status: &ContextSemanticStatus) -> String {
    if status.selected_vector_ranges > 0 {
        return "semantic vector matches were selected".to_string();
    }
    if status.vector_status == "provider_not_configured" {
        if status.fallback_candidates > 0 {
            return format!(
                "{} and run semantic-index to enable vector matches; deterministic semantic chunk fallback was available",
                embedding::provider_help()
            );
        }
        return format!(
            "{} and run semantic-index to enable semantic context",
            embedding::provider_help()
        );
    }
    if status.vector_status == "embeddings_missing_for_provider" {
        return format!(
            "run semantic-index with {}={} to build vectors for this provider/model",
            embedding::PROVIDER_ENV,
            status.provider
        );
    }
    if status.selected_fallback_ranges > 0 {
        return "semantic chunk fallback ranges were selected".to_string();
    }
    if status.fallback_candidates > 0 {
        return "semantic fallback candidates existed but were not selected within the token budget"
            .to_string();
    }
    if status.vector_status.starts_with("provider_error:")
        || status.vector_status.starts_with("index_error:")
    {
        return "fix the reported semantic provider/index error or rely on deterministic context signals"
            .to_string();
    }
    "no semantic context candidates matched; broaden task terms or run semantic-index".to_string()
}

fn semantic_chunks_for_file(
    file: &str,
    source: &str,
    chunk_lines: usize,
) -> Vec<SemanticChunkInput> {
    let lines = source.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    for (chunk_index, chunk) in lines.chunks(chunk_lines).enumerate() {
        let text = chunk.join("\n");
        if text.trim().is_empty() {
            continue;
        }
        let start_line = chunk_index * chunk_lines + 1;
        let end_line = start_line + chunk.len() - 1;
        chunks.push(SemanticChunkInput {
            file: file.to_string(),
            start_line,
            end_line,
            content_hash: hash_text(&text),
            token_estimate: estimate_tokens(&text),
            text,
        });
    }
    chunks
}

fn hash_text(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    format!("{:x}", hasher.finalize())
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

fn normalize_impact_format(format: &str) -> Result<String> {
    match format.trim().to_ascii_lowercase().as_str() {
        "full" => Ok("full".to_string()),
        "summary" => Ok("summary".to_string()),
        other => {
            bail!("unsupported impact analysis format '{other}'; expected 'summary' or 'full'")
        }
    }
}

fn impact_file_symbol_scan_limit(output_limit: usize, seed_file_count: usize) -> usize {
    output_limit
        .max(IMPACT_FILE_SYMBOL_SCAN_PER_FILE.saturating_mul(seed_file_count.max(1)))
        .min(IMPACT_FILE_SYMBOL_SCAN_MAX)
}

fn impact_risk_level(impacted_files: &[ImpactFile], paths: &[ImpactPath]) -> String {
    let max_score = impacted_files
        .iter()
        .map(|file| file.score)
        .max()
        .unwrap_or_default();
    let max_depth = paths
        .iter()
        .map(|path| path.depth)
        .max()
        .unwrap_or_default();

    if impacted_files.len() >= IMPACT_RISK_HIGH_FILE_COUNT
        || max_score >= IMPACT_RISK_HIGH_SCORE
        || max_depth >= IMPACT_RISK_HIGH_DEPTH
    {
        "high".to_string()
    } else if impacted_files.len() >= IMPACT_RISK_MEDIUM_FILE_COUNT
        || max_score >= IMPACT_RISK_MEDIUM_SCORE
        || max_depth >= IMPACT_RISK_MEDIUM_DEPTH
    {
        "medium".to_string()
    } else {
        "low".to_string()
    }
}

fn impact_top_reasons(impacted_files: &[ImpactFile], limit: usize) -> Vec<String> {
    let mut reason_scores = BTreeMap::<String, i32>::new();
    for file in impacted_files {
        for reason in &file.reasons {
            *reason_scores.entry(reason.clone()).or_default() += file.score;
        }
    }

    let mut reasons = reason_scores.into_iter().collect::<Vec<_>>();
    reasons.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    reasons
        .into_iter()
        .take(limit)
        .map(|(reason, _score)| reason)
        .collect()
}

fn impact_breakdown_from_reason_sets<'a>(
    reason_sets: impl IntoIterator<Item = &'a BTreeSet<String>>,
    paths: &[ImpactPath],
    errors: usize,
) -> ImpactBreakdown {
    let mut seed_files = 0;
    let mut symbol_definition_files = 0;
    let mut reference_files = 0;
    let mut call_related_files = 0;
    let mut dependency_related_files = 0;

    for reasons in reason_sets {
        if reasons.iter().any(|reason| reason == "seed_file") {
            seed_files += 1;
        }
        if reasons
            .iter()
            .any(|reason| reason.starts_with("symbol_definition:"))
        {
            symbol_definition_files += 1;
        }
        if reasons
            .iter()
            .any(|reason| reason.starts_with("reference:"))
        {
            reference_files += 1;
        }
        if reasons.iter().any(|reason| {
            reason.starts_with("caller:")
                || reason.starts_with("caller_depth_")
                || reason.starts_with("callee_source:")
                || reason.starts_with("callee_target:")
        }) {
            call_related_files += 1;
        }
        if reasons.iter().any(|reason| {
            reason.starts_with("dependency_source:")
                || reason.starts_with("dependency_target:")
                || reason.starts_with("dependency_importer_depth_")
                || reason.starts_with("type_relation_source:")
        }) {
            dependency_related_files += 1;
        }
    }

    ImpactBreakdown {
        seed_files,
        symbol_definition_files,
        reference_files,
        call_related_files,
        dependency_related_files,
        call_paths: paths.iter().filter(|path| path.kind == "call").count(),
        dependency_paths: paths
            .iter()
            .filter(|path| path.kind == "dependency" || path.kind == "type_relation")
            .count(),
        errors,
    }
}

fn impact_suggested_checks(
    root: &Path,
    risk_level: &str,
    impacted_files: &[ImpactFile],
    paths: &[ImpactPath],
    errors: &[IndexError],
) -> Result<Vec<SuggestedCheck>> {
    let languages = impacted_files
        .iter()
        .filter_map(|file| detect_language(Path::new(&file.file)))
        .map(Language::as_str)
        .collect::<BTreeSet<_>>();
    let mut checks = Vec::new();
    let mut seen_commands = BTreeSet::new();
    let project_config = load_project_config(root)?;
    let configured_commands = project_config
        .as_ref()
        .map(|config| {
            push_configured_impact_checks(
                &config.impact_analysis.test_commands,
                &config.impact_analysis.suggested_checks,
                impacted_files,
                &languages,
                &mut checks,
                &mut seen_commands,
            )
        })
        .unwrap_or_default();

    if configured_commands == 0 {
        push_builtin_impact_command_checks(
            root,
            &languages,
            impacted_files,
            &mut checks,
            &mut seen_commands,
        );
    }

    push_impact_review_checks(&mut checks, risk_level, impacted_files, paths, errors);

    checks.truncate(8);
    Ok(checks)
}

fn push_configured_impact_checks(
    test_commands: &[String],
    configured_checks: &[ConfiguredSuggestedCheck],
    impacted_files: &[ImpactFile],
    languages: &BTreeSet<&'static str>,
    checks: &mut Vec<SuggestedCheck>,
    seen_commands: &mut BTreeSet<String>,
) -> usize {
    let mut pushed = 0;
    for command in test_commands {
        let command = command.trim();
        if command.is_empty() {
            continue;
        }
        if push_command_check(
            checks,
            seen_commands,
            command,
            &format!(
                "Configured by {} impact_analysis.test_commands.",
                project_config_path()
            ),
        ) {
            pushed += 1;
        }
    }

    for check in configured_checks {
        if !configured_check_matches(check, impacted_files, languages) {
            continue;
        }
        let command = check.command.trim();
        if command.is_empty() {
            continue;
        }
        let reason = check
            .reason
            .as_deref()
            .filter(|reason| !reason.trim().is_empty())
            .unwrap_or("Configured project impact analysis check.");
        if push_command_check(checks, seen_commands, command, reason) {
            pushed += 1;
        }
    }
    pushed
}

fn configured_check_matches(
    check: &ConfiguredSuggestedCheck,
    impacted_files: &[ImpactFile],
    languages: &BTreeSet<&'static str>,
) -> bool {
    let matches_language = check.languages.is_empty()
        || check
            .languages
            .iter()
            .map(|language| language.trim().to_ascii_lowercase())
            .any(|language| languages.contains(language.as_str()));
    let matches_file = check.files.is_empty()
        || check
            .files
            .iter()
            .map(|file| file.trim())
            .filter(|file| !file.is_empty())
            .any(|prefix| {
                impacted_files
                    .iter()
                    .any(|file| configured_file_filter_matches(&file.file, prefix))
            });
    matches_language && matches_file
}

fn configured_file_filter_matches(file: &str, filter: &str) -> bool {
    let file = file.replace('\\', "/");
    let filter = filter.trim().replace('\\', "/");
    if filter.is_empty() {
        return false;
    }
    if file == filter {
        return true;
    }
    if let Some(prefix) = filter.strip_suffix('/') {
        return !prefix.is_empty() && file.starts_with(&format!("{prefix}/"));
    }
    file.starts_with(&format!("{filter}/")) || file.starts_with(&format!("{filter}."))
}

fn push_builtin_impact_command_checks(
    root: &Path,
    languages: &BTreeSet<&'static str>,
    impacted_files: &[ImpactFile],
    checks: &mut Vec<SuggestedCheck>,
    seen_commands: &mut BTreeSet<String>,
) {
    let commands = suggested_test_commands_for_root(root);
    for file in impacted_files
        .iter()
        .filter(|file| is_test_source_file(&file.file))
        .take(3)
    {
        for command in &commands {
            let Some(focused_command) = focused_test_command(command, &file.file) else {
                continue;
            };
            let reason = format!(
                "Focused test file {} is impacted; run it before changing behavior.",
                file.file
            );
            push_command_check(checks, seen_commands, &focused_command, &reason);
        }
    }

    for command in &commands {
        let Some(reason) = builtin_impact_command_reason(command, languages) else {
            continue;
        };
        push_command_check(checks, seen_commands, command, reason);
    }
}

fn builtin_impact_command_reason(
    command: &str,
    languages: &BTreeSet<&'static str>,
) -> Option<&'static str> {
    match command {
        "cargo test --locked" if languages.contains("rust") => {
            Some("Rust files are impacted and Cargo.toml is present.")
        }
        "pnpm test" | "yarn test" | "npm test"
            if languages
                .iter()
                .any(|language| matches!(*language, "javascript" | "typescript" | "tsx")) =>
        {
            Some("JavaScript or TypeScript files are impacted and package metadata is present.")
        }
        "pytest" if languages.contains("python") => {
            Some("Python files are impacted and Python test metadata is present.")
        }
        "go test ./..." if languages.contains("go") => {
            Some("Go files are impacted and go.mod is present.")
        }
        "mvn test" | "./gradlew --no-daemon test" | "gradle test" if languages.contains("java") => {
            Some("Java files are impacted and build metadata is present.")
        }
        "dotnet test" if languages.contains("csharp") => {
            Some("C# files are impacted and a .csproj file is present.")
        }
        "bundle exec rspec" if languages.contains("ruby") => {
            Some("Ruby files are impacted and Gemfile is present.")
        }
        "composer test" if languages.contains("php") => {
            Some("PHP files are impacted and composer.json is present.")
        }
        _ => None,
    }
}

fn push_impact_review_checks(
    checks: &mut Vec<SuggestedCheck>,
    risk_level: &str,
    impacted_files: &[ImpactFile],
    paths: &[ImpactPath],
    errors: &[IndexError],
) {
    if matches!(risk_level, "medium" | "high") {
        checks.push(SuggestedCheck {
            kind: "review".to_string(),
            command: None,
            file: None,
            reason: format!(
                "Risk level is {risk_level}; review the ranked impacted files before changing behavior."
            ),
        });
    }

    if !paths.is_empty() {
        checks.push(SuggestedCheck {
            kind: "review".to_string(),
            command: None,
            file: None,
            reason: "Review multi-hop call and dependency paths because the change may propagate beyond direct references.".to_string(),
        });
    }

    if !errors.is_empty() {
        checks.push(SuggestedCheck {
            kind: "review".to_string(),
            command: None,
            file: None,
            reason: "Resolve impact analysis seed or index errors before trusting the report."
                .to_string(),
        });
    }

    if let Some(top_file) = impacted_files.first() {
        checks.push(SuggestedCheck {
            kind: "review".to_string(),
            command: None,
            file: Some(top_file.file.clone()),
            reason: format!(
                "Review the highest-ranked impacted file with score {} and its evidence reasons.",
                top_file.score
            ),
        });
    }
}

fn push_command_check(
    checks: &mut Vec<SuggestedCheck>,
    seen_commands: &mut BTreeSet<String>,
    command: &str,
    reason: &str,
) -> bool {
    if seen_commands.insert(command.to_string()) {
        checks.push(SuggestedCheck {
            kind: "command".to_string(),
            command: Some(command.to_string()),
            file: None,
            reason: reason.to_string(),
        });
        true
    } else {
        false
    }
}

fn focused_test_command(base_command: &str, file: &str) -> Option<String> {
    if !is_test_source_file(file) {
        return None;
    }
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    let file_arg = shell_arg(file);
    match base_command {
        "pnpm test" if is_javascript_like_file(&normalized) => {
            Some(format!("pnpm test -- {file_arg}"))
        }
        "yarn test" if is_javascript_like_file(&normalized) => {
            Some(format!("yarn test {file_arg}"))
        }
        "npm test" if is_javascript_like_file(&normalized) => {
            Some(format!("npm test -- {file_arg}"))
        }
        "pytest" if normalized.ends_with(".py") => Some(format!("pytest {file_arg}")),
        "go test ./..." if normalized.ends_with(".go") => {
            Some(format!("go test {}", go_test_package_arg(file)))
        }
        "cargo test --locked" if normalized.ends_with(".rs") => {
            focused_rust_test_command(base_command, file)
        }
        "mvn test" | "./gradlew --no-daemon test" | "gradle test"
            if normalized.ends_with(".java") =>
        {
            focused_java_test_command(base_command, file)
        }
        "dotnet test" if normalized.ends_with(".cs") => {
            focused_dotnet_test_command(base_command, file)
        }
        "bundle exec rspec" if normalized.ends_with(".rb") => {
            Some(format!("bundle exec rspec {file_arg}"))
        }
        _ => None,
    }
}

fn is_javascript_like_file(file: &str) -> bool {
    matches!(
        Path::new(file)
            .extension()
            .and_then(|extension| extension.to_str()),
        Some("js" | "jsx" | "ts" | "tsx")
    )
}

fn is_test_source_file(file: &str) -> bool {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    normalized.starts_with("test/")
        || normalized.starts_with("tests/")
        || normalized.starts_with("spec/")
        || normalized.starts_with("specs/")
        || normalized.contains("/test/")
        || normalized.contains("/tests/")
        || normalized.contains("/spec/")
        || normalized.contains("/specs/")
        || normalized.contains("/__tests__/")
        || normalized.ends_with("_test.go")
        || normalized.ends_with("_test.py")
        || normalized.ends_with("_test.rb")
        || normalized.ends_with("_test.php")
        || normalized.ends_with("_test.rs")
        || normalized.ends_with("_smoke.sh")
        || normalized.ends_with("-smoke.sh")
        || normalized.ends_with(".smoke.sh")
        || normalized.ends_with("_spec.rb")
        || normalized.ends_with("test.java")
        || normalized.ends_with("test.cs")
        || normalized.ends_with("tests.cs")
        || normalized.ends_with(".test.js")
        || normalized.ends_with(".test.jsx")
        || normalized.ends_with(".test.ts")
        || normalized.ends_with(".test.tsx")
        || normalized.ends_with(".spec.js")
        || normalized.ends_with(".spec.jsx")
        || normalized.ends_with(".spec.ts")
        || normalized.ends_with(".spec.tsx")
}

fn go_test_package_arg(file: &str) -> String {
    let normalized = file.replace('\\', "/");
    let package = if let Some((dir, _)) = normalized.rsplit_once('/') {
        if dir.is_empty() {
            ".".to_string()
        } else {
            format!("./{dir}")
        }
    } else {
        ".".to_string()
    };
    shell_arg(&package)
}

fn focused_rust_test_command(base_command: &str, file: &str) -> Option<String> {
    let normalized = file.replace('\\', "/");
    if let Some(test_file) = normalized.strip_prefix("tests/")
        && let Some(test_target) = test_file.strip_suffix(".rs")
        && !test_target.is_empty()
        && !test_target.contains('/')
    {
        return Some(format!("{base_command} --test {}", shell_arg(test_target)));
    }

    let stem = Path::new(&normalized).file_stem()?.to_string_lossy();
    if stem.is_empty() {
        return None;
    }
    Some(format!("{base_command} {}", shell_arg(stem.as_ref())))
}

fn focused_java_test_command(base_command: &str, file: &str) -> Option<String> {
    let normalized = file.replace('\\', "/");
    let class_name = Path::new(&normalized).file_stem()?.to_string_lossy();
    if class_name.is_empty() {
        return None;
    }

    match base_command {
        "mvn test" => Some(format!(
            "mvn -Dtest={} test",
            shell_arg(class_name.as_ref())
        )),
        "./gradlew --no-daemon test" | "gradle test" => Some(format!(
            "{base_command} --tests {}",
            shell_arg(class_name.as_ref())
        )),
        _ => None,
    }
}

fn focused_dotnet_test_command(base_command: &str, file: &str) -> Option<String> {
    let normalized = file.replace('\\', "/");
    let class_name = Path::new(&normalized).file_stem()?.to_string_lossy();
    if class_name.is_empty() {
        return None;
    }
    Some(format!(
        "{base_command} --filter FullyQualifiedName~{}",
        shell_arg(class_name.as_ref())
    ))
}

fn shell_arg(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
    {
        value.to_string()
    } else {
        format!("'{}'", value.replace('\'', "'\"'\"'"))
    }
}

fn impact_call_paths(
    store: &Store,
    seed_terms: &BTreeSet<String>,
    callers: &mut Vec<CallEdge>,
    impact: &mut BTreeMap<String, (i32, BTreeSet<String>)>,
    depth: usize,
    limit: usize,
) -> Result<Vec<ImpactPath>> {
    let mut paths = Vec::new();
    let mut seen_paths = BTreeSet::new();
    let mut visited_symbols = seed_terms.clone();
    let mut frontier = VecDeque::new();

    for call in callers.iter() {
        push_call_path(&mut paths, &mut seen_paths, call, 1, limit);
        if visited_symbols.insert(call.caller.clone()) {
            frontier.push_back((call.caller.clone(), 1));
        }
    }

    while let Some((symbol, current_depth)) = frontier.pop_front() {
        if current_depth >= depth || paths.len() >= limit {
            continue;
        }
        let next_depth = current_depth + 1;
        let remaining = limit.saturating_sub(paths.len());
        let next_callers = store.callers(&symbol, remaining)?;
        for call in next_callers {
            if paths.len() >= limit {
                break;
            }
            let score = (IMPACT_SCORE_CALLER_DEPTH_BASE
                - ((next_depth as i32 - 1) * IMPACT_SCORE_CALLER_DEPTH_DECAY))
                .max(IMPACT_SCORE_DEPTH_FLOOR);
            add_impact(
                impact,
                &call.file,
                score,
                format!("caller_depth_{next_depth}:{}->{}", call.caller, call.callee),
            );
            push_call_path(&mut paths, &mut seen_paths, &call, next_depth, limit);
            if callers.len() < limit {
                callers.push(call.clone());
            }
            if visited_symbols.insert(call.caller.clone()) {
                frontier.push_back((call.caller.clone(), next_depth));
            }
        }
    }

    Ok(paths)
}

fn push_call_path(
    paths: &mut Vec<ImpactPath>,
    seen_paths: &mut BTreeSet<(String, String, usize, usize)>,
    call: &CallEdge,
    depth: usize,
    limit: usize,
) {
    if paths.len() >= limit
        || !seen_paths.insert((call.callee.clone(), call.caller.clone(), depth, call.line))
    {
        return;
    }
    paths.push(ImpactPath {
        kind: "call".to_string(),
        depth,
        from: call.callee.clone(),
        to: call.caller.clone(),
        file: call.file.clone(),
        via: format!("{}->{}", call.caller, call.callee),
        line: call.line,
    });
}

fn push_downstream_call_path(
    paths: &mut Vec<ImpactPath>,
    seen_paths: &mut BTreeSet<(String, String, usize, usize)>,
    call: &CallEdge,
    depth: usize,
    limit: usize,
) {
    let Some(callee_file) = &call.callee_file else {
        return;
    };
    if paths.len() >= limit
        || !seen_paths.insert((call.caller.clone(), call.callee.clone(), depth, call.line))
    {
        return;
    }
    paths.push(ImpactPath {
        kind: "call".to_string(),
        depth,
        from: call.caller.clone(),
        to: call.callee.clone(),
        file: callee_file.clone(),
        via: format!("{}->{}", call.caller, call.callee),
        line: call.line,
    });
}

fn impact_dependency_paths(
    store: &Store,
    seed_files: &[String],
    dependencies: &mut Vec<Dependency>,
    impact: &mut BTreeMap<String, (i32, BTreeSet<String>)>,
    depth: usize,
    limit: usize,
) -> Result<Vec<ImpactPath>> {
    let seed_file_set = seed_files.iter().cloned().collect::<BTreeSet<_>>();
    let mut paths = Vec::new();
    let mut seen_paths = BTreeSet::new();
    let mut visited_files = seed_file_set.clone();
    let mut frontier = VecDeque::new();

    for dependency in dependencies.iter() {
        let Some(resolved_file) = &dependency.resolved_file else {
            continue;
        };
        if !seed_file_set.contains(resolved_file) {
            continue;
        }
        push_dependency_path(&mut paths, &mut seen_paths, dependency, 1, limit);
        if visited_files.insert(dependency.source_file.clone()) {
            frontier.push_back((dependency.source_file.clone(), 1));
        }
    }

    while let Some((file, current_depth)) = frontier.pop_front() {
        if current_depth >= depth || paths.len() >= limit {
            continue;
        }
        let next_depth = current_depth + 1;
        let remaining = limit.saturating_sub(paths.len());
        let next_dependencies =
            store.dependency_importers_for_files(std::slice::from_ref(&file), remaining)?;
        for dependency in next_dependencies {
            if paths.len() >= limit {
                break;
            }
            let score = (IMPACT_SCORE_DEPENDENCY_DEPTH_BASE
                - ((next_depth as i32 - 1) * IMPACT_SCORE_DEPENDENCY_DEPTH_DECAY))
                .max(IMPACT_SCORE_DEPTH_FLOOR);
            add_impact(
                impact,
                &dependency.source_file,
                score,
                format!(
                    "dependency_importer_depth_{next_depth}:{}",
                    dependency.target
                ),
            );
            push_dependency_path(&mut paths, &mut seen_paths, &dependency, next_depth, limit);
            if dependencies.len() < limit {
                dependencies.push(dependency.clone());
            }
            if visited_files.insert(dependency.source_file.clone()) {
                frontier.push_back((dependency.source_file.clone(), next_depth));
            }
        }
    }

    Ok(paths)
}

fn impact_type_relation_target_file(symbols: &[Symbol], dependency: &Dependency) -> Option<String> {
    symbols
        .iter()
        .find(|symbol| symbol_matches_type_relation_target(symbol, &dependency.target))
        .map(|symbol| symbol.file.clone())
}

fn push_type_relation_path(
    paths: &mut Vec<ImpactPath>,
    seen_paths: &mut BTreeSet<(String, String, usize, usize)>,
    dependency: &Dependency,
    target_file: &str,
    relation: &str,
    depth: usize,
    limit: usize,
) {
    if paths.len() >= limit
        || !seen_paths.insert((
            target_file.to_string(),
            dependency.source_file.clone(),
            depth,
            dependency.line,
        ))
    {
        return;
    }
    paths.push(ImpactPath {
        kind: "type_relation".to_string(),
        depth,
        from: target_file.to_string(),
        to: dependency.source_file.clone(),
        file: dependency.source_file.clone(),
        via: format!("{relation}:{}", dependency.target),
        line: dependency.line,
    });
}

fn push_dependency_path(
    paths: &mut Vec<ImpactPath>,
    seen_paths: &mut BTreeSet<(String, String, usize, usize)>,
    dependency: &Dependency,
    depth: usize,
    limit: usize,
) {
    let Some(resolved_file) = &dependency.resolved_file else {
        return;
    };
    if paths.len() >= limit
        || !seen_paths.insert((
            resolved_file.clone(),
            dependency.source_file.clone(),
            depth,
            dependency.line,
        ))
    {
        return;
    }
    paths.push(ImpactPath {
        kind: "dependency".to_string(),
        depth,
        from: resolved_file.clone(),
        to: dependency.source_file.clone(),
        file: dependency.source_file.clone(),
        via: dependency.target.clone(),
        line: dependency.line,
    });
}

fn add_impact(
    impact: &mut BTreeMap<String, (i32, BTreeSet<String>)>,
    file: &str,
    score: i32,
    reason: impl Into<String>,
) {
    let entry = impact
        .entry(file.to_string())
        .or_insert_with(|| (0, BTreeSet::new()));
    entry.0 += score;
    entry.1.insert(reason.into());
}

fn dedup_symbols(symbols: &mut Vec<Symbol>) {
    let mut seen = BTreeSet::new();
    symbols.retain(|symbol| {
        seen.insert((
            symbol.file.clone(),
            symbol.qualified_name.clone(),
            symbol.start_line,
            symbol.end_line,
        ))
    });
}

fn dedup_references(references: &mut Vec<ReferenceMatch>) {
    let mut seen = BTreeSet::new();
    references.retain(|reference| {
        seen.insert((
            reference.file.clone(),
            reference.line,
            reference.column,
            reference.context.clone(),
        ))
    });
}

fn dedup_calls(calls: &mut Vec<CallEdge>) {
    let mut seen = BTreeSet::new();
    calls.retain(|call| {
        seen.insert((
            call.file.clone(),
            call.caller.clone(),
            call.callee.clone(),
            call.callee_file.clone(),
            call.line,
            call.column,
        ))
    });
}

#[derive(Debug)]
struct ReferenceCandidate {
    reference: ReferenceMatch,
    score: i32,
}

#[derive(Debug, Clone)]
struct ContextCandidateRange {
    start_line: usize,
    end_line: usize,
    reason: String,
    source: String,
    score: i32,
}

#[derive(Debug)]
struct ContextFileCandidate {
    seed_order: Option<usize>,
    file: String,
    ranges: Vec<ContextCandidateRange>,
    max_score: i32,
    source_mix_score: i32,
    recent_edit_score: i32,
    total_score: i32,
}

#[derive(Debug, Clone, Copy)]
struct ContextScoringPolicy {
    prefer_low_value_files: bool,
    prefer_agent_first_read_source_files: bool,
    prefer_indexing_pipeline_source_files: bool,
    prefer_data_persistence_source_files: bool,
    prefer_semantic_context_source_files: bool,
    prefer_semantic_context_orchestration_files: bool,
    prefer_dependency_graph_source_files: bool,
    prefer_project_overview_source_files: bool,
    prefer_symbol_search_source_files: bool,
    prefer_reference_search_source_files: bool,
    prefer_call_graph_traversal_source_files: bool,
    prefer_file_parsing_source_files: bool,
    prefer_binding_validation_source_files: bool,
    prefer_import_resolution_source_files: bool,
}

fn seed_file_ranges(
    root: &Path,
    file: &str,
    symbols: &[Symbol],
    task_keywords: &[String],
    seed_symbols: &[String],
    task_path_locations: &[TaskPathLocation],
) -> Vec<ContextCandidateRange> {
    let path = root.join(file);
    let source = fs::read_to_string(path).unwrap_or_default();
    let lines = source.lines().collect::<Vec<_>>();
    let line_count = lines.len().max(1);
    let mut ranges = Vec::new();

    for location in task_path_locations {
        let requested_start = location.start_line.clamp(1, line_count);
        let requested_end = location.end_line.clamp(requested_start, line_count);
        ranges.push(ContextCandidateRange {
            start_line: requested_start,
            end_line: requested_end,
            reason: if requested_start == requested_end {
                format!("Task requested {file} at line {requested_start}")
            } else {
                format!("Task requested {file} lines {requested_start}-{requested_end}")
            },
            source: "task_location".to_string(),
            score: CONTEXT_SCORE_TASK_LOCATION,
        });

        let context_start = requested_start.saturating_sub(3).max(1);
        let context_end = requested_end.saturating_add(3).min(line_count);
        if context_start == requested_start && context_end == requested_end {
            continue;
        }
        ranges.push(ContextCandidateRange {
            start_line: context_start,
            end_line: context_end,
            reason: format!("Context around task-requested lines in {file}"),
            source: "task_location".to_string(),
            score: CONTEXT_SCORE_TASK_LOCATION_CONTEXT,
        });
    }

    if let Some(end_line) = header_range_end(&lines) {
        let matched_keywords = auto_seed_file_matched_keywords(root, file, None, task_keywords);
        ranges.push(ContextCandidateRange {
            start_line: 1,
            end_line,
            reason: seed_range_reason(
                &format!("Seed file header and imports for task: {file}"),
                &matched_keywords,
                &seed_request_lifecycle_reasons(file, None, task_keywords),
            ),
            source: "seed_file".to_string(),
            score: CONTEXT_SCORE_SEED_HEADER + seed_file_task_boost(&matched_keywords),
        });
    }

    let mut primary_symbols = symbols
        .iter()
        .filter(|symbol| symbol.file == file && is_primary_seed_symbol(symbol))
        .collect::<Vec<_>>();
    primary_symbols.sort_by(|left, right| {
        context_symbol_matches_seed(right, seed_symbols)
            .cmp(&context_symbol_matches_seed(left, seed_symbols))
            .then_with(|| {
                seed_primary_symbol_score(right, file, task_keywords)
                    .cmp(&seed_primary_symbol_score(left, file, task_keywords))
            })
            .then_with(|| left.start_line.cmp(&right.start_line))
            .then_with(|| left.end_line.cmp(&right.end_line))
    });
    let mut primary_symbols = primary_symbols.into_iter().take(12).collect::<Vec<_>>();
    primary_symbols.sort_by_key(|symbol| (symbol.start_line, symbol.end_line));

    for symbol in primary_symbols {
        let matched_keywords =
            auto_seed_matched_keywords(file, Some(&symbol.qualified_name), task_keywords);
        ranges.push(ContextCandidateRange {
            start_line: symbol.start_line.saturating_sub(2).max(1),
            end_line: (capped_symbol_end_line(symbol) + 2).min(line_count),
            reason: seed_range_reason(
                &format!("Seed file defines symbol {}", symbol.qualified_name),
                &matched_keywords,
                &seed_request_lifecycle_reasons(file, Some(&symbol.qualified_name), task_keywords),
            ),
            source: "seed_file".to_string(),
            score: CONTEXT_SCORE_SEED_FILE
                + seed_symbol_task_boost(symbol, task_keywords)
                + if context_symbol_matches_seed(symbol, seed_symbols) {
                    CONTEXT_SCORE_TASK_MATCH_BOOST
                } else {
                    0
                },
        });
    }

    if ranges.is_empty() {
        let matched_keywords = auto_seed_file_matched_keywords(root, file, None, task_keywords);
        ranges.push(ContextCandidateRange {
            start_line: 1,
            end_line: line_count.min(80),
            reason: seed_range_reason(
                &format!("Seed file requested for task: {file}"),
                &matched_keywords,
                &seed_request_lifecycle_reasons(file, None, task_keywords),
            ),
            source: "seed_file".to_string(),
            score: CONTEXT_SCORE_SEED_FILE + seed_file_task_boost(&matched_keywords),
        });
    }

    ranges
}

fn context_symbol_matches_seed(symbol: &Symbol, seed_symbols: &[String]) -> bool {
    seed_symbols
        .iter()
        .any(|seed| backend_symbol_name_matches(seed, &symbol.name, &symbol.qualified_name))
}

fn seed_primary_symbol_score(symbol: &Symbol, file: &str, task_keywords: &[String]) -> i32 {
    auto_seed_task_match_score(file, Some(&symbol.qualified_name), task_keywords)
        + auto_seed_task_focus_boost(file, Some(&symbol.qualified_name), task_keywords, false)
        + auto_seed_symbol_kind_priority(&symbol.kind)
}

fn seed_range_reason(base: &str, matched_keywords: &[String], extra_reasons: &[String]) -> String {
    let mut reasons = vec![base.to_string()];
    if !matched_keywords.is_empty() {
        reasons.push(format!(
            "matched task keywords: {}",
            matched_keywords.join(", ")
        ));
    }
    reasons.extend(extra_reasons.iter().cloned());
    reasons.join("; ")
}

fn seed_request_lifecycle_reasons(
    file: &str,
    symbol: Option<&str>,
    task_keywords: &[String],
) -> Vec<String> {
    if !auto_seed_request_lifecycle_task(task_keywords) {
        return Vec::new();
    }

    let mut reasons = Vec::new();
    if auto_seed_request_lifecycle_file_matches(file) {
        reasons
            .push("request lifecycle task matched framework handler or app seed file".to_string());
    }
    if symbol
        .map(auto_seed_request_lifecycle_symbol_matches)
        .unwrap_or(false)
    {
        reasons.push("request lifecycle task matched request/response dispatch symbol".to_string());
    }
    reasons
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
    source: &str,
    score: i32,
) {
    let ranges = ranges_by_file.entry(file).or_default();
    if let Some(existing) = ranges
        .iter_mut()
        .find(|range| range.start_line == start_line && range.end_line == end_line)
    {
        if score > existing.score {
            existing.source = source.to_string();
        }
        existing.score = existing.score.max(score);
        append_context_range_reason(&mut existing.reason, &reason);
        return;
    }
    ranges.push(ContextCandidateRange {
        start_line,
        end_line,
        reason,
        source: source.to_string(),
        score,
    });
}

fn append_context_range_reason(existing: &mut String, reason: &str) {
    if existing.contains(reason) {
        return;
    }

    let additional_len = 2 + reason.len();
    if existing.len().saturating_add(additional_len) <= CONTEXT_RANGE_REASON_MAX_BYTES {
        existing.push_str("; ");
        existing.push_str(reason);
    } else if !existing.contains(CONTEXT_RANGE_REASON_OMITTED) {
        existing.push_str("; ");
        existing.push_str(CONTEXT_RANGE_REASON_OMITTED);
    }
}

fn compare_context_file_candidates(
    left: &ContextFileCandidate,
    right: &ContextFileCandidate,
) -> Ordering {
    match (left.seed_order, right.seed_order) {
        (Some(left_order), Some(right_order)) => left_order.cmp(&right_order),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => right
            .max_score
            .cmp(&left.max_score)
            .then_with(|| right.source_mix_score.cmp(&left.source_mix_score))
            .then_with(|| right.recent_edit_score.cmp(&left.recent_edit_score))
            .then_with(|| right.total_score.cmp(&left.total_score))
            .then_with(|| left.file.cmp(&right.file)),
    }
    .then_with(|| {
        right
            .max_score
            .cmp(&left.max_score)
            .then_with(|| right.source_mix_score.cmp(&left.source_mix_score))
            .then_with(|| right.recent_edit_score.cmp(&left.recent_edit_score))
            .then_with(|| right.total_score.cmp(&left.total_score))
            .then_with(|| left.file.cmp(&right.file))
    })
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

fn uncovered_segments(
    start_line: usize,
    end_line: usize,
    selected_ranges: &[(usize, usize)],
) -> Vec<(usize, usize)> {
    if start_line > end_line {
        return Vec::new();
    }

    let mut cursor = start_line;
    let mut segments = Vec::new();
    let mut overlaps = selected_ranges
        .iter()
        .copied()
        .filter(|(selected_start, selected_end)| {
            *selected_end >= start_line && *selected_start <= end_line
        })
        .collect::<Vec<_>>();
    overlaps.sort_by_key(|(selected_start, selected_end)| (*selected_start, *selected_end));

    for (selected_start, selected_end) in overlaps {
        if selected_end < cursor {
            continue;
        }
        if selected_start > cursor {
            segments.push((cursor, (selected_start - 1).min(end_line)));
        }
        cursor = cursor.max(selected_end.saturating_add(1));
        if cursor > end_line {
            return segments;
        }
    }

    segments.push((cursor, end_line));
    segments
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

fn context_score_for_file(file: &str, score: i32, policy: &ContextScoringPolicy) -> i32 {
    let score = if is_low_value_reference_file(file) && policy.prefer_low_value_files {
        score.saturating_add(CONTEXT_SCORE_LOW_VALUE_FILE_TEST_BOOST)
    } else if is_low_value_reference_file(file) {
        score.saturating_sub(CONTEXT_SCORE_LOW_VALUE_FILE_PENALTY)
    } else {
        score
    };

    if policy.prefer_agent_first_read_source_files {
        context_agent_first_read_source_score(file, score)
    } else if policy.prefer_indexing_pipeline_source_files {
        context_indexing_pipeline_source_score(file, score)
    } else if policy.prefer_data_persistence_source_files {
        context_data_persistence_source_score(file, score)
    } else if policy.prefer_semantic_context_source_files {
        context_semantic_context_source_score(
            file,
            score,
            policy.prefer_semantic_context_orchestration_files,
        )
    } else if policy.prefer_dependency_graph_source_files {
        context_dependency_graph_source_score(file, score)
    } else if policy.prefer_project_overview_source_files {
        context_project_overview_source_score(file, score)
    } else if policy.prefer_symbol_search_source_files {
        context_symbol_search_source_score(file, score)
    } else if policy.prefer_reference_search_source_files {
        context_reference_search_source_score(file, score)
    } else if policy.prefer_call_graph_traversal_source_files {
        context_call_graph_traversal_source_score(file, score)
    } else if policy.prefer_file_parsing_source_files {
        context_file_parsing_source_score(file, score)
    } else if policy.prefer_binding_validation_source_files {
        context_binding_validation_source_score(file, score)
    } else if policy.prefer_import_resolution_source_files {
        context_import_resolution_source_score(file, score)
    } else {
        score
    }
}

fn context_agent_first_read_source_score(file: &str, score: i32) -> i32 {
    if context_agent_first_read_support_file(file) {
        score.saturating_sub(1000)
    } else {
        let normalized = file.replace('\\', "/").to_ascii_lowercase();
        if (normalized.starts_with("src/") || normalized.contains("/src/"))
            && auto_seed_agent_first_read_core_file_matches(file)
        {
            score.saturating_add(120)
        } else {
            score
        }
    }
}

fn context_agent_first_read_support_file(file: &str) -> bool {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    normalized == "scripts"
        || normalized.starts_with("scripts/")
        || normalized == "docs"
        || normalized.starts_with("docs/")
        || is_low_value_reference_file(file)
}

fn context_indexing_pipeline_source_score(file: &str, score: i32) -> i32 {
    let priority = auto_seed_indexing_pipeline_file_priority(file);
    if context_agent_first_read_support_file(file) {
        score.saturating_sub(900)
    } else if priority > 0 {
        score.saturating_add(priority)
    } else {
        score
    }
}

fn context_data_persistence_source_score(file: &str, score: i32) -> i32 {
    let priority = auto_seed_data_persistence_file_priority(file);
    if context_agent_first_read_support_file(file) {
        score.saturating_sub(900)
    } else if priority > 0 {
        score.saturating_add(priority)
    } else {
        score
    }
}

fn context_semantic_context_source_score(
    file: &str,
    score: i32,
    prefer_orchestration: bool,
) -> i32 {
    let priority = auto_seed_semantic_context_file_priority(file, prefer_orchestration);
    if context_agent_first_read_support_file(file) {
        score.saturating_sub(900)
    } else if priority > 0 {
        score.saturating_add(priority)
    } else {
        score
    }
}

fn context_dependency_graph_source_score(file: &str, score: i32) -> i32 {
    let priority = auto_seed_dependency_graph_file_priority(file);
    if context_agent_first_read_support_file(file) {
        score.saturating_sub(900)
    } else if priority > 0 {
        score.saturating_add(priority)
    } else {
        score
    }
}

fn context_project_overview_source_score(file: &str, score: i32) -> i32 {
    context_priority_source_score(file, score, auto_seed_project_overview_file_priority(file))
}

fn context_symbol_search_source_score(file: &str, score: i32) -> i32 {
    context_priority_source_score(file, score, auto_seed_symbol_search_file_priority(file))
}

fn context_reference_search_source_score(file: &str, score: i32) -> i32 {
    context_priority_source_score(file, score, auto_seed_tool_analysis_file_priority(file))
}

fn context_call_graph_traversal_source_score(file: &str, score: i32) -> i32 {
    context_priority_source_score(file, score, auto_seed_tool_analysis_file_priority(file))
}

fn context_file_parsing_source_score(file: &str, score: i32) -> i32 {
    context_priority_source_score(file, score, auto_seed_file_parsing_file_priority(file))
}

fn context_binding_validation_source_score(file: &str, score: i32) -> i32 {
    context_priority_source_score(
        file,
        score,
        auto_seed_binding_validation_file_priority_for_task(file, &[]),
    )
}

fn context_import_resolution_source_score(file: &str, score: i32) -> i32 {
    context_priority_source_score(file, score, auto_seed_import_resolution_file_priority(file))
}

fn context_priority_source_score(file: &str, score: i32, priority: i32) -> i32 {
    if context_agent_first_read_support_file(file) {
        score.saturating_sub(900)
    } else if priority > 0 {
        score.saturating_add(priority)
    } else {
        score
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

fn seed_file_task_boost(matched_keywords: &[String]) -> i32 {
    if matched_keywords.is_empty() {
        0
    } else {
        CONTEXT_SCORE_SEED_SYMBOL_TASK_MATCH_BOOST
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

fn semantic_chunk_task_boost(chunk: &SemanticChunk, keywords: &[String]) -> i32 {
    task_match_boost(
        keywords,
        [chunk.file.as_str(), chunk.text.as_str()].into_iter(),
    )
}

fn semantic_chunk_density_boost(chunk: &SemanticChunk) -> i32 {
    if chunk.token_estimate <= 120 { 5 } else { 0 }
}

fn semantic_vector_score_boost(score: f64) -> i32 {
    if !score.is_finite() {
        return 0;
    }
    (score.clamp(0.0, 1.0) * 25.0).round() as i32
}

fn semantic_ranking_terms(task_keywords: &[String], seed_symbols: &[String]) -> Vec<String> {
    let mut terms = task_keywords.to_vec();
    for symbol in seed_symbols {
        terms.extend(
            symbol
                .split(|ch: char| !ch.is_ascii_alphanumeric())
                .map(str::to_ascii_lowercase)
                .filter(|term| term.len() >= 3),
        );
    }
    terms.sort();
    terms.dedup();
    terms.truncate(24);
    terms
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

fn context_prefers_low_value_files(task_keywords: &[String], seed_files: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "test"
                | "tests"
                | "testing"
                | "spec"
                | "specs"
                | "fixture"
                | "fixtures"
                | "coverage"
                | "regression"
                | "unit"
                | "integration"
                | "e2e"
        )
    }) || seed_files
        .iter()
        .any(|file| is_low_value_reference_file(file))
}

#[derive(Debug)]
struct AutoContextSeedSelection {
    strategy: String,
    files: Vec<String>,
    seeds: Vec<ContextSeed>,
    task_path_locations: TaskPathLocations,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TaskPathLocation {
    start_line: usize,
    end_line: usize,
}

type TaskPathLocations = BTreeMap<String, Vec<TaskPathLocation>>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskPathReference {
    file: String,
    location: Option<TaskPathLocation>,
}

#[derive(Debug, Clone)]
struct AutoSeedCandidate {
    file: String,
    role: String,
    source: String,
    score: i32,
    matched_keywords: Vec<String>,
    matched_symbols: Vec<String>,
}

fn auto_context_seed_files(
    store: &Store,
    root: &Path,
    task: &str,
    task_keywords: &[String],
) -> Result<AutoContextSeedSelection> {
    let indexed_files = store.indexed_files()?;
    let indexed_file_set = indexed_files.iter().cloned().collect::<BTreeSet<_>>();
    let task_path_references = auto_seed_task_path_references(root, task);
    let mut task_path_files = Vec::new();
    let mut task_path_locations = BTreeMap::<String, Vec<TaskPathLocation>>::new();
    for reference in &task_path_references {
        if !indexed_file_set.contains(&reference.file) {
            continue;
        }

        if !task_path_files.contains(&reference.file) {
            if task_path_files.len() >= 3 {
                continue;
            }
            task_path_files.push(reference.file.clone());
        }
        if let Some(location) = reference.location {
            let locations = task_path_locations
                .entry(reference.file.clone())
                .or_default();
            if !locations.contains(&location) {
                locations.push(location);
            }
        }
    }
    if !task_path_files.is_empty() {
        let seeds = task_path_files
            .iter()
            .map(|file| ContextSeed {
                kind: "file".to_string(),
                value: file.clone(),
                source: "task_path".to_string(),
                start_line: task_path_locations
                    .get(file)
                    .and_then(|locations| locations.first())
                    .map(|location| location.start_line),
                end_line: task_path_locations
                    .get(file)
                    .and_then(|locations| locations.first())
                    .map(|location| location.end_line),
                locations: context_seed_locations(task_path_locations.get(file).map(Vec::as_slice)),
                role: Some(auto_seed_file_role(file).to_string()),
                matched_keywords: Vec::new(),
                matched_symbols: Vec::new(),
            })
            .collect::<Vec<_>>();
        return Ok(AutoContextSeedSelection {
            strategy: "auto_task_path".to_string(),
            files: task_path_files,
            seeds,
            task_path_locations,
        });
    }
    let mut seen_unindexed_task_paths = BTreeSet::new();
    let unindexed_task_path_files = task_path_references
        .iter()
        .filter(|reference| !indexed_file_set.contains(&reference.file))
        .filter(|reference| auto_seed_task_path_exists_in_project(root, &reference.file))
        .filter(|reference| seen_unindexed_task_paths.insert(reference.file.clone()))
        .map(|reference| reference.file.clone())
        .take(3)
        .collect::<Vec<_>>();
    if !unindexed_task_path_files.is_empty() {
        let seeds = unindexed_task_path_files
            .iter()
            .map(|file| ContextSeed {
                kind: "file".to_string(),
                value: file.clone(),
                source: "task_path_unindexed".to_string(),
                start_line: task_path_references
                    .iter()
                    .find(|reference| reference.file == *file)
                    .and_then(|reference| reference.location)
                    .map(|location| location.start_line),
                end_line: task_path_references
                    .iter()
                    .find(|reference| reference.file == *file)
                    .and_then(|reference| reference.location)
                    .map(|location| location.end_line),
                locations: context_seed_locations_from_references(&task_path_references, file),
                role: Some(auto_seed_file_role(file).to_string()),
                matched_keywords: Vec::new(),
                matched_symbols: Vec::new(),
            })
            .collect::<Vec<_>>();
        return Ok(AutoContextSeedSelection {
            strategy: "auto_task_path_unindexed".to_string(),
            files: Vec::new(),
            seeds,
            task_path_locations: BTreeMap::new(),
        });
    }

    let overview = store.overview(root)?;
    let task_symbol_matches = auto_seed_task_symbol_matches(store, task_keywords)?;
    let mut candidates = BTreeMap::<String, AutoSeedCandidate>::new();

    for entrypoint in overview.entrypoints.iter().filter(|entrypoint| {
        auto_seed_role_allowed(&entrypoint.role, task_keywords)
            && auto_seed_role_allowed(auto_seed_file_role(&entrypoint.file), task_keywords)
    }) {
        let matched_keywords = auto_seed_matched_keywords(
            &entrypoint.file,
            entrypoint.symbol.as_deref(),
            task_keywords,
        );
        let score = entrypoint.score as i32
            + auto_seed_task_match_score(
                &entrypoint.file,
                entrypoint.symbol.as_deref(),
                task_keywords,
            )
            + auto_seed_task_focus_boost(
                &entrypoint.file,
                entrypoint.symbol.as_deref(),
                task_keywords,
                true,
            );
        upsert_auto_seed_candidate(
            &mut candidates,
            AutoSeedCandidate {
                file: entrypoint.file.clone(),
                role: entrypoint.role.clone(),
                source: "overview_entrypoint".to_string(),
                score,
                matched_keywords,
                matched_symbols: entrypoint.symbol.iter().cloned().collect(),
            },
        );
    }

    for file in indexed_files
        .iter()
        .filter(|file| auto_seed_role_allowed(auto_seed_file_role(file), task_keywords))
    {
        let mut symbols = store.symbols_for_files(std::slice::from_ref(file), 12)?;
        if let Some(task_symbols) = task_symbol_matches.get(file) {
            symbols.extend(task_symbols.iter().cloned());
            dedup_symbols(&mut symbols);
        }
        let symbol_or_path_score = symbols
            .iter()
            .map(|symbol| {
                auto_seed_task_match_score(file, Some(&symbol.qualified_name), task_keywords)
                    + auto_seed_task_focus_boost(
                        file,
                        Some(&symbol.qualified_name),
                        task_keywords,
                        false,
                    )
            })
            .max()
            .unwrap_or_else(|| {
                auto_seed_task_match_score(file, None, task_keywords)
                    + auto_seed_task_focus_boost(file, None, task_keywords, false)
            });
        let text_match = auto_seed_file_text_match(root, file, task_keywords);
        let task_score = symbol_or_path_score.max(text_match.score);
        if task_score == 0 {
            continue;
        }
        let matched_keywords = symbols
            .iter()
            .flat_map(|symbol| {
                auto_seed_matched_keywords(file, Some(&symbol.qualified_name), task_keywords)
            })
            .chain(auto_seed_matched_keywords(file, None, task_keywords))
            .chain(text_match.matched_keywords)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let matched_symbols = auto_seed_matched_symbols(&symbols, task_keywords);
        upsert_auto_seed_candidate(
            &mut candidates,
            AutoSeedCandidate {
                file: file.clone(),
                role: auto_seed_file_role(file).to_string(),
                source: "task_match".to_string(),
                score: 60 + task_score,
                matched_keywords,
                matched_symbols,
            },
        );
    }

    for file in indexed_files
        .iter()
        .filter(|file| auto_seed_role_allowed(auto_seed_file_role(file), task_keywords))
    {
        let priority = auto_seed_http_operation_file_priority(file, task_keywords);
        if priority < 3 {
            continue;
        }
        upsert_auto_seed_candidate(
            &mut candidates,
            AutoSeedCandidate {
                file: file.clone(),
                role: auto_seed_file_role(file).to_string(),
                source: "task_match".to_string(),
                score: 90 + priority,
                matched_keywords: auto_seed_matched_keywords(file, None, task_keywords),
                matched_symbols: Vec::new(),
            },
        );
    }

    if auto_seed_route_dispatch_task(task_keywords) {
        for file in indexed_files
            .iter()
            .filter(|file| auto_seed_role_allowed(auto_seed_file_role(file), task_keywords))
        {
            let priority = auto_seed_route_dispatch_file_priority(file, task_keywords);
            if priority < 3 {
                continue;
            }
            upsert_auto_seed_candidate(
                &mut candidates,
                AutoSeedCandidate {
                    file: file.clone(),
                    role: auto_seed_file_role(file).to_string(),
                    source: "task_match".to_string(),
                    score: 90 + priority,
                    matched_keywords: auto_seed_matched_keywords(file, None, task_keywords),
                    matched_symbols: Vec::new(),
                },
            );
        }
    }

    if auto_seed_agent_first_read_task(task_keywords)
        && !auto_seed_agent_first_read_evidence_task(task_keywords)
    {
        for file in indexed_files
            .iter()
            .filter(|file| auto_seed_role_allowed(auto_seed_file_role(file), task_keywords))
        {
            let priority = auto_seed_agent_first_read_file_priority(file, task_keywords);
            if priority < 70 {
                continue;
            }
            upsert_auto_seed_candidate(
                &mut candidates,
                AutoSeedCandidate {
                    file: file.clone(),
                    role: auto_seed_file_role(file).to_string(),
                    source: "task_match".to_string(),
                    score: 90 + priority,
                    matched_keywords: auto_seed_matched_keywords(file, None, task_keywords),
                    matched_symbols: Vec::new(),
                },
            );
        }
    }

    let mut candidates = candidates.into_values().collect::<Vec<_>>();
    let route_miss_task = auto_seed_route_miss_handling_task(task_keywords);
    let websocket_task = auto_seed_websocket_connection_task(task_keywords);
    let request_body_parsing_task = auto_seed_request_body_parsing_task(task_keywords);
    let response_headers_task = auto_seed_response_headers_task(task_keywords);
    let response_cookies_task = auto_seed_response_cookies_task(task_keywords);
    let response_redirect_task = auto_seed_response_redirect_task(task_keywords);
    let request_lifecycle_task = auto_seed_request_lifecycle_task(task_keywords);
    let middleware_task = auto_seed_middleware_task(task_keywords);
    let route_dispatch_task = auto_seed_route_dispatch_task(task_keywords);
    let agent_first_read_task = auto_seed_agent_first_read_task(task_keywords);
    let indexing_pipeline_task = auto_seed_indexing_pipeline_task(task_keywords);
    let data_persistence_task = auto_seed_data_persistence_task(task_keywords);
    let semantic_context_task = auto_seed_semantic_context_task(task_keywords);
    let semantic_context_prefers_orchestration =
        auto_seed_semantic_context_prefers_orchestration(task_keywords);
    let dependency_graph_task = auto_seed_dependency_graph_task(task_keywords);
    let project_overview_task = auto_seed_project_overview_task(task_keywords);
    let symbol_search_task = auto_seed_symbol_search_task(task_keywords);
    let reference_search_task = auto_seed_reference_search_task(task_keywords);
    let call_graph_traversal_task = auto_seed_call_graph_traversal_task(task_keywords);
    let file_parsing_task = auto_seed_file_parsing_task(task_keywords);
    let binding_validation_task = auto_seed_binding_validation_task(task_keywords);
    let import_resolution_task = auto_seed_import_resolution_task(task_keywords);
    let startup_entrypoint_task = auto_seed_startup_entrypoint_task(task_keywords);
    let startup_flow_task = auto_seed_startup_flow_task(task_keywords);
    candidates.sort_by(|left, right| {
        if agent_first_read_task {
            auto_seed_agent_first_read_file_priority(&right.file, task_keywords)
                .cmp(&auto_seed_agent_first_read_file_priority(
                    &left.file,
                    task_keywords,
                ))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if indexing_pipeline_task {
            auto_seed_indexing_pipeline_file_priority(&right.file)
                .cmp(&auto_seed_indexing_pipeline_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if data_persistence_task {
            auto_seed_data_persistence_file_priority(&right.file)
                .cmp(&auto_seed_data_persistence_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if semantic_context_task {
            auto_seed_semantic_context_file_priority(
                &right.file,
                semantic_context_prefers_orchestration,
            )
            .cmp(&auto_seed_semantic_context_file_priority(
                &left.file,
                semantic_context_prefers_orchestration,
            ))
            .then_with(|| right.score.cmp(&left.score))
            .then_with(|| left.file.cmp(&right.file))
        } else if dependency_graph_task {
            auto_seed_dependency_graph_file_priority(&right.file)
                .cmp(&auto_seed_dependency_graph_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if project_overview_task {
            auto_seed_project_overview_file_priority(&right.file)
                .cmp(&auto_seed_project_overview_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if symbol_search_task {
            auto_seed_symbol_search_file_priority(&right.file)
                .cmp(&auto_seed_symbol_search_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if reference_search_task || call_graph_traversal_task {
            auto_seed_tool_analysis_file_priority(&right.file)
                .cmp(&auto_seed_tool_analysis_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if file_parsing_task {
            auto_seed_file_parsing_file_priority(&right.file)
                .cmp(&auto_seed_file_parsing_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if binding_validation_task {
            auto_seed_binding_validation_file_priority_for_task(&right.file, task_keywords)
                .cmp(&auto_seed_binding_validation_file_priority_for_task(
                    &left.file,
                    task_keywords,
                ))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if import_resolution_task {
            auto_seed_import_resolution_file_priority(&right.file)
                .cmp(&auto_seed_import_resolution_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if startup_entrypoint_task {
            auto_seed_startup_entrypoint_file_priority(&right.file, task_keywords)
                .cmp(&auto_seed_startup_entrypoint_file_priority(
                    &left.file,
                    task_keywords,
                ))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if startup_flow_task {
            auto_seed_startup_flow_file_priority(&right.file)
                .cmp(&auto_seed_startup_flow_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if route_miss_task {
            auto_seed_route_miss_file_priority(&right.file)
                .cmp(&auto_seed_route_miss_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if websocket_task {
            auto_seed_websocket_file_priority(&right.file)
                .cmp(&auto_seed_websocket_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if request_body_parsing_task {
            auto_seed_request_body_parsing_file_priority(&right.file)
                .cmp(&auto_seed_request_body_parsing_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if response_headers_task {
            auto_seed_response_headers_file_priority(&right.file)
                .cmp(&auto_seed_response_headers_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if response_cookies_task {
            auto_seed_response_cookies_file_priority(&right.file)
                .cmp(&auto_seed_response_cookies_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if response_redirect_task {
            auto_seed_response_redirect_file_priority(&right.file)
                .cmp(&auto_seed_response_redirect_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if request_lifecycle_task {
            auto_seed_request_lifecycle_file_priority(&right.file)
                .cmp(&auto_seed_request_lifecycle_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if middleware_task {
            auto_seed_middleware_file_priority(&right.file)
                .cmp(&auto_seed_middleware_file_priority(&left.file))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else if route_dispatch_task {
            auto_seed_route_dispatch_file_priority(&right.file, task_keywords)
                .cmp(&auto_seed_route_dispatch_file_priority(
                    &left.file,
                    task_keywords,
                ))
                .then_with(|| right.score.cmp(&left.score))
                .then_with(|| left.file.cmp(&right.file))
        } else {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.file.cmp(&right.file))
        }
    });

    let priority_routed_task = request_body_parsing_task
        || response_headers_task
        || response_cookies_task
        || response_redirect_task
        || request_lifecycle_task
        || middleware_task
        || route_dispatch_task
        || (agent_first_read_task && !auto_seed_agent_first_read_evidence_task(task_keywords))
        || indexing_pipeline_task
        || data_persistence_task
        || semantic_context_task
        || dependency_graph_task
        || project_overview_task
        || symbol_search_task
        || file_parsing_task
        || binding_validation_task
        || import_resolution_task;
    let selected_candidate =
        if route_miss_task || auto_seed_prefers_entrypoint(task_keywords) || startup_flow_task {
            candidates.first()
        } else if priority_routed_task {
            candidates
                .first()
                .filter(|candidate| {
                    auto_seed_priority_routed_file_priority(&candidate.file, task_keywords) > 0
                })
                .or_else(|| {
                    candidates
                        .iter()
                        .find(|candidate| candidate.source == "task_match")
                })
                .or_else(|| candidates.first())
        } else {
            candidates
                .iter()
                .find(|candidate| candidate.source == "task_match")
                .or_else(|| candidates.first())
        };

    if let Some(candidate) = selected_candidate {
        let file = candidate.file.clone();
        let source = if priority_routed_task
            && auto_seed_priority_routed_file_priority(&candidate.file, task_keywords) > 0
        {
            "task_match".to_string()
        } else {
            candidate.source.clone()
        };
        let strategy = if source == "task_match" {
            "auto_task_match"
        } else {
            "auto_entrypoint"
        };
        let companion_entrypoint = (source == "task_match")
            .then(|| {
                overview
                    .entrypoints
                    .iter()
                    .find(|entrypoint| {
                        entrypoint.role == "source"
                            && entrypoint.file != file
                            && auto_seed_role_allowed(&entrypoint.role, task_keywords)
                            && auto_seed_role_allowed(
                                auto_seed_file_role(&entrypoint.file),
                                task_keywords,
                            )
                            && auto_seed_companion_entrypoint_allowed(
                                &file,
                                &entrypoint.file,
                                task_keywords,
                            )
                            && (!agent_first_read_task
                                || auto_seed_agent_first_read_file_priority(
                                    &entrypoint.file,
                                    task_keywords,
                                ) >= 0)
                            && (!indexing_pipeline_task
                                || auto_seed_indexing_pipeline_file_priority(&entrypoint.file) >= 0)
                            && (!data_persistence_task
                                || auto_seed_data_persistence_file_priority(&entrypoint.file) >= 0)
                            && (!semantic_context_task
                                || auto_seed_semantic_context_file_priority(
                                    &entrypoint.file,
                                    semantic_context_prefers_orchestration,
                                ) >= 0)
                            && (!dependency_graph_task
                                || auto_seed_dependency_graph_file_priority(&entrypoint.file) >= 0)
                            && (!project_overview_task
                                || auto_seed_project_overview_file_priority(&entrypoint.file) >= 0)
                            && (!symbol_search_task
                                || auto_seed_symbol_search_file_priority(&entrypoint.file) >= 0)
                            && (!file_parsing_task
                                || auto_seed_file_parsing_file_priority(&entrypoint.file) >= 0)
                            && (!binding_validation_task
                                || auto_seed_binding_validation_file_priority_for_task(
                                    &entrypoint.file,
                                    task_keywords,
                                ) >= 0)
                            && (!import_resolution_task
                                || auto_seed_import_resolution_file_priority(&entrypoint.file) >= 0)
                    })
                    .map(|entrypoint| AutoSeedCandidate {
                        file: entrypoint.file.clone(),
                        role: entrypoint.role.clone(),
                        source: "overview_entrypoint".to_string(),
                        score: entrypoint.score as i32,
                        matched_keywords: auto_seed_matched_keywords(
                            &entrypoint.file,
                            entrypoint.symbol.as_deref(),
                            task_keywords,
                        ),
                        matched_symbols: entrypoint.symbol.iter().cloned().collect(),
                    })
            })
            .flatten();
        let mut files = vec![file.clone()];
        let mut seeds = vec![ContextSeed {
            kind: "file".to_string(),
            value: file,
            source,
            start_line: None,
            end_line: None,
            locations: Vec::new(),
            role: Some(candidate.role.clone()),
            matched_keywords: candidate.matched_keywords.clone(),
            matched_symbols: candidate.matched_symbols.clone(),
        }];
        if let Some(entrypoint) = companion_entrypoint {
            files.push(entrypoint.file.clone());
            seeds.push(ContextSeed {
                kind: "file".to_string(),
                value: entrypoint.file,
                source: entrypoint.source,
                start_line: None,
                end_line: None,
                locations: Vec::new(),
                role: Some(entrypoint.role),
                matched_keywords: entrypoint.matched_keywords,
                matched_symbols: entrypoint.matched_symbols,
            });
        }
        return Ok(AutoContextSeedSelection {
            strategy: strategy.to_string(),
            files,
            seeds,
            task_path_locations: BTreeMap::new(),
        });
    }

    let files = indexed_files
        .into_iter()
        .filter(|file| auto_seed_role_allowed(auto_seed_file_role(file), task_keywords))
        .take(3)
        .collect::<Vec<_>>();
    let seeds = files
        .iter()
        .map(|file| ContextSeed {
            kind: "file".to_string(),
            value: file.clone(),
            source: "indexed_file_fallback".to_string(),
            start_line: None,
            end_line: None,
            locations: Vec::new(),
            role: Some(auto_seed_file_role(file).to_string()),
            matched_keywords: Vec::new(),
            matched_symbols: Vec::new(),
        })
        .collect::<Vec<_>>();

    Ok(AutoContextSeedSelection {
        strategy: "auto_source_fallback".to_string(),
        files,
        seeds,
        task_path_locations: BTreeMap::new(),
    })
}

pub(crate) fn task_has_existing_path(root: &Path, task: &str) -> bool {
    auto_seed_task_path_tokens(root, task)
        .into_iter()
        .any(|token| auto_seed_task_path_exists_in_project(root, &token))
}

fn auto_seed_task_path_exists_in_project(root: &Path, token: &str) -> bool {
    let Ok(canonical_root) = root.canonicalize() else {
        return false;
    };
    let Ok(canonical_path) = root.join(token).canonicalize() else {
        return false;
    };
    canonical_path.is_file() && canonical_path.starts_with(canonical_root)
}

fn auto_seed_task_path_tokens(root: &Path, task: &str) -> Vec<String> {
    let mut seen = BTreeSet::new();
    auto_seed_task_path_references(root, task)
        .into_iter()
        .filter(|reference| seen.insert(reference.file.clone()))
        .map(|reference| reference.file)
        .collect()
}

fn auto_seed_task_path_references(root: &Path, task: &str) -> Vec<TaskPathReference> {
    let canonical_root = root.canonicalize().ok();
    let mut seen = BTreeSet::new();
    auto_seed_task_path_candidates(task)
        .into_iter()
        .filter_map(|token| {
            normalize_auto_seed_task_path_reference(canonical_root.as_deref(), token)
        })
        .filter(|reference| reference.file.contains('/'))
        .filter(|reference| {
            seen.insert((
                reference.file.clone(),
                reference
                    .location
                    .map(|location| (location.start_line, location.end_line)),
            ))
        })
        .collect()
}

fn auto_seed_task_path_candidates(task: &str) -> Vec<&str> {
    let mut candidates = Vec::new();
    let mut token_start = None;
    let mut quote = None;

    for (index, character) in task.char_indices() {
        if let Some(active_quote) = quote {
            if character == active_quote {
                if let Some(start) = token_start.take() {
                    candidates.push(&task[start..index]);
                }
                quote = None;
            }
            continue;
        }

        if matches!(character, '\'' | '"' | '`') {
            if let Some(start) = token_start.take() {
                candidates.push(&task[start..index]);
            }
            quote = Some(character);
            token_start = Some(index + character.len_utf8());
        } else if auto_seed_task_path_character(character) {
            token_start.get_or_insert(index);
        } else if let Some(start) = token_start.take() {
            candidates.push(&task[start..index]);
        }
    }

    if let Some(start) = token_start {
        candidates.push(&task[start..]);
    }
    candidates
}

fn auto_seed_task_path_is_project_relative(token: &str) -> bool {
    !token.is_empty()
        && Path::new(token)
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

fn auto_seed_task_path_character(character: char) -> bool {
    character.is_ascii_alphanumeric()
        || matches!(character, '/' | '\\' | '.' | '_' | '-' | '+' | '#' | ':')
}

fn normalize_auto_seed_task_path_reference(
    canonical_root: Option<&Path>,
    token: &str,
) -> Option<TaskPathReference> {
    let normalized = token.replace('\\', "/");
    let (path_token, location) = split_auto_seed_task_path_location(&normalized);
    let path = Path::new(path_token);
    let file = if path.is_absolute() {
        let canonical_path = path.canonicalize().ok()?;
        let relative_path = canonical_path.strip_prefix(canonical_root?).ok()?;
        let relative_path = relative_path.to_string_lossy().replace('\\', "/");
        auto_seed_task_path_is_project_relative(&relative_path).then_some(relative_path)?
    } else {
        let relative_path = path_token.trim_start_matches("./").to_string();
        auto_seed_task_path_is_project_relative(&relative_path).then_some(relative_path)?
    };

    Some(TaskPathReference { file, location })
}

fn split_auto_seed_task_path_location(token: &str) -> (&str, Option<TaskPathLocation>) {
    if let Some(fragment_start) = token.rfind("#L") {
        let fragment = &token[fragment_start + 2..];
        let location = fragment.split_once("-L").map_or_else(
            || {
                decimal_location_value(fragment).map(|line| TaskPathLocation {
                    start_line: line,
                    end_line: line,
                })
            },
            |(start, end)| {
                Some(TaskPathLocation {
                    start_line: decimal_location_value(start)?,
                    end_line: decimal_location_value(end)?,
                })
            },
        );
        if let Some(location) = location {
            return (
                &token[..fragment_start],
                Some(normalize_task_path_location(location)),
            );
        }
    }

    let Some(last_colon) = token.rfind(':') else {
        return (token, None);
    };
    let Some(last_value) = decimal_location_value(&token[last_colon + 1..]) else {
        return (token, None);
    };

    if let Some(previous_colon) = token[..last_colon].rfind(':')
        && let Some(line) = decimal_location_value(&token[previous_colon + 1..last_colon])
    {
        return (
            &token[..previous_colon],
            Some(TaskPathLocation {
                start_line: line,
                end_line: line,
            }),
        );
    }

    (
        &token[..last_colon],
        Some(TaskPathLocation {
            start_line: last_value,
            end_line: last_value,
        }),
    )
}

fn normalize_task_path_location(location: TaskPathLocation) -> TaskPathLocation {
    TaskPathLocation {
        start_line: location.start_line.min(location.end_line),
        end_line: location.start_line.max(location.end_line),
    }
}

fn decimal_location_value(value: &str) -> Option<usize> {
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse::<usize>().ok())
        .flatten()
        .filter(|value| *value > 0)
}

fn auto_seed_task_symbol_matches(
    store: &Store,
    task_keywords: &[String],
) -> Result<BTreeMap<String, Vec<Symbol>>> {
    let mut matches_by_file = BTreeMap::<String, Vec<Symbol>>::new();
    let mut seen = BTreeSet::<(String, String, String)>::new();
    for keyword in task_keywords
        .iter()
        .filter(|keyword| keyword.len() >= 4 && auto_seed_text_keyword_allowed(keyword))
    {
        for symbol in store.search_symbols(keyword, 24)? {
            let key = (
                symbol.file.clone(),
                symbol.name.clone(),
                symbol.qualified_name.clone(),
            );
            if seen.insert(key) {
                matches_by_file
                    .entry(symbol.file.clone())
                    .or_default()
                    .push(symbol);
            }
        }
    }
    Ok(matches_by_file)
}

fn upsert_auto_seed_candidate(
    candidates: &mut BTreeMap<String, AutoSeedCandidate>,
    candidate: AutoSeedCandidate,
) {
    let entry = candidates
        .entry(candidate.file.clone())
        .or_insert_with(|| AutoSeedCandidate {
            file: candidate.file.clone(),
            role: candidate.role.clone(),
            source: candidate.source.clone(),
            score: candidate.score,
            matched_keywords: candidate.matched_keywords.clone(),
            matched_symbols: candidate.matched_symbols.clone(),
        });
    if entry.source == "overview_entrypoint" && candidate.source == "task_match" {
        entry.score = entry.score.max(candidate.score);
        entry.matched_keywords = entry
            .matched_keywords
            .iter()
            .cloned()
            .chain(candidate.matched_keywords)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        entry.matched_symbols = entry
            .matched_symbols
            .iter()
            .cloned()
            .chain(candidate.matched_symbols)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        return;
    }
    if candidate.score > entry.score
        || (candidate.score == entry.score && candidate.source == "task_match")
    {
        *entry = candidate;
    }
}

fn auto_seed_matched_symbols(symbols: &[Symbol], task_keywords: &[String]) -> Vec<String> {
    let mut scored_symbols = symbols
        .iter()
        .filter_map(|symbol| {
            let score = auto_seed_task_match_score(
                &symbol.file,
                Some(&symbol.qualified_name),
                task_keywords,
            );
            (score > 0).then(|| (score + auto_seed_symbol_kind_priority(&symbol.kind), symbol))
        })
        .collect::<Vec<_>>();
    scored_symbols.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.name.cmp(&right.1.name))
    });
    let mut seen = BTreeSet::new();
    let mut names = Vec::new();
    for (_score, symbol) in scored_symbols {
        if seen.insert(symbol.name.clone()) {
            names.push(symbol.name.clone());
            if names.len() >= 3 {
                break;
            }
        }
    }
    names
}

fn auto_seed_symbol_kind_priority(kind: &SymbolKind) -> i32 {
    match kind {
        SymbolKind::Function | SymbolKind::Method => 80,
        SymbolKind::Class | SymbolKind::Interface | SymbolKind::Struct => 30,
        SymbolKind::Variable | SymbolKind::Constant => 0,
    }
}

fn auto_seed_matched_keywords(
    file: &str,
    symbol: Option<&str>,
    task_keywords: &[String],
) -> Vec<String> {
    task_keywords
        .iter()
        .filter(|keyword| {
            auto_seed_field_matches(file, keyword)
                || symbol
                    .map(|symbol| auto_seed_field_matches(symbol, keyword))
                    .unwrap_or(false)
        })
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn auto_seed_file_matched_keywords(
    root: &Path,
    file: &str,
    symbol: Option<&str>,
    task_keywords: &[String],
) -> Vec<String> {
    auto_seed_matched_keywords(file, symbol, task_keywords)
        .into_iter()
        .chain(auto_seed_file_text_match(root, file, task_keywords).matched_keywords)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn auto_seed_task_match_score(file: &str, symbol: Option<&str>, task_keywords: &[String]) -> i32 {
    if task_keywords.is_empty() {
        return 0;
    }

    let mut score = 0;
    for (index, keyword) in task_keywords.iter().enumerate() {
        let boost = 70 + (task_keywords.len().saturating_sub(index) as i32 * 3);
        let file_weight = auto_seed_field_match_weight(file, keyword);
        let symbol_weight = symbol
            .map(|symbol| auto_seed_field_match_weight(symbol, keyword))
            .unwrap_or(0);
        score += boost * file_weight.max(symbol_weight);
    }
    score
}

fn auto_seed_task_file_stem_score(file: &str, task_keywords: &[String]) -> i32 {
    let Some(stem) = Path::new(file).file_stem().and_then(|stem| stem.to_str()) else {
        return 0;
    };

    task_keywords
        .iter()
        .filter(|keyword| auto_seed_text_keyword_allowed(keyword))
        .map(|keyword| auto_seed_field_match_weight(stem, keyword) * 420)
        .sum()
}

fn auto_seed_type_declaration_file(normalized_file: &str) -> bool {
    normalized_file.ends_with(".d.ts")
        || normalized_file.ends_with(".d.mts")
        || normalized_file.ends_with(".d.cts")
}

fn auto_seed_type_declaration_task(task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "type" | "types" | "declaration" | "declarations" | "interface" | "interfaces"
        )
    })
}

fn auto_seed_task_focus_boost(
    file: &str,
    symbol: Option<&str>,
    task_keywords: &[String],
    overview_entrypoint: bool,
) -> i32 {
    let mut score = 0;
    let normalized_file = file.replace('\\', "/").to_ascii_lowercase();

    if auto_seed_task_named_package_source_root(&normalized_file, task_keywords).is_some() {
        score += 900;
        score += auto_seed_task_file_stem_score(file, task_keywords);
    }
    if auto_seed_type_declaration_file(&normalized_file)
        && !auto_seed_type_declaration_task(task_keywords)
    {
        score -= 1400;
    }

    if auto_seed_agent_first_read_task(task_keywords) {
        let file_match = auto_seed_agent_first_read_field_matches(file);
        let symbol_match = symbol
            .map(auto_seed_agent_first_read_field_matches)
            .unwrap_or(false);

        score += match (file_match, symbol_match) {
            (true, true) => 5000,
            (true, false) => 3500,
            (false, true) => 3200,
            _ => 0,
        };
    }

    if task_keywords
        .iter()
        .any(|keyword| auto_seed_lifecycle_keyword(keyword))
    {
        let lifecycle_match = task_keywords
            .iter()
            .filter(|keyword| auto_seed_lifecycle_keyword(keyword))
            .any(|keyword| {
                auto_seed_field_matches(file, keyword)
                    || symbol
                        .map(|symbol| auto_seed_field_matches(symbol, keyword))
                        .unwrap_or(false)
            });
        let entrypoint_symbol = symbol
            .map(|symbol| auto_seed_field_matches(symbol, "main"))
            .unwrap_or(false)
            || auto_seed_field_matches(file, "main");
        let entrypoint_file = auto_seed_entrypoint_file_matches(file);

        score += match (
            overview_entrypoint,
            lifecycle_match,
            entrypoint_symbol || entrypoint_file,
        ) {
            (true, true, true) => 760,
            (true, true, false) => 260,
            (true, false, true) => 120,
            (false, true, true) => 360,
            (false, true, _) => 90,
            _ => 0,
        };
    }

    if task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "config" | "configuration"))
        && auto_seed_file_stem_matches(file, "config")
    {
        score += 260;
    }

    if task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "session" | "sessions"))
        && (auto_seed_file_stem_matches(file, "session")
            || auto_seed_file_stem_matches(file, "sessions"))
    {
        score += 320;
    }

    if auto_seed_response_headers_task(task_keywords) {
        let file_action_match = auto_seed_response_headers_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_response_headers_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_response_headers_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2700,
            (true, false) => 1500,
            (false, true) => 1200,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 2400;
        } else if framework_file_match {
            score += 1700;
        }
    }

    if auto_seed_response_cookies_task(task_keywords) {
        let file_action_match = auto_seed_response_cookies_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_response_cookies_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_response_cookies_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2800,
            (true, false) => 1500,
            (false, true) => 1200,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 2600;
        } else if framework_file_match {
            score += 1800;
        }
    }

    if auto_seed_http_state_headers_task(task_keywords) {
        let file_action_match = auto_seed_http_state_headers_file_matches(file, task_keywords);
        let symbol_action_match = symbol
            .map(|symbol| auto_seed_http_state_headers_symbol_matches(symbol, task_keywords))
            .unwrap_or(false);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2000,
            (true, false) => 1500,
            (false, true) => 900,
            _ => 0,
        };
    }

    if auto_seed_request_body_parsing_task(task_keywords) {
        let file_action_match = auto_seed_request_body_parsing_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_request_body_parsing_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_request_body_parsing_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2500,
            (true, false) => 1500,
            (false, true) => 1100,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 1900;
        } else if framework_file_match {
            score += 1400;
        }
    }

    if auto_seed_binding_validation_task(task_keywords) {
        let priority = auto_seed_binding_validation_file_priority_for_task(file, task_keywords);
        let symbol_validation_match = symbol
            .map(|symbol| {
                auto_seed_field_matches(symbol, "validation")
                    || auto_seed_field_matches(symbol, "validator")
                    || auto_seed_field_matches(symbol, "schema")
                    || auto_seed_field_matches(symbol, "json")
                    || auto_seed_field_matches(symbol, "binding")
            })
            .unwrap_or(false);

        if priority >= 240 {
            score += 2400;
        } else if priority >= 160 {
            score += 1200;
        }
        if symbol_validation_match {
            score += 900;
        }
    }

    if auto_seed_request_query_params_task(task_keywords) {
        let file_action_match = auto_seed_request_query_params_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_request_query_params_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_request_query_params_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2600,
            (true, false) => 1400,
            (false, true) => 1100,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 2200;
        } else if framework_file_match {
            score += 1500;
        }
    }

    if auto_seed_route_parameters_task(task_keywords) {
        let file_action_match = auto_seed_route_parameters_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_route_parameters_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_route_parameters_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2800,
            (true, false) => 1500,
            (false, true) => 1200,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 5000;
        } else if framework_file_match {
            score += 4000;
        }
    }

    if auto_seed_url_building_task(task_keywords) {
        let file_action_match = auto_seed_url_building_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_url_building_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_url_building_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2700,
            (true, false) => 1400,
            (false, true) => 1200,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 2600;
        } else if framework_file_match {
            score += 1600;
        }
    }

    if auto_seed_route_grouping_task(task_keywords) {
        let file_action_match = auto_seed_route_grouping_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_route_grouping_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_route_grouping_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2900,
            (true, false) => 1500,
            (false, true) => 1200,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 3000;
        } else if framework_file_match {
            score += 2100;
        }
    }

    if auto_seed_route_miss_handling_task(task_keywords) {
        let file_action_match = auto_seed_route_miss_handling_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_route_miss_handling_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_route_miss_handling_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 3000,
            (true, false) => 1600,
            (false, true) => 1300,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 3400;
        } else if framework_file_match {
            score += 2400;
        }
    }

    if auto_seed_http_method_routing_task(task_keywords) {
        let file_action_match = auto_seed_http_method_routing_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_http_method_routing_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_http_method_routing_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 3000,
            (true, false) => 1600,
            (false, true) => 1300,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 3200;
        } else if framework_file_match {
            score += 2200;
        }
    }

    if auto_seed_route_dispatch_task(task_keywords) {
        let file_action_match = auto_seed_route_dispatch_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_route_dispatch_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_route_dispatch_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 3000,
            (true, false) => 1600,
            (false, true) => 1300,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 3200;
        } else if framework_file_match {
            score += 2200;
        }
    }

    if auto_seed_response_redirect_task(task_keywords) {
        let file_action_match = auto_seed_response_redirect_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_response_redirect_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_response_redirect_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2600,
            (true, false) => 1400,
            (false, true) => 1100,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 2100;
        } else if framework_file_match {
            score += 1500;
        }
    }

    if auto_seed_static_file_serving_task(task_keywords) {
        let file_action_match = auto_seed_static_file_serving_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_static_file_serving_symbol_matches)
            .unwrap_or(false);
        let framework_file_match = auto_seed_static_file_serving_framework_file_matches(file);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2400,
            (true, false) => 1500,
            (false, true) => 1200,
            _ => 0,
        };
        if framework_file_match && symbol_action_match {
            score += 1800;
        } else if framework_file_match {
            score += 1400;
        }
    }

    if auto_seed_response_rendering_task(task_keywords) {
        let central_file_match = auto_seed_response_rendering_central_file_matches(file);
        let response_file_match =
            auto_seed_response_rendering_response_file_matches(file, task_keywords);
        let file_action_match = auto_seed_response_rendering_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_response_rendering_symbol_matches)
            .unwrap_or(false);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 2000,
            (true, false) => 1500,
            (false, true) => 900,
            _ => 0,
        };
        if central_file_match {
            score += 900;
        }
        if response_file_match {
            score += 2400;
        }
    }

    if auto_seed_request_lifecycle_task(task_keywords) {
        let file_lifecycle_match = auto_seed_request_lifecycle_file_matches(file);
        let symbol_lifecycle_match = symbol
            .map(auto_seed_request_lifecycle_symbol_matches)
            .unwrap_or(false);

        score += match (
            overview_entrypoint,
            file_lifecycle_match,
            symbol_lifecycle_match,
        ) {
            (true, true, true) => 2200,
            (false, true, true) => 1800,
            (_, _, true) => 700,
            (true, true, _) => 1800,
            (false, true, _) => 1400,
            _ => 0,
        };
    }

    if auto_seed_runtime_lifecycle_task(task_keywords) {
        let file_lifecycle_match = auto_seed_runtime_lifecycle_field_matches(file);
        let symbol_lifecycle_match = symbol
            .map(auto_seed_runtime_lifecycle_field_matches)
            .unwrap_or(false);

        score += match (file_lifecycle_match, symbol_lifecycle_match) {
            (true, true) => 2800,
            (true, false) => 1800,
            (false, true) => 1200,
            _ => 0,
        };
    }

    if auto_seed_file_upload_task(task_keywords) {
        let file_upload_match = auto_seed_file_upload_field_matches(file);
        let symbol_upload_match = symbol
            .map(auto_seed_file_upload_field_matches)
            .unwrap_or(false);

        score += match (file_upload_match, symbol_upload_match) {
            (true, true) => 2800,
            (true, false) => 1800,
            (false, true) => 1200,
            _ => 0,
        };
    }

    if auto_seed_websocket_connection_task(task_keywords) {
        let file_websocket_match = auto_seed_websocket_connection_field_matches(file);
        let symbol_websocket_match = symbol
            .map(auto_seed_websocket_connection_field_matches)
            .unwrap_or(false);

        score += match (file_websocket_match, symbol_websocket_match) {
            (true, true) => 3600,
            (true, false) => 2600,
            (false, true) => 1800,
            _ => 0,
        };
    }

    if auto_seed_error_recovery_handling_task(task_keywords) {
        let recovery_file_match =
            auto_seed_error_recovery_recovery_file_matches(file, task_keywords);
        let application_file_match =
            auto_seed_error_recovery_application_file_matches(file, task_keywords);
        let file_action_match = auto_seed_error_recovery_action_file_matches(file, task_keywords);
        let symbol_action_match = symbol
            .map(auto_seed_error_recovery_action_symbol_matches)
            .unwrap_or(false);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 1800,
            (true, false) => 1300,
            (false, true) => 700,
            _ => 0,
        };
        if recovery_file_match {
            score += 1800;
        }
        if application_file_match {
            score += 3000;
        }
    }

    if auto_seed_tls_certificate_task(task_keywords) {
        let file_action_match = auto_seed_tls_certificate_action_file_matches(file);
        let symbol_action_match = symbol
            .map(auto_seed_tls_certificate_action_symbol_matches)
            .unwrap_or(false);

        score += match (file_action_match, symbol_action_match) {
            (true, true) => 1900,
            (true, false) => 1400,
            (false, true) => 800,
            _ => 0,
        };
    }

    if task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "route" | "routes" | "router" | "routing"))
    {
        let file_route_registration_match = auto_seed_route_registration_matches(file);
        let symbol_route_registration_match = symbol
            .map(auto_seed_route_registration_matches)
            .unwrap_or(false);
        let file_route_match = auto_seed_field_matches(file, "route")
            || auto_seed_field_matches(file, "router")
            || auto_seed_field_matches(file, "routing");
        let symbol_route_match = symbol
            .map(|symbol| {
                auto_seed_field_matches(symbol, "route")
                    || auto_seed_field_matches(symbol, "router")
                    || auto_seed_field_matches(symbol, "routing")
            })
            .unwrap_or(false);

        score += match (
            overview_entrypoint,
            file_route_registration_match,
            symbol_route_registration_match,
        ) {
            (false, _, true) => 620,
            (false, true, _) => 520,
            (true, true, _) => 180,
            (true, false, true) => 160,
            _ => 0,
        };

        score += match (overview_entrypoint, file_route_match, symbol_route_match) {
            (false, true, _) => 360,
            (false, false, true) => 120,
            (true, true, _) => 120,
            (true, false, true) => 40,
            _ => 0,
        };
    }

    score
}

fn auto_seed_http_state_headers_task(task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "cookie"
                | "cookies"
                | "cookiejar"
                | "jar"
                | "headers"
                | "header"
                | "case"
                | "insensitive"
        )
    })
}

fn auto_seed_http_state_headers_file_matches(file: &str, task_keywords: &[String]) -> bool {
    let cookie_task = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "cookie" | "cookies" | "cookiejar" | "jar"));
    let header_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "headers" | "header" | "case" | "insensitive"
        )
    });

    (cookie_task
        && (auto_seed_file_stem_matches(file, "cookie")
            || auto_seed_file_stem_matches(file, "cookies")
            || auto_seed_file_stem_matches(file, "cookiejar")))
        || (header_task
            && (auto_seed_file_stem_matches(file, "header")
                || auto_seed_file_stem_matches(file, "headers")
                || auto_seed_file_stem_matches(file, "structure")
                || auto_seed_file_stem_matches(file, "structures")))
}

fn auto_seed_http_state_headers_symbol_matches(symbol: &str, task_keywords: &[String]) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);
    let cookie_task = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "cookie" | "cookies" | "cookiejar" | "jar"));
    let header_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "headers" | "header" | "case" | "insensitive"
        )
    });

    (cookie_task && (has_exact("cookie") || has_exact("cookies") || has_exact("cookiejar")))
        || (header_task
            && (has_exact("header")
                || has_exact("headers")
                || has_exact("caseinsensitive")
                || (has_exact("case") && has_exact("insensitive"))))
}

fn auto_seed_response_headers_task(task_keywords: &[String]) -> bool {
    let header_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "header" | "headers" | "contenttype" | "content-type"
        )
    });
    let response_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "response" | "responses" | "status" | "content" | "type" | "server" | "handler"
        )
    });
    let request_client_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "request"
                | "requests"
                | "client"
                | "network"
                | "proxy"
                | "proxies"
                | "adapter"
                | "adapters"
                | "transport"
                | "transports"
                | "binding"
                | "bind"
        )
    });

    header_task && response_task && !request_client_task
}

fn auto_seed_response_headers_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "response")
        || auto_seed_file_stem_matches(file, "responses")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
        || auto_seed_file_stem_matches(file, "wrapper")
        || auto_seed_file_stem_matches(file, "wrappers")
        || auto_seed_file_stem_matches(file, "render")
        || auto_seed_file_stem_matches(file, "renderer")
}

fn auto_seed_response_headers_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "response")
        || auto_seed_file_stem_matches(file, "responses")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
        || auto_seed_file_stem_matches(file, "wrapper")
        || auto_seed_file_stem_matches(file, "wrappers")
}

fn auto_seed_response_headers_file_priority(file: &str) -> i32 {
    auto_seed_framework_action_file_priority(
        file,
        auto_seed_response_headers_framework_file_matches(file),
        auto_seed_response_headers_file_matches(file),
    )
}

fn auto_seed_response_headers_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("header")
        || has_exact("headers")
        || has_exact("status")
        || has_exact("contenttype")
        || has_exact("content")
        || has_exact("type")
        || (has_exact("set") && has_exact("cookie"))
        || matches!(
            normalized.as_str(),
            "header"
                | "headers"
                | "set"
                | "get"
                | "append"
                | "vary"
                | "status"
                | "sendstatus"
                | "writeheader"
                | "writeheadernow"
                | "writestatusnow"
                | "writecontenttype"
                | "contenttype"
                | "setcontenttype"
                | "setheader"
                | "appendheader"
                | "setcookie"
        )
}

fn auto_seed_response_cookies_task(task_keywords: &[String]) -> bool {
    let cookie_task = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "cookie" | "cookies"));
    let response_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "response"
                | "responses"
                | "set"
                | "send"
                | "server"
                | "handler"
                | "header"
                | "headers"
                | "option"
                | "options"
        )
    });
    let client_cookie_jar_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "request" | "requests" | "client" | "network"
        )
    });

    cookie_task && response_task && !client_cookie_jar_task
}

fn auto_seed_response_cookies_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "response")
        || auto_seed_file_stem_matches(file, "responses")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
        || auto_seed_file_stem_matches(file, "session")
        || auto_seed_file_stem_matches(file, "sessions")
        || auto_seed_file_stem_matches(file, "cookie")
        || auto_seed_file_stem_matches(file, "cookies")
}

fn auto_seed_response_cookies_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "response")
        || auto_seed_file_stem_matches(file, "responses")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "session")
        || auto_seed_file_stem_matches(file, "sessions")
}

fn auto_seed_response_cookies_file_priority(file: &str) -> i32 {
    auto_seed_framework_action_file_priority(
        file,
        auto_seed_response_cookies_framework_file_matches(file),
        auto_seed_response_cookies_file_matches(file),
    )
}

fn auto_seed_response_cookies_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("cookie")
        || has_exact("cookies")
        || (has_exact("set") && has_exact("cookie"))
        || matches!(
            normalized.as_str(),
            "cookie" | "cookies" | "setcookie" | "setcookiedata" | "deletecookie" | "clearcookie"
        )
}

fn auto_seed_request_body_parsing_task(task_keywords: &[String]) -> bool {
    let request_or_payload = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "request" | "requests" | "body" | "bodies" | "payload" | "payloads"
        )
    });
    let parsing_or_binding = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "parse"
                | "parser"
                | "parsers"
                | "parsing"
                | "bind"
                | "binding"
                | "bindings"
                | "decode"
                | "decoder"
                | "form"
                | "multipart"
                | "contenttype"
                | "content-type"
        )
    });

    request_or_payload && parsing_or_binding
}

fn auto_seed_request_body_parsing_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "express")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "wrappers")
        || auto_seed_file_stem_matches(file, "wrapper")
        || auto_seed_file_stem_matches(file, "request")
        || auto_seed_file_stem_matches(file, "requests")
        || auto_seed_file_stem_matches(file, "binding")
        || auto_seed_file_stem_matches(file, "bindings")
        || auto_seed_file_stem_matches(file, "json")
        || auto_seed_file_stem_matches(file, "form")
        || auto_seed_file_stem_matches(file, "multipart")
        || auto_seed_file_stem_matches(file, "plain")
        || auto_seed_file_stem_matches(file, "xml")
        || auto_seed_file_stem_matches(file, "yaml")
        || auto_seed_file_stem_matches(file, "toml")
}

fn auto_seed_request_body_parsing_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "express")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "wrappers")
        || auto_seed_file_stem_matches(file, "wrapper")
}

fn auto_seed_request_body_parsing_file_priority(file: &str) -> i32 {
    auto_seed_framework_action_file_priority(
        file,
        auto_seed_request_body_parsing_framework_file_matches(file),
        auto_seed_request_body_parsing_file_matches(file),
    )
}

fn auto_seed_request_body_parsing_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("body")
        || has_exact("bodyparser")
        || has_exact("payload")
        || has_exact("json")
        || has_exact("raw")
        || has_exact("text")
        || has_exact("urlencoded")
        || has_exact("bind")
        || has_exact("binding")
        || has_exact("form")
        || has_exact("multipart")
        || has_exact("contenttype")
        || has_exact("decode")
        || has_exact("decoder")
        || (has_exact("load") && has_exact("form") && has_exact("data"))
        || matches!(
            normalized.as_str(),
            "bodyparser"
                | "bodybyteskey"
                | "bindjson"
                | "bindxml"
                | "bindyaml"
                | "bindtoml"
                | "bindplain"
                | "mustbindwith"
                | "shouldbind"
                | "shouldbindwith"
                | "shouldbindbodywith"
                | "shouldbindjson"
                | "shouldbindxml"
                | "shouldbindyaml"
                | "shouldbindtoml"
                | "shouldbindplain"
                | "loadformdata"
                | "onjsonloadingfailed"
                | "maxformmemorysize"
                | "maxformparts"
        )
}

fn auto_seed_agent_first_read_task(task_keywords: &[String]) -> bool {
    let agent_or_context = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "agent" | "agents" | "assistant" | "assistants" | "context" | "adoption" | "evidence"
        )
    });
    let first_read_or_route = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "first"
                | "read"
                | "reading"
                | "routing"
                | "route"
                | "router"
                | "quality"
                | "workflow"
                | "pack"
        )
    });
    let reading_plan_handoff = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "reading" | "plan" | "execution"))
        && task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "tool" | "tools" | "handoff"));
    let omitted_candidate_follow_up = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "omitted" | "candidate" | "candidates"))
        && task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "follow" | "followup" | "continuation"));
    let source_line_reduction = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "source"))
        && task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "line" | "lines"))
        && task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "reduction" | "metrics" | "metric"));
    let current_reading_step_contract = auto_seed_current_reading_step_contract_task(task_keywords);

    (agent_or_context && first_read_or_route)
        || reading_plan_handoff
        || omitted_candidate_follow_up
        || source_line_reduction
        || current_reading_step_contract
}

fn auto_seed_current_reading_step_contract_task(task_keywords: &[String]) -> bool {
    let current_reading_step = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "current" | "currentreadingstep" | "current_reading_step"
        )
    }) && task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "reading" | "step"));
    let mirror = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "mirror" | "mirrors" | "mirrored"));
    let reading_plan = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "reading" | "plan" | "readingplan"));

    current_reading_step && (mirror || reading_plan)
}

fn auto_seed_agent_first_read_field_matches(field: &str) -> bool {
    auto_seed_field_matches(field, "agent")
        || auto_seed_field_matches(field, "workflow")
        || auto_seed_field_matches(field, "context")
        || auto_seed_field_matches(field, "pack")
        || auto_seed_field_matches(field, "readless")
        || auto_seed_field_matches(field, "evidence")
        || auto_seed_field_matches(field, "adoption")
}

fn auto_seed_agent_first_read_file_priority(file: &str, task_keywords: &[String]) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    let evidence_task = auto_seed_agent_first_read_evidence_task(task_keywords);

    if normalized == "docs" || normalized.starts_with("docs/") {
        return if evidence_task { 20 } else { -10 };
    }

    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return if evidence_task && auto_seed_agent_first_read_evidence_file(&normalized) {
            25
        } else if evidence_task {
            5
        } else {
            -80
        };
    }

    if is_low_value_reference_file(file) {
        return if evidence_task { 10 } else { -30 };
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let core_file = auto_seed_agent_first_read_core_file_matches(file);

    if source_file && core_file {
        120
    } else if core_file {
        70
    } else if source_file {
        20
    } else {
        0
    }
}

fn auto_seed_agent_first_read_core_file_matches(file: &str) -> bool {
    auto_seed_field_matches(file, "agent")
        || auto_seed_field_matches(file, "workflow")
        || auto_seed_field_matches(file, "context")
        || auto_seed_field_matches(file, "pack")
        || auto_seed_field_matches(file, "router")
        || auto_seed_field_matches(file, "routing")
        || auto_seed_field_matches(file, "route")
        || auto_seed_field_matches(file, "tool")
        || auto_seed_field_matches(file, "tools")
        || auto_seed_field_matches(file, "mcp")
}

fn auto_seed_agent_first_read_evidence_task(task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "adoption"
                | "benchmark"
                | "benchmarks"
                | "beta"
                | "demo"
                | "demos"
                | "evidence"
                | "external"
                | "report"
                | "reports"
                | "release"
                | "releases"
                | "smoke"
                | "smokes"
                | "trial"
                | "trials"
        )
    })
}

fn auto_seed_agent_first_read_evidence_file(normalized_file: &str) -> bool {
    normalized_file.contains("adoption")
        || normalized_file.contains("benchmark")
        || normalized_file.contains("beta")
        || normalized_file.contains("demo")
        || normalized_file.contains("evidence")
        || normalized_file.contains("report")
        || normalized_file.contains("release")
        || normalized_file.contains("smoke")
        || normalized_file.contains("summary")
        || normalized_file.contains("trial")
}

fn auto_seed_request_query_params_task(task_keywords: &[String]) -> bool {
    let query_task = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "query" | "queries"));
    let params_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "parameter"
                | "parameters"
                | "param"
                | "params"
                | "args"
                | "arguments"
                | "url"
                | "request"
                | "parse"
                | "parser"
                | "parsing"
        )
    });
    let non_http_query_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "database" | "sql" | "graphql" | "semantic" | "search"
        )
    });

    query_task && params_task && !non_http_query_task
}

fn auto_seed_request_query_params_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "request")
        || auto_seed_file_stem_matches(file, "requests")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
        || auto_seed_file_stem_matches(file, "query")
        || auto_seed_file_stem_matches(file, "queries")
        || auto_seed_file_stem_matches(file, "url")
        || auto_seed_file_stem_matches(file, "urls")
}

fn auto_seed_request_query_params_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "request")
        || auto_seed_file_stem_matches(file, "requests")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
}

fn auto_seed_request_query_params_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("query")
        || has_exact("queries")
        || has_exact("args")
        || has_exact("arguments")
        || (has_exact("url") && (has_exact("for") || has_exact("query")))
        || matches!(
            normalized.as_str(),
            "query"
                | "querybinding"
                | "defaultquery"
                | "getquery"
                | "queryarray"
                | "getqueryarray"
                | "querymap"
                | "getquerymap"
                | "initquerycache"
                | "compilequeryparser"
                | "parseextendedquerystring"
        )
}

fn auto_seed_route_parameters_task(task_keywords: &[String]) -> bool {
    let route_or_path = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "route" | "routes" | "router" | "routing" | "path" | "paths" | "url"
        )
    });
    let parameter_or_variable = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "parameter"
                | "parameters"
                | "param"
                | "params"
                | "variable"
                | "variables"
                | "wildcard"
                | "wildcards"
                | "viewargs"
                | "view_args"
        )
    });
    let query_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "query" | "queries" | "args" | "arguments" | "database" | "sql" | "graphql"
        )
    });

    route_or_path && parameter_or_variable && !query_task
}

fn auto_seed_route_parameters_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "tree")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "route")
        || auto_seed_file_stem_matches(file, "routes")
}

fn auto_seed_route_parameters_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "tree")
}

fn auto_seed_route_parameters_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("param")
        || has_exact("params")
        || has_exact("parameter")
        || has_exact("parameters")
        || has_exact("wildcard")
        || has_exact("wildcards")
        || has_exact("viewargs")
        || has_exact("view_args")
        || (has_exact("view") && has_exact("args"))
        || (has_exact("dispatch") && has_exact("request"))
        || (has_exact("add") && has_exact("url") && has_exact("rule"))
        || (has_exact("url") && has_exact("for"))
        || matches!(
            normalized.as_str(),
            "param"
                | "params"
                | "byname"
                | "addparam"
                | "viewargs"
                | "dispatchrequest"
                | "addurlrule"
                | "urlfor"
                | "getvalue"
                | "countparams"
        )
}

fn auto_seed_url_building_task(task_keywords: &[String]) -> bool {
    let url_or_path = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "url" | "urls" | "path" | "paths" | "route" | "routes" | "router" | "routing"
        )
    });
    let build_or_join = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "build"
                | "builds"
                | "builder"
                | "builders"
                | "building"
                | "generate"
                | "generates"
                | "generation"
                | "generator"
                | "generators"
                | "reverse"
                | "join"
                | "joins"
                | "joining"
                | "absolute"
                | "base"
        )
    });
    let parameter_or_query = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "query"
                | "queries"
                | "parameter"
                | "parameters"
                | "param"
                | "params"
                | "variable"
                | "variables"
                | "args"
                | "arguments"
                | "database"
                | "sql"
                | "graphql"
                | "static"
                | "filesystem"
        )
    });

    url_or_path && build_or_join && !parameter_or_query
}

fn auto_seed_url_building_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "route")
        || auto_seed_file_stem_matches(file, "routes")
        || auto_seed_file_stem_matches(file, "url")
        || auto_seed_file_stem_matches(file, "urls")
        || auto_seed_file_stem_matches(file, "path")
        || auto_seed_file_stem_matches(file, "paths")
}

fn auto_seed_url_building_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "application")
}

fn auto_seed_url_building_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    (has_exact("url") && (has_exact("for") || has_exact("build") || has_exact("builder")))
        || (has_exact("path") && (has_exact("join") || has_exact("joining")))
        || (has_exact("calculate") && has_exact("absolute") && has_exact("path"))
        || (has_exact("base") && has_exact("path"))
        || matches!(
            normalized.as_str(),
            "urlfor"
                | "urlbuilder"
                | "buildurl"
                | "buildurls"
                | "generateurl"
                | "reverseurl"
                | "joinpath"
                | "joinpaths"
                | "calculateabsolutepath"
                | "basepath"
        )
}

fn auto_seed_route_grouping_task(task_keywords: &[String]) -> bool {
    let grouping_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "blueprint"
                | "blueprints"
                | "mount"
                | "mounted"
                | "mounting"
                | "subrouter"
                | "subrouters"
                | "nested"
                | "group"
                | "groups"
                | "prefix"
                | "prefixes"
        )
    });
    let routing_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "route"
                | "routes"
                | "router"
                | "routing"
                | "app"
                | "application"
                | "middleware"
                | "handler"
                | "handlers"
        )
    });

    grouping_task && routing_task
}

fn auto_seed_route_grouping_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "blueprint")
        || auto_seed_file_stem_matches(file, "blueprints")
        || auto_seed_file_stem_matches(file, "scaffold")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "route")
        || auto_seed_file_stem_matches(file, "routes")
        || auto_seed_file_stem_matches(file, "group")
        || auto_seed_file_stem_matches(file, "groups")
}

fn auto_seed_route_grouping_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "blueprint")
        || auto_seed_file_stem_matches(file, "blueprints")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "scaffold")
}

fn auto_seed_route_grouping_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("blueprint")
        || has_exact("blueprints")
        || has_exact("mount")
        || has_exact("mounted")
        || has_exact("mounting")
        || has_exact("group")
        || has_exact("groups")
        || has_exact("prefix")
        || has_exact("subrouter")
        || has_exact("nested")
        || (has_exact("register") && has_exact("blueprint"))
        || (has_exact("add") && has_exact("url") && has_exact("rule"))
        || (has_exact("handle") && has_exact("fromlist"))
        || matches!(
            normalized.as_str(),
            "blueprint"
                | "blueprints"
                | "registerblueprint"
                | "register_blueprint"
                | "routergroup"
                | "group"
                | "groupfunc"
                | "handle"
                | "handlefromlist"
                | "addurlrule"
                | "mount"
                | "mounted"
                | "subrouter"
                | "prefix"
        )
}

fn auto_seed_route_miss_handling_task(task_keywords: &[String]) -> bool {
    let miss_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "404"
                | "405"
                | "no"
                | "not"
                | "found"
                | "notfound"
                | "noroute"
                | "nomethod"
                | "allowed"
                | "miss"
                | "missing"
                | "fallback"
                | "fallthrough"
                | "final"
                | "finalhandler"
                | "exception"
        )
    });
    let routing_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "route"
                | "routes"
                | "router"
                | "routing"
                | "method"
                | "methods"
                | "handler"
                | "handlers"
                | "http"
                | "request"
                | "response"
        )
    });
    let template_task = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "template" | "templates"));

    miss_task && routing_task && !template_task
}

fn auto_seed_route_miss_handling_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "gin")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "route")
        || auto_seed_file_stem_matches(file, "routes")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "wrapper")
        || auto_seed_file_stem_matches(file, "wrappers")
}

fn auto_seed_route_miss_handling_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "gin")
}

fn auto_seed_route_miss_file_priority(file: &str) -> i32 {
    if auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "gin")
    {
        2
    } else if auto_seed_file_stem_matches(file, "wrapper")
        || auto_seed_file_stem_matches(file, "wrappers")
        || auto_seed_file_stem_matches(file, "routergroup")
    {
        -1
    } else {
        0
    }
}

fn auto_seed_route_miss_handling_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("notfound")
        || has_exact("noroute")
        || has_exact("nomethod")
        || has_exact("finalhandler")
        || has_exact("routingexception")
        || has_exact("handlemethodnotallowed")
        || has_exact("serveerror")
        || (has_exact("not") && has_exact("found"))
        || (has_exact("method") && has_exact("not") && has_exact("allowed"))
        || (has_exact("handle") && has_exact("http") && has_exact("exception"))
        || (has_exact("raise") && has_exact("routing") && has_exact("exception"))
        || (has_exact("rebuild") && has_exact("404"))
        || (has_exact("rebuild") && has_exact("405"))
        || matches!(
            normalized.as_str(),
            "finalhandler"
                | "noroute"
                | "nomethod"
                | "allnoroute"
                | "allnomethod"
                | "rebuild404handlers"
                | "rebuild405handlers"
                | "serveerror"
                | "handlemethodnotallowed"
                | "routingexception"
                | "raiseroutingexception"
                | "handlehttpexception"
                | "dispatchrequest"
                | "fulldispatchrequest"
                | "notfound"
                | "methodnotallowed"
        )
}

fn auto_seed_http_method_routing_task(task_keywords: &[String]) -> bool {
    let method_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "method"
                | "methods"
                | "verb"
                | "verbs"
                | "get"
                | "post"
                | "put"
                | "delete"
                | "patch"
                | "options"
                | "head"
        )
    });
    let routing_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "http"
                | "https"
                | "request"
                | "requests"
                | "route"
                | "routes"
                | "router"
                | "routing"
                | "dispatch"
                | "register"
                | "registration"
                | "handler"
                | "handlers"
        )
    });
    let client_transport_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "proxy" | "proxies" | "adapter" | "adapters" | "transport" | "transports"
        )
    });
    let route_miss_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "404"
                | "405"
                | "no"
                | "not"
                | "found"
                | "notfound"
                | "noroute"
                | "nomethod"
                | "allowed"
                | "miss"
                | "missing"
                | "fallback"
                | "fallthrough"
                | "final"
                | "finalhandler"
                | "exception"
        )
    });

    method_task && routing_task && !client_transport_task && !route_miss_task
}

fn auto_seed_http_method_routing_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "route")
        || auto_seed_file_stem_matches(file, "routes")
        || auto_seed_file_stem_matches(file, "view")
        || auto_seed_file_stem_matches(file, "views")
        || auto_seed_file_stem_matches(file, "method")
        || auto_seed_file_stem_matches(file, "methods")
        || auto_seed_file_stem_matches(file, "utils")
}

fn auto_seed_http_method_routing_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "views")
        || auto_seed_file_stem_matches(file, "view")
}

fn auto_seed_http_method_routing_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("method")
        || has_exact("methods")
        || has_exact("httpmethod")
        || has_exact("dispatch")
        || has_exact("dispatchrequest")
        || has_exact("handle")
        || has_exact("match")
        || has_exact("any")
        || has_exact("get")
        || has_exact("post")
        || has_exact("put")
        || has_exact("delete")
        || has_exact("patch")
        || has_exact("options")
        || has_exact("head")
        || matches!(
            normalized.as_str(),
            "httpmethod"
                | "httpmethods"
                | "methods"
                | "dispatchrequest"
                | "methodview"
                | "asview"
                | "handle"
                | "match"
                | "any"
                | "get"
                | "post"
                | "put"
                | "delete"
                | "patch"
                | "options"
                | "head"
        )
}

fn auto_seed_route_dispatch_task(task_keywords: &[String]) -> bool {
    let routing_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "url" | "urls" | "route" | "routes" | "router" | "routing"
        )
    });
    let dispatch_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "app"
                | "application"
                | "engine"
                | "handler"
                | "handlers"
                | "dispatch"
                | "match"
                | "matching"
                | "register"
                | "registration"
                | "resolver"
                | "resolvers"
                | "resolve"
                | "resolving"
                | "behavior"
                | "flow"
        )
    });

    routing_task
        && dispatch_task
        && !auto_seed_request_query_params_task(task_keywords)
        && !auto_seed_route_parameters_task(task_keywords)
        && !auto_seed_url_building_task(task_keywords)
        && !auto_seed_route_grouping_task(task_keywords)
        && !auto_seed_route_miss_handling_task(task_keywords)
        && !auto_seed_http_method_routing_task(task_keywords)
        && !auto_seed_static_file_serving_task(task_keywords)
}

fn auto_seed_route_dispatch_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "express")
        || auto_seed_file_stem_matches(file, "gin")
        || auto_seed_file_stem_matches(file, "scaffold")
        || auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "routing")
        || auto_seed_file_stem_matches(file, "route")
        || auto_seed_file_stem_matches(file, "routes")
        || auto_seed_file_stem_matches(file, "url")
        || auto_seed_file_stem_matches(file, "urls")
        || auto_seed_file_stem_matches(file, "resolver")
        || auto_seed_file_stem_matches(file, "resolvers")
        || auto_seed_file_stem_matches(file, "tree")
}

fn auto_seed_route_dispatch_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "express")
        || auto_seed_file_stem_matches(file, "gin")
        || auto_seed_file_stem_matches(file, "scaffold")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "routing")
        || auto_seed_file_stem_matches(file, "resolver")
        || auto_seed_file_stem_matches(file, "resolvers")
}

fn auto_seed_route_dispatch_file_priority(file: &str, task_keywords: &[String]) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized.contains("/checks/")
        || normalized.starts_with("checks/")
        || normalized.contains("/admindocs/")
        || normalized.starts_with("docs/")
        || normalized.starts_with("docs_src/")
        || normalized.starts_with("examples/")
        || normalized.contains("/tests/")
        || normalized.starts_with("tests/")
    {
        -2
    } else if auto_seed_task_named_package_source_file_priority(&normalized, task_keywords) > 0 {
        auto_seed_task_named_package_source_file_priority(&normalized, task_keywords)
    } else if auto_seed_file_stem_matches(file, "express")
        || auto_seed_file_stem_matches(file, "gin")
        || auto_seed_file_stem_matches(file, "scaffold")
        || auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "routing")
        || auto_seed_file_stem_matches(file, "resolver")
        || auto_seed_file_stem_matches(file, "resolvers")
    {
        4
    } else if auto_seed_file_stem_matches(file, "router")
        || auto_seed_file_stem_matches(file, "route")
        || auto_seed_file_stem_matches(file, "routes")
        || auto_seed_file_stem_matches(file, "url")
        || auto_seed_file_stem_matches(file, "urls")
        || auto_seed_file_stem_matches(file, "tree")
    {
        3
    } else if auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "app")
    {
        2
    } else if normalized.contains("/router/") || normalized.contains("/routing/") {
        1
    } else {
        0
    }
}

fn auto_seed_task_named_package_source_file_priority(
    normalized_file: &str,
    task_keywords: &[String],
) -> i32 {
    if auto_seed_task_named_package_source_root(normalized_file, task_keywords).is_some() {
        5
    } else {
        0
    }
}

fn auto_seed_task_named_package_source_root(
    normalized_file: &str,
    task_keywords: &[String],
) -> Option<String> {
    if !normalized_file.contains("/src/") {
        return None;
    }

    let segments = normalized_file.split('/').collect::<Vec<_>>();
    for window in segments.windows(3) {
        let [parent, package, source] = window else {
            continue;
        };
        if !matches!(*parent, "packages" | "crates" | "libs" | "modules") || *source != "src" {
            continue;
        }
        if task_keywords.iter().any(|keyword| {
            keyword.len() >= 3
                && auto_seed_package_name_keyword_allowed(keyword)
                && keyword.eq_ignore_ascii_case(package)
        }) {
            return Some(format!("{parent}/{package}"));
        }
    }

    None
}

fn auto_seed_companion_entrypoint_allowed(
    seed_file: &str,
    entrypoint_file: &str,
    task_keywords: &[String],
) -> bool {
    let seed_normalized = seed_file.replace('\\', "/").to_ascii_lowercase();
    let Some(seed_package_root) =
        auto_seed_task_named_package_source_root(&seed_normalized, task_keywords)
    else {
        return true;
    };
    let entrypoint_normalized = entrypoint_file.replace('\\', "/").to_ascii_lowercase();
    auto_seed_workspace_package_root(&entrypoint_normalized)
        .as_deref()
        .is_some_and(|entrypoint_package_root| entrypoint_package_root == seed_package_root)
}

fn auto_seed_workspace_package_root(normalized_file: &str) -> Option<String> {
    let segments = normalized_file.split('/').collect::<Vec<_>>();
    for window in segments.windows(2) {
        let [parent, package] = window else {
            continue;
        };
        if matches!(*parent, "packages" | "crates" | "libs" | "modules") {
            return Some(format!("{parent}/{package}"));
        }
    }
    None
}

fn auto_seed_route_dispatch_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("resolver")
        || has_exact("resolvers")
        || has_exact("resolve")
        || has_exact("resolveerror")
        || has_exact("urlresolver")
        || has_exact("urlpattern")
        || has_exact("route")
        || has_exact("router")
        || has_exact("routing")
        || has_exact("dispatch")
        || has_exact("match")
        || has_exact("matcher")
        || has_exact("handle")
        || has_exact("handler")
        || has_exact("register")
        || (has_exact("add") && has_exact("url") && has_exact("rule"))
        || matches!(
            normalized.as_str(),
            "urlresolver"
                | "urlpattern"
                | "resolvermatch"
                | "resolve"
                | "check"
                | "match"
                | "matcher"
                | "dispatchrequest"
                | "fulldispatchrequest"
                | "addurlrule"
                | "handle"
                | "handler"
        )
}

fn auto_seed_response_redirect_task(task_keywords: &[String]) -> bool {
    let redirect_task = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "redirect" | "redirects" | "redirection"));
    let response_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "response" | "responses" | "status" | "location" | "handler" | "server"
        )
    });
    let client_transport_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "client"
                | "network"
                | "proxy"
                | "proxies"
                | "adapter"
                | "adapters"
                | "transport"
                | "transports"
        )
    });

    redirect_task && response_task && !client_transport_task
}

fn auto_seed_response_redirect_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "response")
        || auto_seed_file_stem_matches(file, "responses")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
        || auto_seed_file_stem_matches(file, "redirect")
        || auto_seed_file_stem_matches(file, "redirects")
        || file
            .split('/')
            .any(|part| part.eq_ignore_ascii_case("render"))
}

fn auto_seed_response_redirect_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "response")
        || auto_seed_file_stem_matches(file, "responses")
        || auto_seed_file_stem_matches(file, "context")
        || auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
}

fn auto_seed_response_redirect_file_priority(file: &str) -> i32 {
    auto_seed_framework_action_file_priority(
        file,
        auto_seed_response_redirect_framework_file_matches(file),
        auto_seed_response_redirect_file_matches(file),
    )
}

fn auto_seed_http_operation_file_priority(file: &str, task_keywords: &[String]) -> i32 {
    let mut priority = 0;
    if auto_seed_request_body_parsing_task(task_keywords) {
        priority = priority.max(auto_seed_request_body_parsing_file_priority(file));
    }
    if auto_seed_response_headers_task(task_keywords) {
        priority = priority.max(auto_seed_response_headers_file_priority(file));
    }
    if auto_seed_response_cookies_task(task_keywords) {
        priority = priority.max(auto_seed_response_cookies_file_priority(file));
    }
    if auto_seed_response_redirect_task(task_keywords) {
        priority = priority.max(auto_seed_response_redirect_file_priority(file));
    }
    priority
}

fn auto_seed_priority_routed_file_priority(file: &str, task_keywords: &[String]) -> i32 {
    let mut priority = auto_seed_http_operation_file_priority(file, task_keywords);
    if auto_seed_agent_first_read_task(task_keywords)
        && !auto_seed_agent_first_read_evidence_task(task_keywords)
    {
        priority = priority.max(auto_seed_agent_first_read_file_priority(
            file,
            task_keywords,
        ));
    }
    if auto_seed_indexing_pipeline_task(task_keywords) {
        priority = priority.max(auto_seed_indexing_pipeline_file_priority(file));
    }
    if auto_seed_data_persistence_task(task_keywords) {
        priority = priority.max(auto_seed_data_persistence_file_priority(file));
    }
    if auto_seed_semantic_context_task(task_keywords) {
        priority = priority.max(auto_seed_semantic_context_file_priority(
            file,
            auto_seed_semantic_context_prefers_orchestration(task_keywords),
        ));
    }
    if auto_seed_dependency_graph_task(task_keywords) {
        priority = priority.max(auto_seed_dependency_graph_file_priority(file));
    }
    if auto_seed_project_overview_task(task_keywords) {
        priority = priority.max(auto_seed_project_overview_file_priority(file));
    }
    if auto_seed_symbol_search_task(task_keywords) {
        priority = priority.max(auto_seed_symbol_search_file_priority(file));
    }
    if auto_seed_reference_search_task(task_keywords)
        || auto_seed_call_graph_traversal_task(task_keywords)
    {
        priority = priority.max(auto_seed_tool_analysis_file_priority(file));
    }
    if auto_seed_file_parsing_task(task_keywords) {
        priority = priority.max(auto_seed_file_parsing_file_priority(file));
    }
    if auto_seed_binding_validation_task(task_keywords) {
        priority = priority.max(auto_seed_binding_validation_file_priority_for_task(
            file,
            task_keywords,
        ));
    }
    if auto_seed_import_resolution_task(task_keywords) {
        priority = priority.max(auto_seed_import_resolution_file_priority(file));
    }
    if auto_seed_request_lifecycle_task(task_keywords) {
        priority = priority.max(auto_seed_request_lifecycle_file_priority(file));
    }
    if auto_seed_middleware_task(task_keywords) {
        priority = priority.max(auto_seed_middleware_file_priority(file));
    }
    if auto_seed_route_dispatch_task(task_keywords) {
        priority = priority.max(auto_seed_route_dispatch_file_priority(file, task_keywords));
    }
    priority
}

fn auto_seed_indexing_pipeline_task(task_keywords: &[String]) -> bool {
    let indexing = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "index" | "indexing" | "indexer" | "indexed"
        )
    });
    let pipeline = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "pipeline" | "project" | "source" | "sources" | "parse" | "parser" | "scan" | "scanner"
        )
    });

    indexing && pipeline
}

fn auto_seed_indexing_pipeline_file_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs" || normalized.starts_with("docs/") || is_low_value_reference_file(file)
    {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);

    let mut priority = if source_file { 20 } else { 0 };
    if matches!(stem, "index" | "indexer") {
        priority = priority.max(150);
    }
    if auto_seed_field_matches(file, "parser")
        || auto_seed_field_matches(file, "parse")
        || auto_seed_field_matches(file, "scanner")
        || auto_seed_field_matches(file, "scan")
    {
        priority = priority.max(110);
    }
    if auto_seed_field_matches(file, "storage") || auto_seed_field_matches(file, "store") {
        priority = priority.max(90);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_data_persistence_task(task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "database"
                | "db"
                | "persistence"
                | "persist"
                | "storage"
                | "store"
                | "repository"
                | "migration"
                | "migrations"
                | "migrate"
                | "sql"
        )
    })
}

fn auto_seed_data_persistence_file_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs" || normalized.starts_with("docs/") || is_low_value_reference_file(file)
    {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);

    let mut priority = if source_file { 20 } else { 0 };
    if matches!(
        stem,
        "storage" | "store" | "database" | "db" | "repository" | "repo"
    ) {
        priority = priority.max(150);
    }
    if auto_seed_field_matches(file, "migration")
        || auto_seed_field_matches(file, "migrations")
        || auto_seed_field_matches(file, "migrate")
        || auto_seed_field_matches(file, "schema")
    {
        priority = priority.max(120);
    }
    if auto_seed_field_matches(file, "model") || auto_seed_field_matches(file, "entity") {
        priority = priority.max(80);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_semantic_context_task(task_keywords: &[String]) -> bool {
    let semantic = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "semantic" | "embedding" | "embeddings" | "vector" | "vectors"
        )
    });
    let context = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "search"
                | "index"
                | "indexing"
                | "fallback"
                | "chunk"
                | "chunks"
                | "provider"
                | "providers"
        )
    });

    semantic && context
}

fn auto_seed_semantic_context_prefers_orchestration(task_keywords: &[String]) -> bool {
    let orchestration = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "search" | "fallback" | "chunk" | "chunks" | "index" | "indexing"
        )
    });
    let provider_configuration = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "provider" | "providers" | "configuration" | "config" | "status"
        )
    });

    orchestration && !provider_configuration
}

fn auto_seed_embedding_provider_status_task(task_keywords: &[String]) -> bool {
    let embedding_provider = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "embedding" | "embeddings" | "provider" | "providers"
        )
    });
    let status_reporting = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "status" | "report" | "reports" | "reporting" | "diagnostic" | "diagnostics"
        )
    });
    let configuration = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "config" | "configuration" | "setting" | "settings"
        )
    });

    embedding_provider && status_reporting && !configuration
}

fn auto_seed_semantic_provider_fallback_task(task_keywords: &[String]) -> bool {
    let semantic = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "semantic"));
    let provider = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "provider" | "providers"));
    let fallback_or_disabled = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "disabled"
                | "disable"
                | "fallback"
                | "fallbacks"
                | "unavailable"
                | "missing"
                | "unconfigured"
                | "readiness"
                | "ready"
        )
    });

    semantic && provider && fallback_or_disabled
}

fn auto_seed_blocked_no_seed_route_task(task_keywords: &[String]) -> bool {
    let blocked = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "blocked" | "block" | "empty" | "missing" | "no"
        )
    });
    let seed = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "seed" | "seeds"));
    let routing = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "route" | "routes" | "router" | "routing" | "mcp" | "agent"
        )
    });

    blocked && seed && routing
}

fn auto_seed_recommended_next_tools_contract_task(task_keywords: &[String]) -> bool {
    let recommendation = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "recommended" | "recommendation" | "recommend" | "next"
        )
    });
    let tools = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "tool" | "tools"));
    let contract = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "contract" | "contracts" | "priority" | "priorities" | "argument" | "arguments"
        )
    });

    recommendation && tools && contract
}

fn auto_seed_project_entrypoint_ranking_task(task_keywords: &[String]) -> bool {
    let overview = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "overview" | "project"));
    let entrypoint = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "entrypoint" | "entrypoints" | "entry" | "entries"
        )
    });
    let ranking = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "rank" | "ranking" | "ranked" | "score" | "scoring" | "priority"
        )
    });

    overview && entrypoint && ranking
}

fn auto_seed_budget_continuation_task(task_keywords: &[String]) -> bool {
    let budget = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "budget" | "budgets" | "token" | "tokens" | "context"
        )
    });
    let continuation = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "continuation"
                | "continue"
                | "omitted"
                | "candidate"
                | "candidates"
                | "truncated"
                | "exhausted"
        )
    });

    budget && continuation
}

fn auto_seed_impact_suggested_checks_task(task_keywords: &[String]) -> bool {
    let impact = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "impact" | "impacted"));
    let checks = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "check" | "checks" | "suggested" | "command" | "commands" | "test" | "tests"
        )
    });

    impact && checks
}

fn auto_seed_mcp_tool_schema_validation_task(task_keywords: &[String]) -> bool {
    let mcp_tool = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "mcp"))
        && task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "tool" | "tools"));
    let schema_validation = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "schema"
                | "schemas"
                | "validation"
                | "validate"
                | "argument"
                | "arguments"
                | "binding"
                | "bind"
        )
    });

    mcp_tool && schema_validation
}

fn auto_seed_config_status_reporting_task(task_keywords: &[String]) -> bool {
    let config = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "config" | "configuration"));
    let status = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "status" | "report" | "reporting"));
    let diagnostics = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "parse" | "parser" | "parsing" | "error" | "errors" | "diagnostic" | "diagnostics"
        )
    });

    config && status && diagnostics
}

fn auto_seed_semantic_index_explain_task(task_keywords: &[String]) -> bool {
    let semantic_index = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "semantic"))
        && task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "index" | "indexing" | "indexed"));
    let explain = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "explain" | "explains" | "explanation" | "output" | "report"
        )
    });

    semantic_index && explain
}

fn auto_seed_semantic_context_file_priority(file: &str, prefer_orchestration: bool) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs" || normalized.starts_with("docs/") || is_low_value_reference_file(file)
    {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);

    let mut priority = if source_file { 20 } else { 0 };
    if prefer_orchestration && matches!(stem, "tools" | "tool") {
        priority = priority.max(170);
    }
    if matches!(stem, "embedding" | "embeddings" | "semantic") {
        priority = priority.max(150);
    }
    if !prefer_orchestration && matches!(stem, "tools" | "tool") {
        priority = priority.max(120);
    }
    if matches!(stem, "storage" | "store" | "database" | "db") {
        priority = priority.max(105);
    }
    if matches!(stem, "index" | "indexer") {
        priority = priority.max(80);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_dependency_graph_task(task_keywords: &[String]) -> bool {
    let dependency = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "dependency" | "dependencies"));
    let graph = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "graph"
                | "graphs"
                | "generation"
                | "generate"
                | "generator"
                | "extract"
                | "extraction"
                | "import"
                | "imports"
                | "edge"
                | "edges"
        )
    });

    dependency && graph
}

fn auto_seed_dependency_graph_file_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs" || normalized.starts_with("docs/") || is_low_value_reference_file(file)
    {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);

    let mut priority = if source_file { 20 } else { 0 };
    if matches!(stem, "index" | "indexer") {
        priority = priority.max(170);
    }
    if matches!(stem, "storage" | "store" | "database" | "db") {
        priority = priority.max(145);
    }
    if matches!(stem, "tools" | "tool") {
        priority = priority.max(125);
    }
    if matches!(stem, "mcp") {
        priority = priority.max(55);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_project_overview_task(task_keywords: &[String]) -> bool {
    let overview = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "overview" | "summary"));
    let project_entrypoint = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "project"
                | "repository"
                | "repo"
                | "entrypoint"
                | "entrypoints"
                | "detect"
                | "detection"
                | "candidate"
                | "candidates"
        )
    });

    overview && project_entrypoint
}

fn auto_seed_project_overview_file_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs" || normalized.starts_with("docs/") || is_low_value_reference_file(file)
    {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);

    let mut priority = if source_file { 20 } else { 0 };
    if matches!(stem, "storage" | "store" | "database" | "db") {
        priority = priority.max(180);
    }
    if matches!(stem, "tools" | "tool") {
        priority = priority.max(130);
    }
    if matches!(stem, "model" | "models") {
        priority = priority.max(110);
    }
    if matches!(stem, "mcp") {
        priority = priority.max(35);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_symbol_search_task(task_keywords: &[String]) -> bool {
    let symbol = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "symbol" | "symbols"));
    let search = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "search" | "lookup" | "find"));

    symbol && search
}

fn auto_seed_reference_search_task(task_keywords: &[String]) -> bool {
    let reference = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "reference" | "references" | "usage" | "usages"
        )
    });
    let search = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "find"
                | "search"
                | "lookup"
                | "classification"
                | "classify"
                | "classified"
                | "filter"
                | "filtering"
        )
    });

    reference && search
}

fn auto_seed_call_graph_traversal_task(task_keywords: &[String]) -> bool {
    if task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "impact" | "impacted"))
    {
        return false;
    }

    let call_graph = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "call" | "calls" | "caller" | "callers" | "callee" | "callees" | "graph" | "graphs"
        )
    });
    let traversal = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "traversal"
                | "traverse"
                | "traversed"
                | "path"
                | "paths"
                | "caller"
                | "callers"
                | "callee"
                | "callees"
        )
    });

    call_graph && traversal
}

fn auto_seed_tool_analysis_file_priority(file: &str) -> i32 {
    auto_seed_symbol_search_file_priority(file)
}

fn auto_seed_symbol_search_file_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs" || normalized.starts_with("docs/") || is_low_value_reference_file(file)
    {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);

    let mut priority = if source_file { 20 } else { 0 };
    if matches!(stem, "tools" | "tool") {
        priority = priority.max(180);
    }
    if matches!(stem, "storage" | "store" | "database" | "db") {
        priority = priority.max(150);
    }
    if matches!(stem, "index" | "indexer") {
        priority = priority.max(80);
    }
    if matches!(stem, "mcp") {
        priority = priority.max(60);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_file_parsing_task(task_keywords: &[String]) -> bool {
    let parsing = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "parse" | "parser" | "parsing" | "deserialize" | "deserialization"
        )
    });
    let language = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "language" | "languages" | "support" | "supported"
        )
    });

    parsing && language
}

fn auto_seed_file_parsing_file_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs" || normalized.starts_with("docs/") || is_low_value_reference_file(file)
    {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);

    let mut priority = if source_file { 20 } else { 0 };
    if matches!(stem, "index" | "indexer") {
        priority = priority.max(180);
    }
    if matches!(stem, "language" | "languages") {
        priority = priority.max(150);
    }
    if matches!(stem, "model" | "models") {
        priority = priority.max(90);
    }
    if matches!(stem, "tools" | "tool") {
        priority = priority.max(70);
    }
    if matches!(stem, "storage" | "store" | "database" | "db") {
        priority = priority.max(30);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_binding_validation_task(task_keywords: &[String]) -> bool {
    let validation = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "validation" | "validate" | "validator" | "schema" | "schemas"
        )
    });
    let binding = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "binding" | "bindings" | "bind" | "json"));

    validation && binding
}

fn auto_seed_binding_validation_file_priority_for_task(
    file: &str,
    task_keywords: &[String],
) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs"
        || normalized.starts_with("docs/")
        || normalized == "docs_src"
        || normalized.starts_with("docs_src/")
        || normalized == "examples"
        || normalized.starts_with("examples/")
    {
        return -30;
    }

    let low_value = is_low_value_reference_file(file);
    let coverage_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "test" | "tests" | "testing" | "spec" | "specs" | "coverage" | "regression"
        )
    });
    if low_value && !coverage_task {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);
    let json_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "json" | "serialize" | "serialization" | "deserialize" | "deserialization"
        )
    });
    let validation_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "validation" | "validate" | "validator" | "validators" | "schema" | "schemas"
        )
    });

    let mut priority = if source_file { 20 } else { 0 };
    if low_value && coverage_task {
        priority = priority.max(220);
    }
    if json_task && auto_seed_field_matches(file, "json") {
        priority = priority.max(if low_value { 290 } else { 260 });
    }
    if validation_task
        && (auto_seed_field_matches(file, "validation")
            || auto_seed_field_matches(file, "validator")
            || auto_seed_field_matches(file, "schema"))
    {
        priority = priority.max(if low_value { 285 } else { 250 });
    }
    if matches!(stem, "validation" | "validator" | "validators" | "schema") {
        priority = priority.max(210);
    }
    if matches!(stem, "binding" | "bindings") {
        priority = priority.max(170);
    }
    if auto_seed_field_matches(file, "binding") || auto_seed_field_matches(file, "bindings") {
        priority = priority.max(160);
    }
    if matches!(stem, "mcp") {
        priority = priority.max(130);
    }
    if matches!(stem, "tools" | "tool") {
        priority = priority.max(90);
    }
    if matches!(stem, "index" | "indexer") {
        priority = priority.max(80);
    }
    if matches!(stem, "storage" | "store" | "database" | "db") {
        priority = priority.max(50);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_import_resolution_task(task_keywords: &[String]) -> bool {
    let import = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "import" | "imports" | "package" | "packages" | "dependency" | "dependencies"
        )
    });
    let resolution = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "resolution" | "resolve" | "resolves" | "resolver" | "resolving"
        )
    });

    import && resolution
}

fn auto_seed_import_resolution_file_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if normalized == "docs" || normalized.starts_with("docs/") || is_low_value_reference_file(file)
    {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let file_name = normalized.rsplit('/').next().unwrap_or(normalized.as_str());
    let stem = file_name.split('.').next().unwrap_or(file_name);

    let mut priority = if source_file { 20 } else { 0 };
    if matches!(stem, "index" | "indexer") {
        priority = priority.max(180);
    }
    if matches!(stem, "storage" | "store" | "database" | "db") {
        priority = priority.max(130);
    }
    if matches!(stem, "tools" | "tool") {
        priority = priority.max(90);
    }
    if matches!(stem, "language" | "languages" | "model" | "models") {
        priority = priority.max(50);
    }
    if source_file && matches!(stem, "main" | "lib" | "mod") {
        priority = priority.max(45);
    }

    priority
}

fn auto_seed_framework_action_file_priority(
    file: &str,
    framework_match: bool,
    action_match: bool,
) -> i32 {
    let normalized = file.to_ascii_lowercase();
    if normalized.contains("/tests/")
        || normalized.starts_with("tests/")
        || normalized.starts_with("docs/")
        || normalized.starts_with("docs_src/")
        || normalized.starts_with("examples/")
    {
        -2
    } else if framework_match {
        3
    } else if action_match {
        2
    } else {
        0
    }
}

fn auto_seed_response_redirect_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("redirect")
        || has_exact("redirects")
        || has_exact("redirection")
        || has_exact("location")
        || (has_exact("status") && has_exact("redirect"))
        || matches!(
            normalized.as_str(),
            "redirect"
                | "redirects"
                | "redirectrequest"
                | "redirectresponse"
                | "redirecttrailingslash"
                | "redirectfixedpath"
        )
}

fn auto_seed_static_file_serving_task(task_keywords: &[String]) -> bool {
    let static_or_asset = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "static" | "asset" | "assets"));
    let file_or_serving = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "file"
                | "files"
                | "folder"
                | "folders"
                | "directory"
                | "directories"
                | "serve"
                | "serving"
                | "filesystem"
                | "sendfile"
        )
    });
    let direct_file_response = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "sendfile" | "send_file"));

    (static_or_asset && file_or_serving) || direct_file_response
}

fn auto_seed_static_file_serving_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "express")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "blueprint")
        || auto_seed_file_stem_matches(file, "blueprints")
        || auto_seed_file_stem_matches(file, "response")
        || auto_seed_file_stem_matches(file, "responses")
        || auto_seed_file_stem_matches(file, "helper")
        || auto_seed_file_stem_matches(file, "helpers")
        || file.split('/').any(|part| {
            part.eq_ignore_ascii_case("fs")
                || part.eq_ignore_ascii_case("filesystem")
                || part.eq_ignore_ascii_case("static")
                || part.eq_ignore_ascii_case("assets")
        })
}

fn auto_seed_static_file_serving_framework_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "routergroup")
        || auto_seed_file_stem_matches(file, "express")
        || auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "application")
}

fn auto_seed_static_file_serving_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    let normalized = parts.join("");
    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);

    has_exact("static")
        || has_exact("staticfile")
        || has_exact("staticfs")
        || has_exact("sendfile")
        || has_exact("sendstaticfile")
        || has_exact("sendfromdirectory")
        || has_exact("createstatichandler")
        || has_exact("filesystem")
        || (has_exact("file") && (has_exact("serve") || has_exact("send")))
        || matches!(
            normalized.as_str(),
            "staticfile"
                | "staticfilefs"
                | "staticfs"
                | "sendfile"
                | "sendstaticfile"
                | "sendfromdirectory"
                | "createstatichandler"
        )
}

fn auto_seed_response_rendering_task(task_keywords: &[String]) -> bool {
    let render_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "render" | "renders" | "renderer" | "renderers" | "rendering"
        )
    });
    let backend_response_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "response"
                | "responses"
                | "http"
                | "handler"
                | "output"
                | "outputs"
                | "template"
                | "templates"
        )
    });
    let frontend_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "frontend"
                | "ui"
                | "component"
                | "components"
                | "page"
                | "pages"
                | "screen"
                | "screens"
        )
    });

    render_task && backend_response_task && !frontend_task
}

fn auto_seed_response_rendering_file_matches(file: &str) -> bool {
    auto_seed_response_rendering_central_file_matches(file)
        || file.split('/').any(|part| {
            part.eq_ignore_ascii_case("render") || part.eq_ignore_ascii_case("rendering")
        })
}

fn auto_seed_response_rendering_central_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "render")
        || auto_seed_file_stem_matches(file, "renderer")
        || auto_seed_file_stem_matches(file, "rendering")
}

fn auto_seed_response_rendering_response_file_matches(
    file: &str,
    task_keywords: &[String],
) -> bool {
    task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "response" | "responses"))
        && (auto_seed_file_stem_matches(file, "response")
            || auto_seed_file_stem_matches(file, "responses"))
}

fn auto_seed_response_rendering_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);
    has_exact("render")
        || has_exact("renders")
        || has_exact("renderer")
        || has_exact("renderers")
        || has_exact("rendering")
}

fn auto_seed_request_lifecycle_task(task_keywords: &[String]) -> bool {
    if auto_seed_response_rendering_task(task_keywords) {
        return false;
    }

    let request_or_response = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "request" | "requests" | "response" | "responses"
        )
    });
    let lifecycle = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "lifecycle" | "before" | "after" | "dispatch" | "handling" | "handle"
        )
    });
    let handler_chain = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "handler" | "handlers"))
        && task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "context" | "chain" | "chains"));

    request_or_response && (lifecycle || handler_chain)
}

fn auto_seed_request_lifecycle_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "base")
        || auto_seed_file_stem_matches(file, "handler")
        || auto_seed_file_stem_matches(file, "handlers")
        || file.to_ascii_lowercase().contains("/handlers/")
}

fn auto_seed_request_lifecycle_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);
    (has_exact("request") || has_exact("response"))
        && (has_exact("dispatch")
            || has_exact("preprocess")
            || has_exact("process")
            || has_exact("finalize")
            || has_exact("teardown"))
}

fn auto_seed_request_lifecycle_file_priority(file: &str) -> i32 {
    let normalized = file.to_ascii_lowercase();
    if normalized.contains("/tests/")
        || normalized.starts_with("tests/")
        || normalized.starts_with("docs/")
        || normalized.starts_with("docs_src/")
        || normalized.starts_with("examples/")
    {
        -2
    } else if normalized.contains("/core/handlers/base.") || normalized.contains("/basehandler.") {
        5
    } else if auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "base")
    {
        4
    } else if normalized.contains("/handlers/")
        || auto_seed_file_stem_matches(file, "handler")
        || auto_seed_file_stem_matches(file, "handlers")
    {
        3
    } else if auto_seed_request_lifecycle_file_matches(file) {
        1
    } else {
        0
    }
}

fn auto_seed_middleware_task(task_keywords: &[String]) -> bool {
    task_keywords
        .iter()
        .any(|keyword| keyword == "middleware" || keyword == "middlewares")
}

fn auto_seed_middleware_file_priority(file: &str) -> i32 {
    let normalized = file.to_ascii_lowercase();
    if normalized.contains("/tests/")
        || normalized.starts_with("tests/")
        || normalized.starts_with("docs/")
        || normalized.starts_with("docs_src/")
        || normalized.starts_with("examples/")
    {
        -2
    } else if normalized.contains("/core/handlers/base.") {
        5
    } else if auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "applications")
        || normalized.contains("/core/handlers/")
        || normalized.contains("/basehandler.")
        || auto_seed_file_stem_matches(file, "handler")
        || auto_seed_file_stem_matches(file, "handlers")
    {
        4
    } else if normalized.starts_with("django/middleware/")
        || normalized.contains("/middleware/")
        || normalized.contains("/middlewares/")
        || auto_seed_file_stem_matches(file, "middleware")
        || auto_seed_file_stem_matches(file, "middlewares")
    {
        3
    } else if normalized.contains("/handlers/") || auto_seed_file_stem_matches(file, "base") {
        2
    } else {
        0
    }
}

fn auto_seed_runtime_lifecycle_task(task_keywords: &[String]) -> bool {
    let runtime = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "script" | "runner" | "runtime" | "execution"
        )
    });
    let lifecycle = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "lifecycle" | "rerun" | "reruns" | "shutdown"
        )
    });
    runtime && lifecycle
}

fn auto_seed_runtime_lifecycle_field_matches(field: &str) -> bool {
    auto_seed_field_matches(field, "script")
        || auto_seed_field_matches(field, "runner")
        || auto_seed_field_matches(field, "scriptrunner")
        || auto_seed_field_matches(field, "runtime")
        || auto_seed_field_matches(field, "rerun")
        || auto_seed_field_matches(field, "lifecycle")
}

fn auto_seed_file_upload_task(task_keywords: &[String]) -> bool {
    let upload = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "upload" | "uploaded" | "uploader" | "uploads"
        )
    });
    let file = task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "file" | "files" | "manager" | "storage"));
    upload && file
}

fn auto_seed_file_upload_field_matches(field: &str) -> bool {
    auto_seed_field_matches(field, "upload")
        || auto_seed_field_matches(field, "uploaded")
        || auto_seed_field_matches(field, "uploader")
        || auto_seed_field_matches(field, "uploadedfile")
}

fn auto_seed_websocket_connection_task(task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "websocket" | "websockets" | "socket" | "sockets"
        )
    })
}

fn auto_seed_websocket_connection_field_matches(field: &str) -> bool {
    auto_seed_field_matches(field, "websocket")
        || auto_seed_field_matches(field, "websockets")
        || auto_seed_field_matches(field, "socket")
}

fn auto_seed_websocket_file_priority(file: &str) -> i32 {
    let normalized = file.to_ascii_lowercase();
    if normalized.starts_with("docs/")
        || normalized.starts_with("docs_src/")
        || normalized.starts_with("examples/")
        || normalized.contains("/tutorial")
    {
        -2
    } else if auto_seed_file_stem_matches(file, "websocket")
        || auto_seed_file_stem_matches(file, "websockets")
    {
        3
    } else if auto_seed_websocket_connection_field_matches(file) {
        1
    } else {
        0
    }
}

fn auto_seed_error_recovery_handling_task(task_keywords: &[String]) -> bool {
    let error_or_recovery = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "retry"
                | "retries"
                | "timeout"
                | "timeouts"
                | "error"
                | "errors"
                | "exception"
                | "exceptions"
                | "failure"
                | "failures"
                | "recovery"
                | "recover"
                | "panic"
                | "panics"
        )
    });
    let handling = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "handling"
                | "handle"
                | "debug"
                | "error"
                | "errors"
                | "failure"
                | "failures"
                | "recovery"
                | "recover"
                | "middleware"
                | "handler"
                | "handlers"
        )
    });

    error_or_recovery && handling
}

fn auto_seed_error_recovery_action_file_matches(file: &str, task_keywords: &[String]) -> bool {
    let retry_timeout_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "retry" | "retries" | "timeout" | "timeouts"
        )
    });
    let retry_timeout_file_match = auto_seed_file_stem_matches(file, "adapter")
        || auto_seed_file_stem_matches(file, "adapters")
        || auto_seed_file_stem_matches(file, "transport")
        || auto_seed_file_stem_matches(file, "transports")
        || auto_seed_file_stem_matches(file, "client")
        || auto_seed_file_stem_matches(file, "clients")
        || auto_seed_file_stem_matches(file, "session")
        || auto_seed_file_stem_matches(file, "sessions")
        || auto_seed_file_stem_matches(file, "request")
        || auto_seed_file_stem_matches(file, "requests");

    if retry_timeout_task {
        return retry_timeout_file_match;
    }

    auto_seed_file_stem_matches(file, "app")
        || auto_seed_file_stem_matches(file, "application")
        || auto_seed_file_stem_matches(file, "error")
        || auto_seed_file_stem_matches(file, "errors")
        || auto_seed_file_stem_matches(file, "exception")
        || auto_seed_file_stem_matches(file, "exceptions")
        || auto_seed_file_stem_matches(file, "recovery")
        || auto_seed_file_stem_matches(file, "recover")
        || retry_timeout_file_match
}

fn auto_seed_error_recovery_recovery_file_matches(file: &str, task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "recovery" | "recover" | "panic" | "panics"
        )
    }) && (auto_seed_file_stem_matches(file, "recovery")
        || auto_seed_file_stem_matches(file, "recover"))
}

fn auto_seed_error_recovery_application_file_matches(file: &str, task_keywords: &[String]) -> bool {
    let retry_timeout_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "retry" | "retries" | "timeout" | "timeouts"
        )
    });
    let error_handling_task = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "error" | "errors" | "exception" | "exceptions" | "handling" | "handler" | "handlers"
        )
    });

    !retry_timeout_task
        && error_handling_task
        && (auto_seed_file_stem_matches(file, "app")
            || auto_seed_file_stem_matches(file, "application"))
}

fn auto_seed_error_recovery_action_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);
    has_exact("send")
        || has_exact("request")
        || has_exact("error")
        || has_exact("errors")
        || has_exact("exception")
        || has_exact("exceptions")
        || has_exact("recovery")
        || has_exact("recover")
        || has_exact("panic")
        || has_exact("panics")
        || has_exact("finalhandler")
        || has_exact("onerror")
        || has_exact("logerror")
        || (has_exact("handle") && (has_exact("error") || has_exact("exception")))
        || has_exact("adapter")
        || has_exact("transport")
        || has_exact("client")
        || has_exact("connection")
        || (has_exact("get") && has_exact("connection"))
}

fn auto_seed_tls_certificate_task(task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "tls"
                | "ssl"
                | "certificate"
                | "certificates"
                | "cert"
                | "certs"
                | "verify"
                | "verification"
                | "trust"
                | "trusted"
                | "truststore"
        )
    })
}

fn auto_seed_tls_certificate_action_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "adapter")
        || auto_seed_file_stem_matches(file, "adapters")
        || auto_seed_file_stem_matches(file, "transport")
        || auto_seed_file_stem_matches(file, "transports")
        || auto_seed_file_stem_matches(file, "client")
        || auto_seed_file_stem_matches(file, "clients")
        || auto_seed_file_stem_matches(file, "connection")
        || auto_seed_file_stem_matches(file, "connections")
        || auto_seed_file_stem_matches(file, "session")
        || auto_seed_file_stem_matches(file, "sessions")
}

fn auto_seed_tls_certificate_action_symbol_matches(symbol: &str) -> bool {
    let parts = symbol
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);
    has_exact("cert")
        || has_exact("certificate")
        || has_exact("verify")
        || has_exact("tls")
        || has_exact("ssl")
        || has_exact("adapter")
        || has_exact("transport")
        || has_exact("connection")
}

fn auto_seed_route_registration_matches(field: &str) -> bool {
    let parts = field
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();

    let has_exact = |needle: &str| parts.iter().any(|part| part == needle);
    has_exact("route")
        || has_exact("routes")
        || has_exact("router")
        || has_exact("urls")
        || has_exact("routergroup")
        || has_exact("iroutes")
        || (has_exact("add") && has_exact("url") && has_exact("rule"))
}

fn auto_seed_entrypoint_file_matches(file: &str) -> bool {
    auto_seed_field_matches(file, "bootstrap")
        || auto_seed_field_matches(file, "main")
        || auto_seed_field_matches(file, "cli")
        || auto_seed_file_stem_matches(file, "__init__")
}

fn auto_seed_file_stem_matches(file: &str, keyword: &str) -> bool {
    Path::new(file)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| stem.eq_ignore_ascii_case(keyword))
        .unwrap_or(false)
}

fn auto_seed_lifecycle_keyword(keyword: &str) -> bool {
    matches!(
        keyword,
        "startup" | "start" | "boot" | "program" | "entrypoint" | "entrypoints" | "main"
    )
}

fn auto_seed_startup_entrypoint_task(task_keywords: &[String]) -> bool {
    task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "entrypoint" | "entrypoints" | "entry" | "entries" | "main"
        )
    }) && !task_keywords
        .iter()
        .any(|keyword| matches!(keyword.as_str(), "package" | "packages"))
}

fn auto_seed_startup_flow_task(task_keywords: &[String]) -> bool {
    !auto_seed_startup_entrypoint_task(task_keywords)
        && task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "startup" | "start" | "boot" | "bootstrap"))
        && !task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "package" | "packages"))
}

fn auto_seed_startup_entrypoint_file_priority(file: &str, task_keywords: &[String]) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if is_low_value_reference_file(file) {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    let entrypoint_file = auto_seed_entrypoint_file_matches(file);
    let rust_task = task_keywords
        .iter()
        .any(|keyword| keyword == "rust" || keyword == "rs");
    let rust_file = normalized.ends_with(".rs");

    let mut score = if source_file && auto_seed_file_stem_matches(file, "main") {
        160
    } else if source_file && entrypoint_file {
        130
    } else if entrypoint_file {
        90
    } else if source_file {
        20
    } else {
        0
    };

    if rust_task {
        score += if rust_file { 40 } else { -20 };
    }

    score
}

fn auto_seed_startup_flow_file_priority(file: &str) -> i32 {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    if normalized == "scripts" || normalized.starts_with("scripts/") {
        return -80;
    }
    if is_low_value_reference_file(file) {
        return -30;
    }

    let source_file = normalized.starts_with("src/") || normalized.contains("/src/");
    if source_file
        && (auto_seed_file_stem_matches(file, "startup")
            || auto_seed_file_stem_matches(file, "bootstrap")
            || auto_seed_file_stem_matches(file, "boot"))
    {
        160
    } else if auto_seed_file_stem_matches(file, "startup")
        || auto_seed_file_stem_matches(file, "bootstrap")
        || auto_seed_file_stem_matches(file, "boot")
    {
        120
    } else if source_file && auto_seed_entrypoint_file_matches(file) {
        80
    } else if source_file {
        20
    } else {
        0
    }
}

fn auto_seed_prefers_entrypoint(task_keywords: &[String]) -> bool {
    (auto_seed_startup_entrypoint_task(task_keywords)
        || auto_seed_request_lifecycle_task(task_keywords))
        && !task_keywords
            .iter()
            .any(|keyword| matches!(keyword.as_str(), "package" | "packages"))
}

#[derive(Debug, Default)]
struct AutoSeedTextMatch {
    score: i32,
    matched_keywords: Vec<String>,
}

fn auto_seed_file_text_match(
    root: &Path,
    file: &str,
    task_keywords: &[String],
) -> AutoSeedTextMatch {
    if task_keywords.is_empty() {
        return AutoSeedTextMatch::default();
    }

    let Ok(source) = fs::read_to_string(root.join(file)) else {
        return AutoSeedTextMatch::default();
    };
    let searchable = source
        .lines()
        .take(AUTO_SEED_TEXT_SCAN_LINES)
        .collect::<Vec<_>>()
        .join("\n");
    let matched_keywords = task_keywords
        .iter()
        .filter(|keyword| auto_seed_text_keyword_allowed(keyword))
        .filter(|keyword| auto_seed_text_matches(&searchable, keyword))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    AutoSeedTextMatch {
        score: if matched_keywords.is_empty() {
            0
        } else {
            80 + (matched_keywords.len() as i32 * 8)
        },
        matched_keywords,
    }
}

fn auto_seed_field_matches(field: &str, keyword: &str) -> bool {
    auto_seed_field_match_weight(field, keyword) > 0
}

fn auto_seed_field_match_weight(field: &str, keyword: &str) -> i32 {
    field
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .map(|part| {
            if part == keyword {
                3
            } else if keyword.len() >= 4 && part.contains(keyword) {
                1
            } else {
                0
            }
        })
        .max()
        .unwrap_or(0)
}

fn auto_seed_text_matches(text: &str, keyword: &str) -> bool {
    text.split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .any(|part| {
            part == keyword
                || (keyword.len() >= 4 && part.contains(keyword))
                || (part.len() >= 4 && keyword.contains(&part))
        })
}

fn auto_seed_text_keyword_allowed(keyword: &str) -> bool {
    !matches!(
        keyword,
        "app"
            | "application"
            | "entrypoint"
            | "entrypoints"
            | "main"
            | "startup"
            | "start"
            | "boot"
            | "program"
    )
}

fn auto_seed_package_name_keyword_allowed(keyword: &str) -> bool {
    keyword.len() >= 3
        && !matches!(
            keyword,
            "app"
                | "application"
                | "behavior"
                | "dispatch"
                | "flow"
                | "handler"
                | "handlers"
                | "main"
                | "match"
                | "matching"
                | "route"
                | "router"
                | "routes"
                | "routing"
                | "start"
                | "startup"
                | "understand"
        )
}

fn explicit_context_seeds(
    seed_symbols: &[String],
    seed_files: &[String],
    task_path_locations: &TaskPathLocations,
) -> Vec<ContextSeed> {
    let mut seeds = seed_symbols
        .iter()
        .map(|symbol| ContextSeed {
            kind: "symbol".to_string(),
            value: symbol.clone(),
            source: "explicit".to_string(),
            start_line: None,
            end_line: None,
            locations: Vec::new(),
            role: None,
            matched_keywords: Vec::new(),
            matched_symbols: Vec::new(),
        })
        .collect::<Vec<_>>();
    seeds.extend(seed_files.iter().map(|file| {
        ContextSeed {
            kind: "file".to_string(),
            value: file.clone(),
            source: "explicit".to_string(),
            start_line: task_path_locations
                .get(file)
                .and_then(|locations| locations.first())
                .map(|location| location.start_line),
            end_line: task_path_locations
                .get(file)
                .and_then(|locations| locations.first())
                .map(|location| location.end_line),
            locations: context_seed_locations(task_path_locations.get(file).map(Vec::as_slice)),
            role: Some(auto_seed_file_role(file).to_string()),
            matched_keywords: Vec::new(),
            matched_symbols: Vec::new(),
        }
    }));
    seeds
}

fn context_seed_locations(locations: Option<&[TaskPathLocation]>) -> Vec<ContextSeedLocation> {
    locations
        .into_iter()
        .flatten()
        .map(|location| ContextSeedLocation {
            start_line: location.start_line,
            end_line: location.end_line,
        })
        .collect()
}

fn context_seed_locations_from_references(
    references: &[TaskPathReference],
    file: &str,
) -> Vec<ContextSeedLocation> {
    let mut locations = Vec::new();
    for location in references
        .iter()
        .filter(|reference| reference.file == file)
        .filter_map(|reference| reference.location)
    {
        if !locations.contains(&location) {
            locations.push(location);
        }
    }
    context_seed_locations(Some(locations.as_slice()))
}

fn auto_seed_role_allowed(role: &str, task_keywords: &[String]) -> bool {
    role == "source"
        || task_keywords.iter().any(|keyword| match keyword.as_str() {
            "test" | "tests" | "testing" | "spec" | "specs" | "coverage" | "regression"
            | "unit" | "integration" | "e2e" => role == "test",
            "fixture" | "fixtures" => role == "fixture",
            "vendor" | "dependency" | "dependencies" | "package" | "packages" | "third"
            | "external" => role == "vendor",
            "doc" | "docs" | "documentation" | "readme" => role == "docs",
            "example" | "examples" | "demo" | "sample" | "samples" => role == "example",
            _ => false,
        })
}

fn auto_seed_file_role(file: &str) -> &'static str {
    let normalized = file.to_ascii_lowercase();
    if normalized == "node_modules" || normalized.starts_with("node_modules/") {
        "vendor"
    } else if normalized.contains("fixture") || normalized.contains("fixtures/") {
        "fixture"
    } else if is_low_value_reference_file(file) {
        "test"
    } else if normalized == "docs" || normalized.starts_with("docs/") {
        "docs"
    } else if normalized == "examples" || normalized.starts_with("examples/") {
        "example"
    } else {
        "source"
    }
}

pub(crate) fn task_keywords(task: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    // Preserve localized intent when a long mixed-language prompt also expands
    // many English aliases and reaches the keyword budget.
    for (phrase, aliases) in chinese_task_keyword_aliases() {
        if task.contains(phrase) {
            for alias in *aliases {
                push_task_keyword(&mut keywords, alias);
            }
        }
    }
    for word in task
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| (word.len() >= 3 || word == "no") && !is_task_stop_word(word))
        .take(16)
    {
        push_task_keyword(&mut keywords, word.as_str());
        for alias in task_keyword_aliases(&word) {
            push_task_keyword(&mut keywords, alias);
        }
    }
    keywords.truncate(32);
    keywords
}

fn chinese_task_keyword_aliases() -> &'static [(&'static str, &'static [&'static str])] {
    &[
        ("路由", &["route", "router", "routing"]),
        ("认证", &["auth", "authentication"]),
        ("登录", &["login", "auth", "authentication"]),
        ("权限", &["permission", "permissions", "authorization"]),
        ("授权", &["authorization", "permission"]),
        ("配置", &["config", "configuration", "settings"]),
        ("设置", &["settings", "config"]),
        ("启动", &["startup", "entrypoint", "bootstrap"]),
        ("入口", &["entrypoint", "startup"]),
        ("中间件", &["middleware"]),
        ("持久化", &["persistence", "storage"]),
        ("存储", &["storage", "persistence"]),
        ("数据库", &["database", "storage"]),
        ("测试", &["test", "tests", "testing"]),
        ("冒烟", &["smoke", "test"]),
        ("脚本", &["script", "scripts"]),
        ("文档", &["docs", "documentation"]),
        ("示例", &["example", "examples"]),
        ("演示", &["demo"]),
        ("基准", &["benchmark"]),
        ("发布", &["release"]),
        ("工作流", &["workflow"]),
        ("打包", &["packaging", "package"]),
        ("缓存", &["cache"]),
        ("性能", &["performance"]),
        ("日志", &["logs", "observability"]),
        ("监控", &["monitoring", "observability"]),
        ("安全", &["security"]),
        ("漏洞", &["vulnerability", "security"]),
        ("支付", &["payment", "billing"]),
        ("订阅", &["subscription", "billing"]),
        ("渲染", &["render", "rendering"]),
        ("组件", &["component", "frontend"]),
        ("队列", &["queue", "job"]),
        ("后台任务", &["background", "job", "worker"]),
        ("上下文", &["context"]),
        ("智能体", &["agent"]),
    ]
}

fn push_task_keyword(keywords: &mut Vec<String>, keyword: &str) {
    if !keywords.iter().any(|existing| existing == keyword) {
        keywords.push(keyword.to_string());
    }
}

fn task_keyword_aliases(keyword: &str) -> &'static [&'static str] {
    match keyword {
        "route" => &["routes", "router", "routing"],
        "routes" => &["route", "router", "routing"],
        "router" => &["route", "routes", "routing"],
        "routing" => &["route", "routes", "router"],
        "blueprint" => &["blueprints", "route", "routes", "routing"],
        "blueprints" => &["blueprint", "route", "routes", "routing"],
        "mounted" => &["mount", "mounting", "router"],
        "mounting" => &["mount", "mounted", "router"],
        "mount" => &["mounted", "mounting", "router"],
        "subrouter" => &["router", "route", "routes"],
        "nested" => &["router", "route", "routes"],
        "group" => &["groups", "route", "routes"],
        "groups" => &["group", "route", "routes"],
        "prefix" => &["prefixes", "route", "routes"],
        "prefixes" => &["prefix", "route", "routes"],
        "notfound" => &["not", "found", "404"],
        "noroute" => &["route", "routes", "404"],
        "nomethod" => &["method", "methods", "405"],
        "allowed" => &["method"],
        "finalhandler" => &["final", "handler", "handlers"],
        "url" => &["urls"],
        "urls" => &["url"],
        "build" => &["building", "builder"],
        "builds" => &["build", "building"],
        "building" => &["build", "builder"],
        "builder" => &["build", "building"],
        "generate" => &["generation", "generator"],
        "generation" => &["generate", "generator"],
        "generator" => &["generate", "generation"],
        "indexing" => &["index", "indexer"],
        "indexed" => &["index", "indexer"],
        "indexer" => &["index", "indexing"],
        "join" => &["joining"],
        "joins" => &["join", "joining"],
        "joining" => &["join"],
        "reverse" => &["routing"],
        "startup" => &["start", "boot", "program"],
        "start" => &["startup", "boot"],
        "boot" => &["startup", "start"],
        "lifecycle" => &["runtime", "execution"],
        "runtime" => &["lifecycle", "execution"],
        "execution" => &["runtime", "lifecycle"],
        "runner" => &["runtime", "script"],
        "rerun" => &["runtime", "lifecycle"],
        "reruns" => &["rerun", "runtime", "lifecycle"],
        "upload" => &["uploaded", "uploader", "file"],
        "uploaded" => &["upload", "uploader", "file"],
        "uploader" => &["upload", "uploaded", "file"],
        "uploads" => &["upload", "uploaded", "file"],
        "websocket" => &["socket"],
        "websockets" => &["websocket", "socket"],
        "socket" => &["websocket", "connection"],
        "sockets" => &["socket", "websocket", "connection"],
        "connection" => &["connections"],
        "connections" => &["connection"],
        "authentication" => &["auth", "login"],
        "authenticate" => &["auth", "login"],
        "access" => &["authorization", "permission", "permissions"],
        "acl" => &["authorization", "permission", "permissions"],
        "rbac" => &["authorization", "permission", "permissions"],
        "authz" => &["authorization", "permission", "permissions"],
        "authorization" => &["authorize", "authz", "permission", "permissions"],
        "authorize" => &["authorization", "authz", "permission"],
        "permission" => &["permissions", "authorization", "authz", "access"],
        "permissions" => &["permission", "authorization", "authz", "access"],
        "token" => &["tokens", "credential", "session"],
        "tokens" => &["token", "credential", "session"],
        "oauth" => &["token", "credential"],
        "jwt" => &["token", "credential"],
        "login" => &["auth", "authentication"],
        "signin" => &["auth", "login"],
        "cookie" => &["cookies", "cookiejar", "jar"],
        "cookies" => &["cookie", "cookiejar", "jar"],
        "cookiejar" => &["cookie", "cookies", "jar"],
        "jar" => &["cookie", "cookies", "cookiejar"],
        "header" => &["headers", "case", "insensitive"],
        "headers" => &["header", "case", "insensitive"],
        "case" => &["insensitive", "headers"],
        "insensitive" => &["case", "headers"],
        "body" => &["payload", "parser", "parse"],
        "bodies" => &["body", "payload", "parser"],
        "payload" => &["body", "binding", "parse"],
        "payloads" => &["payload", "body", "binding"],
        "multipart" => &["form", "body"],
        "contenttype" => &["binding", "parser"],
        "content-type" => &["binding", "parser"],
        "query" => &["queries", "parameter", "params"],
        "queries" => &["query", "parameter", "params"],
        "parameter" => &["parameters", "param"],
        "parameters" => &["parameter", "params"],
        "param" => &["parameter", "params"],
        "params" => &["param", "parameters"],
        "args" => &["arguments"],
        "arguments" => &["args"],
        "static" => &["file", "files", "assets"],
        "asset" => &["assets", "static", "file"],
        "assets" => &["asset", "static", "files"],
        "file" => &["files"],
        "files" => &["file"],
        "folder" => &["folders", "directory"],
        "folders" => &["folder", "directory"],
        "directory" => &["directories", "folder"],
        "directories" => &["directory", "folder"],
        "serve" => &["serving"],
        "serving" => &["serve"],
        "filesystem" => &["file", "files"],
        "sendfile" => &["static", "file", "serving"],
        "send_file" => &["static", "file", "serving"],
        "output" => &["outputs", "response"],
        "outputs" => &["output", "response"],
        "template" => &["templates"],
        "templates" => &["template"],
        "network" => &["http", "transport", "client"],
        "http" => &["https", "network", "client"],
        "https" => &["http", "network", "client"],
        "proxy" => &["proxies", "adapter", "transport"],
        "proxies" => &["proxy", "adapter", "transport"],
        "redirect" => &["redirects", "redirection"],
        "redirects" => &["redirect", "redirection"],
        "redirection" => &["redirect", "redirects"],
        "transport" => &["transports", "adapter", "network"],
        "transports" => &["transport", "adapter", "network"],
        "adapter" => &["adapters", "transport", "network"],
        "adapters" => &["adapter", "transport", "network"],
        "tls" => &["ssl", "certificate", "verify"],
        "ssl" => &["tls", "certificate", "verify"],
        "certificate" => &["cert", "certificates", "tls"],
        "certificates" => &["certificate", "certs", "tls"],
        "cert" => &["certificate", "certs", "tls"],
        "certs" => &["cert", "certificate", "tls"],
        "verify" => &["verification", "certificate", "tls"],
        "verification" => &["verify", "certificate", "tls"],
        "trust" => &["trusted", "certificate", "tls"],
        "trusted" => &["trust", "certificate", "tls"],
        "truststore" => &["certificate", "tls", "verify"],
        "validation" => &["validate", "validator", "schema"],
        "validate" => &["validation", "validator", "schema"],
        "validated" => &["validate", "validation", "validator"],
        "validator" => &["validators", "validation", "schema"],
        "validators" => &["validator", "validation", "schema"],
        "schema" => &["schemas", "validation", "validator"],
        "schemas" => &["schema", "validation", "validator"],
        "binding" => &["bindings", "bind", "validation"],
        "bindings" => &["binding", "bind", "validation"],
        "bind" => &["binding", "bindings", "validation"],
        "parser" => &["parsers", "parse", "parsing"],
        "parsers" => &["parser", "parse", "parsing"],
        "parse" => &["parser", "parsing", "deserialize"],
        "parsing" => &["parse", "parser", "deserialize"],
        "json" => &["serialization", "deserialize", "binding"],
        "serialize" => &["serializer", "serialization", "json"],
        "serializer" => &["serializers", "serialization", "json"],
        "serializers" => &["serializer", "serialization", "json"],
        "serialization" => &["serialize", "serializer", "json"],
        "deserialize" => &["deserializer", "deserialization", "json"],
        "deserializer" => &["deserializers", "deserialization", "json"],
        "deserializers" => &["deserializer", "deserialization", "json"],
        "deserialization" => &["deserialize", "deserializer", "json"],
        "marshal" => &["unmarshal", "serialization", "json"],
        "unmarshal" => &["marshal", "deserialization", "json"],
        "flag" => &["flags", "toggle", "rollout"],
        "flags" => &["flag", "toggle", "rollout"],
        "toggle" => &["toggles", "flag", "rollout"],
        "toggles" => &["toggle", "flag", "rollout"],
        "rollout" => &["rollouts", "flag", "experiment"],
        "rollouts" => &["rollout", "flag", "experiment"],
        "experiment" => &["experiments", "variant", "rollout"],
        "experiments" => &["experiment", "variant", "rollout"],
        "variant" => &["variants", "experiment", "flag"],
        "variants" => &["variant", "experiment", "flag"],
        "config" => &["configuration", "settings", "setting"],
        "configuration" => &["config", "settings", "setting"],
        "setting" => &["config", "configuration", "settings"],
        "settings" => &["config", "configuration", "setting"],
        "api" => &["endpoint", "handler", "controller"],
        "endpoint" => &["endpoints", "api", "handler", "route"],
        "endpoints" => &["endpoint", "api", "handler", "routes"],
        "handler" => &["handlers", "api", "controller"],
        "handlers" => &["handler", "api", "controller"],
        "controller" => &["controllers", "api", "handler"],
        "controllers" => &["controller", "api", "handler"],
        "request" => &["requests", "api", "handler"],
        "requests" => &["request", "api", "handler"],
        "response" => &["responses", "api", "handler"],
        "responses" => &["response", "api", "handler"],
        "action" => &["actions", "handler", "controller"],
        "actions" => &["action", "handler", "controller"],
        "cache" => &["caches", "cached", "performance"],
        "caches" => &["cache", "cached", "performance"],
        "cached" => &["cache", "caching", "performance"],
        "caching" => &["cache", "cached", "performance"],
        "performance" => &["perf", "latency", "optimization"],
        "perf" => &["performance", "latency", "optimization"],
        "latency" => &["performance", "slow", "optimization"],
        "slow" => &["latency", "performance", "optimization"],
        "slowness" => &["slow", "latency", "performance"],
        "optimize" => &["optimization", "performance", "latency"],
        "optimization" => &["optimize", "performance", "latency"],
        "optimise" => &["optimisation", "performance", "latency"],
        "optimisation" => &["optimise", "performance", "latency"],
        "observability" => &["telemetry", "logging", "metrics"],
        "observe" => &["observability", "telemetry", "monitoring"],
        "telemetry" => &["observability", "logging", "metrics"],
        "logging" => &["log", "logs", "logger"],
        "log" => &["logs", "logger", "logging"],
        "logs" => &["log", "logger", "logging"],
        "logger" => &["log", "logs", "logging"],
        "metric" => &["metrics", "telemetry", "monitoring"],
        "metrics" => &["metric", "telemetry", "monitoring"],
        "trace" => &["traces", "tracing", "span"],
        "traces" => &["trace", "tracing", "span"],
        "tracing" => &["trace", "traces", "span"],
        "span" => &["spans", "trace", "tracing"],
        "spans" => &["span", "trace", "tracing"],
        "monitor" => &["monitoring", "observability", "metrics"],
        "monitoring" => &["monitor", "observability", "metrics"],
        "instrumentation" => &["observability", "telemetry", "metrics"],
        "security" => &["secure", "vulnerability", "sanitize"],
        "secure" => &["security", "secret", "encryption"],
        "vulnerability" => &["vulnerabilities", "security", "vuln"],
        "vulnerabilities" => &["vulnerability", "security", "vulns"],
        "vuln" => &["vulnerability", "security"],
        "vulns" => &["vulnerabilities", "security"],
        "secret" => &["secrets", "security", "encryption"],
        "secrets" => &["secret", "security", "encryption"],
        "encrypt" => &["encryption", "security", "secret"],
        "encryption" => &["encrypt", "security", "secret"],
        "decrypt" => &["decryption", "security", "secret"],
        "decryption" => &["decrypt", "security", "secret"],
        "csrf" => &["security", "vulnerability"],
        "xss" => &["security", "vulnerability", "sanitize"],
        "injection" => &["security", "vulnerability", "sanitize"],
        "sanitize" => &["sanitization", "security", "vulnerability"],
        "sanitization" => &["sanitize", "security", "vulnerability"],
        "sanitise" => &["sanitisation", "security", "vulnerability"],
        "sanitisation" => &["sanitise", "security", "vulnerability"],
        "billing" => &["payment", "subscription", "invoice"],
        "bill" => &["billing", "payment", "invoice"],
        "payment" => &["payments", "billing", "checkout"],
        "payments" => &["payment", "billing", "checkout"],
        "checkout" => &["payment", "billing", "subscription"],
        "subscription" => &["subscriptions", "billing", "payment"],
        "subscriptions" => &["subscription", "billing", "payment"],
        "subscribe" => &["subscription", "billing", "payment"],
        "invoice" => &["invoices", "billing", "payment"],
        "invoices" => &["invoice", "billing", "payment"],
        "pricing" => &["price", "billing", "subscription"],
        "stripe" => &["payment", "billing", "checkout"],
        "frontend" => &["ui", "component", "page"],
        "front-end" => &["frontend", "ui", "component"],
        "ui" => &["frontend", "component", "screen"],
        "component" => &["components", "frontend", "ui"],
        "components" => &["component", "frontend", "ui"],
        "page" => &["pages", "frontend", "component"],
        "pages" => &["page", "frontend", "component"],
        "screen" => &["screens", "ui", "component"],
        "screens" => &["screen", "ui", "component"],
        "form" => &["forms", "ui", "component"],
        "forms" => &["form", "ui", "component"],
        "layout" => &["layouts", "ui", "component"],
        "layouts" => &["layout", "ui", "component"],
        "style" => &["styles", "css", "ui"],
        "styles" => &["style", "css", "ui"],
        "css" => &["style", "styles", "ui"],
        "background" => &["job", "queue", "worker"],
        "job" => &["jobs", "queue", "worker"],
        "jobs" => &["job", "queue", "worker"],
        "queue" => &["queues", "job", "worker"],
        "queues" => &["queue", "job", "worker"],
        "worker" => &["workers", "queue", "job"],
        "workers" => &["worker", "queue", "job"],
        "scheduler" => &["schedulers", "schedule", "cron"],
        "schedulers" => &["scheduler", "schedule", "cron"],
        "schedule" => &["scheduler", "scheduled", "cron"],
        "scheduled" => &["schedule", "scheduler", "cron"],
        "cron" => &["scheduler", "schedule", "job"],
        "async" => &["asynchronous", "background", "job"],
        "asynchronous" => &["async", "background", "job"],
        "doc" => &["docs", "documentation", "readme"],
        "docs" => &["doc", "documentation", "readme"],
        "documentation" => &["doc", "docs", "readme"],
        "readme" => &["docs", "documentation", "guide"],
        "guide" => &["guides", "docs", "usage"],
        "guides" => &["guide", "docs", "usage"],
        "usage" => &["docs", "guide", "example"],
        "tutorial" => &["tutorials", "docs", "guide"],
        "tutorials" => &["tutorial", "docs", "guide"],
        "database" => &["db", "persistence", "storage", "repository"],
        "persistence" => &["database", "storage", "repository"],
        "persist" => &["persistence", "database", "storage"],
        "storage" => &["database", "persistence", "repository"],
        "repository" => &["database", "persistence", "storage"],
        "migration" => &["migrations", "migrate", "database", "storage", "schema"],
        "migrations" => &["migration", "migrate", "database", "storage", "schema"],
        "migrate" => &["migration", "migrations", "database", "storage", "schema"],
        "error" => &["errors", "exception", "failure"],
        "errors" => &["error", "exception", "failure"],
        "exception" => &["error", "failure"],
        "exceptions" => &["error", "failure"],
        "failure" => &["error", "exception"],
        "failures" => &["error", "exception"],
        "retry" => &["retries", "error", "failure"],
        "retries" => &["retry", "error", "failure"],
        "timeout" => &["timeouts", "error", "failure"],
        "timeouts" => &["timeout", "error", "failure"],
        "debug" => &["error", "failure"],
        "bug" => &["error", "failure"],
        "fallback" => &["error", "recovery"],
        "recovery" => &["recover", "error", "failure"],
        "recover" => &["recovery", "error", "failure"],
        "test" => &["tests", "testing", "spec", "coverage"],
        "tests" => &["test", "testing", "spec", "coverage"],
        "testing" => &["test", "tests", "spec", "coverage"],
        "spec" => &["test", "tests", "coverage"],
        "specs" => &["test", "tests", "coverage"],
        "coverage" => &["test", "tests", "spec", "regression"],
        "regression" => &["test", "tests", "coverage"],
        "unit" => &["test", "tests"],
        "integration" => &["test", "tests"],
        "e2e" => &["test", "tests"],
        _ => &[],
    }
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
            | "application"
    )
}

fn normalize_seed_file(root: &Path, file: &str) -> Result<String> {
    Ok(normalize_seed_file_reference(root, file)?.file)
}

fn normalize_explicit_seed_files(
    root: &Path,
    seed_files: &[String],
) -> Result<(Vec<String>, TaskPathLocations)> {
    let mut files = Vec::new();
    let mut locations = TaskPathLocations::new();

    for seed_file in seed_files {
        let reference = normalize_seed_file_reference(root, seed_file)?;
        if !files.contains(&reference.file) {
            files.push(reference.file.clone());
        }
        if let Some(location) = reference.location {
            let file_locations = locations.entry(reference.file).or_default();
            if !file_locations.contains(&location) {
                file_locations.push(location);
            }
        }
    }

    Ok((files, locations))
}

fn normalize_seed_file_reference(root: &Path, file: &str) -> Result<TaskPathReference> {
    let normalized = file.replace('\\', "/");
    let (path_token, location) = split_auto_seed_task_path_location(&normalized);
    let path = PathBuf::from(path_token);
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
    Ok(TaskPathReference {
        file: relative.to_string_lossy().replace('\\', "/"),
        location,
    })
}

fn normalize_dependency_languages(languages: &[String]) -> Result<Vec<String>> {
    languages
        .iter()
        .map(|language| normalize_dependency_language(language))
        .collect()
}

fn normalize_dependency_language(language: &str) -> Result<String> {
    let normalized = language.trim().to_ascii_lowercase();
    let normalized = match normalized.as_str() {
        "shell" | "sh" => "bash",
        "js" => "javascript",
        "ts" => "typescript",
        "c++" => "cpp",
        "c#" => "csharp",
        value => value,
    };
    let allowed = [
        Language::Bash,
        Language::C,
        Language::Cpp,
        Language::CSharp,
        Language::Go,
        Language::Java,
        Language::JavaScript,
        Language::Php,
        Language::Python,
        Language::Ruby,
        Language::Rust,
        Language::TypeScript,
        Language::Tsx,
    ];
    if allowed
        .iter()
        .any(|language| language.as_str() == normalized)
    {
        Ok(normalized.to_string())
    } else {
        bail!("unsupported dependency graph language filter: {language}")
    }
}

fn normalize_dependency_kinds(kinds: &[String]) -> Result<Vec<String>> {
    kinds
        .iter()
        .map(|kind| normalize_dependency_kind(kind))
        .collect()
}

fn normalize_dependency_kind(kind: &str) -> Result<String> {
    let normalized = kind.trim().to_ascii_lowercase().replace('-', "_");
    let allowed = [
        "base_type",
        "export_alias",
        "export_namespace",
        "extension_method",
        "import",
        "import_alias",
        "import_namespace",
        "import_static",
        "include",
        "mod",
        "namespace",
        "package",
        "property_type",
        "require",
        "require_relative",
        "type_binding",
        "use",
        "using",
        "using_alias",
        "using_static",
    ];
    if allowed.contains(&normalized.as_str()) {
        Ok(normalized)
    } else {
        bail!("unsupported dependency graph kind filter: {kind}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn compact_response_trimming_preserves_requested_location_excerpts() {
        let mut route = json!({
            "context_pack": {
                "selected_seeds": [{
                    "kind": "file",
                    "value": "src/main.ts",
                    "locations": [
                        {"start_line": 20, "end_line": 20},
                        {"start_line": 70, "end_line": 75}
                    ]
                }],
                "files": [
                    {
                        "file": "src/main.ts",
                        "ranges": [
                            {"start_line": 1, "end_line": 5, "excerpt": "header"},
                            {"start_line": 18, "end_line": 22, "excerpt": "requested first"},
                            {"start_line": 70, "end_line": 75, "excerpt": "requested second"},
                            {"start_line": 90, "end_line": 95, "excerpt": "tail"}
                        ]
                    },
                    {
                        "file": "src/helper.ts",
                        "ranges": [
                            {"start_line": 1, "end_line": 4, "excerpt": "helper"}
                        ]
                    }
                ]
            }
        });

        assert!(remove_last_non_requested_compact_excerpt(&mut route));
        assert!(remove_last_non_requested_compact_excerpt(&mut route));
        assert!(remove_last_non_requested_compact_excerpt(&mut route));
        assert!(!remove_last_non_requested_compact_excerpt(&mut route));

        let ranges = route["context_pack"]["files"][0]["ranges"]
            .as_array()
            .unwrap();
        assert!(ranges[0].get("excerpt").is_none());
        assert_eq!(ranges[1]["excerpt"], "requested first");
        assert_eq!(ranges[2]["excerpt"], "requested second");
        assert!(ranges[3].get("excerpt").is_none());
        assert!(
            route["context_pack"]["files"][1]["ranges"][0]
                .get("excerpt")
                .is_none()
        );
    }

    #[test]
    fn task_paths_cannot_escape_project_root() {
        let parent = tempfile::TempDir::new().unwrap();
        let root = parent.path().join("repo");
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src/main.ts"), "export const main = true;\n").unwrap();
        std::fs::write(
            parent.path().join("outside.ts"),
            "export const secret = true;\n",
        )
        .unwrap();

        assert!(task_has_existing_path(&root, "inspect src/main.ts"));
        assert!(!task_has_existing_path(&root, "inspect ../outside.ts"));
        assert_eq!(
            auto_seed_task_path_tokens(
                &root,
                "inspect src/main.ts, ../outside.ts, src/../../outside.ts and ./src/main.ts"
            ),
            vec!["src/main.ts"]
        );
        let absolute_task = format!("inspect {}", root.join("src/main.ts").display());
        assert_eq!(
            auto_seed_task_path_tokens(&root, &absolute_task),
            vec!["src/main.ts"]
        );
        assert!(task_has_existing_path(&root, &absolute_task));
        for (located_task, expected_location) in [
            (
                "inspect \"src/main.ts:12:4\"".to_string(),
                TaskPathLocation {
                    start_line: 12,
                    end_line: 12,
                },
            ),
            (
                "inspect src/main.ts#L4-L2".to_string(),
                TaskPathLocation {
                    start_line: 2,
                    end_line: 4,
                },
            ),
            (
                format!("inspect '{}#L1'", root.join("src/main.ts").display()),
                TaskPathLocation {
                    start_line: 1,
                    end_line: 1,
                },
            ),
        ] {
            assert_eq!(
                auto_seed_task_path_tokens(&root, &located_task),
                vec!["src/main.ts"]
            );
            assert_eq!(
                auto_seed_task_path_references(&root, &located_task),
                vec![TaskPathReference {
                    file: "src/main.ts".to_string(),
                    location: Some(expected_location),
                }]
            );
            assert!(task_has_existing_path(&root, &located_task));
        }

        let spaced_root = parent.path().join("repo with spaces");
        std::fs::create_dir_all(spaced_root.join("src")).unwrap();
        std::fs::write(
            spaced_root.join("src/main file.ts"),
            "export const main = true;\n",
        )
        .unwrap();
        for quoted_task in [
            "inspect 'src/main file.ts'".to_string(),
            "inspect `src/main file.ts`".to_string(),
            format!(
                "inspect \"{}\"",
                spaced_root.join("src/main file.ts").display()
            ),
        ] {
            assert_eq!(
                auto_seed_task_path_tokens(&spaced_root, &quoted_task),
                vec!["src/main file.ts"]
            );
            assert!(task_has_existing_path(&spaced_root, &quoted_task));
        }

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(
                parent.path().join("outside.ts"),
                root.join("src/outside-link.ts"),
            )
            .unwrap();
            assert!(!task_has_existing_path(
                &root,
                "inspect src/outside-link.ts"
            ));
            assert_eq!(
                auto_seed_task_path_tokens(&root, "inspect src/outside-link.ts"),
                vec!["src/outside-link.ts"]
            );
        }
    }

    #[test]
    fn selected_context_metadata_is_bounded_to_returned_ranges() {
        let files = vec![ContextFile {
            file: "src/main.ts".to_string(),
            source: "seed_file".to_string(),
            score: 100,
            selection_rank: 1,
            reason: "selected test range".to_string(),
            source_mix: Vec::new(),
            ranges: vec![ContextRange {
                start_line: 10,
                end_line: 20,
                source: "seed_file".to_string(),
                score: 100,
                importance: "high".to_string(),
                reason: "selected test range".to_string(),
                excerpt: "export function selected() {}".to_string(),
            }],
        }];
        let selected_symbol = Symbol {
            name: "selected".to_string(),
            qualified_name: "selected".to_string(),
            kind: SymbolKind::Function,
            language: Language::TypeScript,
            file: "src/main.ts".to_string(),
            start_line: 12,
            end_line: 18,
        };
        let mut symbols = vec![
            selected_symbol.clone(),
            selected_symbol,
            Symbol {
                name: "outside".to_string(),
                qualified_name: "outside".to_string(),
                kind: SymbolKind::Function,
                language: Language::TypeScript,
                file: "src/main.ts".to_string(),
                start_line: 30,
                end_line: 35,
            },
        ];
        let selected_reference = ReferenceMatch {
            file: "src/main.ts".to_string(),
            line: 15,
            column: 3,
            context: "selected();".to_string(),
            reference_kind: "call".to_string(),
            confidence: 1.0,
        };
        let mut references = vec![
            selected_reference.clone(),
            selected_reference,
            ReferenceMatch {
                file: "src/other.ts".to_string(),
                line: 15,
                column: 3,
                context: "selected();".to_string(),
                reference_kind: "call".to_string(),
                confidence: 1.0,
            },
        ];

        retain_selected_context_metadata(&files, &mut symbols, &mut references);

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "selected");
        assert_eq!(references.len(), 1);
        assert_eq!(references[0].file, "src/main.ts");
    }

    #[test]
    fn uncovered_segments_keeps_ranges_after_selected_overlap() {
        assert_eq!(uncovered_segments(1, 10, &[(4, 6)]), vec![(1, 3), (7, 10)]);
    }

    #[test]
    fn focused_test_command_targets_supported_test_files() {
        assert_eq!(
            focused_test_command("pnpm test", "src/core.test.ts").as_deref(),
            Some("pnpm test -- src/core.test.ts")
        );
        assert_eq!(
            focused_test_command("pytest", "tests/test_api.py").as_deref(),
            Some("pytest tests/test_api.py")
        );
        assert!(focused_test_command("pytest", "src/core.test.ts").is_none());
        assert!(focused_test_command("pnpm test", "tests/test_api.py").is_none());
        assert_eq!(
            focused_test_command("go test ./...", "binding/default_validator_test.go").as_deref(),
            Some("go test ./binding")
        );
        assert_eq!(
            focused_test_command("pytest", "tests/api test.py").as_deref(),
            Some("pytest 'tests/api test.py'")
        );
        assert_eq!(
            focused_test_command("go test ./...", "go pkg/http_test.go").as_deref(),
            Some("go test './go pkg'")
        );
        assert_eq!(
            focused_test_command("cargo test --locked", "tests/cli.rs").as_deref(),
            Some("cargo test --locked --test cli")
        );
        assert_eq!(
            focused_test_command("cargo test --locked", "src/core_test.rs").as_deref(),
            Some("cargo test --locked core_test")
        );
        assert_eq!(
            focused_test_command(
                "mvn test",
                "src/test/java/com/example/TokenNormalizerTest.java"
            )
            .as_deref(),
            Some("mvn -Dtest=TokenNormalizerTest test")
        );
        assert_eq!(
            focused_test_command(
                "./gradlew --no-daemon test",
                "src/test/java/com/example/TokenNormalizerTest.java"
            )
            .as_deref(),
            Some("./gradlew --no-daemon test --tests TokenNormalizerTest")
        );
        assert_eq!(
            focused_test_command(
                "gradle test",
                "src/test/java/com/example/TokenNormalizerTest.java"
            )
            .as_deref(),
            Some("gradle test --tests TokenNormalizerTest")
        );
        assert_eq!(
            focused_test_command("dotnet test", "tests/TokenNormalizerTests.cs").as_deref(),
            Some("dotnet test --filter FullyQualifiedName~TokenNormalizerTests")
        );
        assert_eq!(
            focused_test_command("bundle exec rspec", "spec/core_spec.rb").as_deref(),
            Some("bundle exec rspec spec/core_spec.rb")
        );
        assert!(focused_test_command("cargo test", "src/lib.rs").is_none());
    }

    #[test]
    fn configured_file_filters_match_paths_without_prefix_bleed() {
        assert!(configured_file_filter_matches("src/core.ts", "src/core.ts"));
        assert!(configured_file_filter_matches("src/core.ts", "src/core"));
        assert!(configured_file_filter_matches(
            "src/core/index.ts",
            "src/core"
        ));
        assert!(configured_file_filter_matches(
            "src/core/index.ts",
            "src/core/"
        ));
        assert!(configured_file_filter_matches(
            "src\\core\\index.ts",
            "src/core/"
        ));
        assert!(!configured_file_filter_matches("src/core2.ts", "src/core"));
        assert!(!configured_file_filter_matches(
            "src/core-extra/index.ts",
            "src/core"
        ));
        assert!(!configured_file_filter_matches("src/core.ts", ""));
    }

    #[test]
    fn impact_breakdown_uses_full_reason_sets_before_display_truncation() {
        let mut reasons = BTreeSet::new();
        for index in 0..12 {
            reasons.insert(format!("callee_source:helper{index}->leaf"));
        }
        reasons.insert("seed_file".to_string());

        let breakdown =
            impact_breakdown_from_reason_sets(std::iter::once(&reasons), &Vec::new(), 0);

        assert_eq!(breakdown.seed_files, 1);
        assert_eq!(breakdown.call_related_files, 1);
    }

    #[test]
    fn task_keywords_map_common_chinese_routing_intents() {
        for (task, expected) in [
            ("理解路由和权限", "routing"),
            ("检查数据库持久化", "persistence"),
            ("排查日志和监控", "observability"),
            ("修复安全漏洞", "security"),
            ("调整支付订阅", "billing"),
            ("检查后台任务队列", "worker"),
            ("更新前端组件渲染", "component"),
            ("优化智能体上下文", "agent"),
        ] {
            assert!(
                task_keywords(task).contains(&expected.to_string()),
                "expected {expected} for {task}"
            );
        }

        let mixed_language_keywords = task_keywords(
            "route routes router routing authentication login authorization permissions configuration settings startup entrypoint middleware persistence storage database 排查日志监控",
        );
        assert!(mixed_language_keywords.contains(&"observability".to_string()));
        assert!(mixed_language_keywords.contains(&"monitoring".to_string()));
    }

    #[test]
    fn query_graph_backend_results_normalize_tabular_rows() {
        let normalized = normalize_backend_query_graph_result(json!({
            "columns": ["f.name", "f.file_path", "f.qualified_name"],
            "rows": [["authenticate", "src/auth.ts", "app.auth.authenticate"]],
            "total": 1,
            "elapsed_ms": 9
        }))
        .unwrap();

        assert_eq!(normalized["results"][0]["file_path"], "src/auth.ts");
        assert_eq!(normalized["results"][0]["name"], "authenticate");
        assert_eq!(normalized["total"], 1);
        assert_eq!(normalized["elapsed_ms"], 9);
    }

    #[test]
    fn query_graph_backend_results_require_a_file_column() {
        let error = normalize_backend_query_graph_result(json!({
            "columns": ["f.name", "f.qualified_name"],
            "rows": [["authenticate", "app.auth.authenticate"]],
            "total": 1
        }))
        .unwrap_err();

        assert!(error.to_string().contains("file_path or file"));
    }

    #[test]
    fn task_keywords_expand_common_agent_routing_aliases() {
        let keywords = task_keywords(
            "understand routing authentication settings startup api handler persistence flow",
        );

        assert!(keywords.contains(&"routing".to_string()));
        assert!(keywords.contains(&"router".to_string()));
        assert!(keywords.contains(&"auth".to_string()));
        assert!(keywords.contains(&"config".to_string()));
        assert!(keywords.contains(&"setting".to_string()));
        assert!(keywords.contains(&"startup".to_string()));
        assert!(keywords.contains(&"program".to_string()));
        assert!(keywords.contains(&"api".to_string()));
        assert!(keywords.contains(&"controller".to_string()));
        assert!(keywords.contains(&"database".to_string()));
        assert!(keywords.contains(&"storage".to_string()));
        assert!(keywords.contains(&"repository".to_string()));
        assert!(!keywords.contains(&"understand".to_string()));
        assert!(!keywords.contains(&"flow".to_string()));

        let debug_keywords = task_keywords("debug retry timeout handling");
        assert!(debug_keywords.contains(&"error".to_string()));
        assert!(debug_keywords.contains(&"failure".to_string()));
        assert!(debug_keywords.contains(&"timeout".to_string()));
        assert!(auto_seed_error_recovery_handling_task(&debug_keywords));

        let error_handling_keywords = task_keywords("understand error handling behavior");
        assert!(error_handling_keywords.contains(&"error".to_string()));
        assert!(error_handling_keywords.contains(&"handling".to_string()));
        assert!(auto_seed_error_recovery_handling_task(
            &error_handling_keywords
        ));

        let recovery_middleware_keywords =
            task_keywords("understand error recovery middleware behavior");
        assert!(recovery_middleware_keywords.contains(&"recovery".to_string()));
        assert!(recovery_middleware_keywords.contains(&"middleware".to_string()));
        assert!(auto_seed_error_recovery_handling_task(
            &recovery_middleware_keywords
        ));

        let coverage_keywords = task_keywords("find regression coverage");
        assert!(coverage_keywords.contains(&"regression".to_string()));
        assert!(coverage_keywords.contains(&"coverage".to_string()));
        assert!(coverage_keywords.contains(&"test".to_string()));

        let docs_keywords = task_keywords("understand documentation usage");
        assert!(docs_keywords.contains(&"docs".to_string()));
        assert!(docs_keywords.contains(&"documentation".to_string()));
        assert!(docs_keywords.contains(&"guide".to_string()));

        let references_keywords = task_keywords("understand find references classification");
        assert!(references_keywords.contains(&"references".to_string()));
        assert!(auto_seed_reference_search_task(&references_keywords));

        let call_graph_keywords = task_keywords("understand callers callees call graph traversal");
        assert!(call_graph_keywords.contains(&"callers".to_string()));
        assert!(call_graph_keywords.contains(&"callees".to_string()));
        assert!(auto_seed_call_graph_traversal_task(&call_graph_keywords));

        let impact_keywords = task_keywords("understand impact analysis risk scoring");
        assert!(impact_keywords.contains(&"impact".to_string()));
        assert!(!auto_seed_call_graph_traversal_task(&impact_keywords));

        let embedding_status_keywords =
            task_keywords("understand embedding provider status reporting");
        assert!(embedding_status_keywords.contains(&"embedding".to_string()));
        assert!(embedding_status_keywords.contains(&"provider".to_string()));
        assert!(auto_seed_embedding_provider_status_task(
            &embedding_status_keywords
        ));

        let embedding_config_keywords =
            task_keywords("understand embedding provider configuration");
        assert!(embedding_config_keywords.contains(&"configuration".to_string()));
        assert!(!auto_seed_embedding_provider_status_task(
            &embedding_config_keywords
        ));

        let config_status_keywords = task_keywords("understand config status parse errors");
        assert!(auto_seed_config_status_reporting_task(
            &config_status_keywords
        ));

        let blocked_no_seed_keywords =
            task_keywords("understand MCP first-call blocked no seed route");
        assert!(auto_seed_blocked_no_seed_route_task(
            &blocked_no_seed_keywords
        ));

        let recommended_next_tools_keywords =
            task_keywords("understand recommended next tools contract");
        assert!(auto_seed_recommended_next_tools_contract_task(
            &recommended_next_tools_keywords
        ));

        let entrypoint_ranking_keywords =
            task_keywords("understand project overview entrypoint ranking");
        assert!(auto_seed_project_entrypoint_ranking_task(
            &entrypoint_ranking_keywords
        ));

        let budget_continuation_keywords = task_keywords("understand token budget continuation");
        assert!(auto_seed_budget_continuation_task(
            &budget_continuation_keywords
        ));
        let budget_continuation_signals =
            ContextTaskSignals::from_task("understand token budget continuation");
        assert!(budget_continuation_signals.budget_continuation);
        assert!(!budget_continuation_signals.auth_session);

        let semantic_index_explain_keywords =
            task_keywords("understand semantic index explain output");
        assert!(auto_seed_semantic_index_explain_task(
            &semantic_index_explain_keywords
        ));

        let impact_checks_keywords = task_keywords("understand impact suggested checks");
        assert!(auto_seed_impact_suggested_checks_task(
            &impact_checks_keywords
        ));

        let mcp_schema_keywords = task_keywords("understand MCP tool schema validation");
        assert!(auto_seed_mcp_tool_schema_validation_task(
            &mcp_schema_keywords
        ));

        let background_keywords = task_keywords("understand background job queue");
        assert!(background_keywords.contains(&"background".to_string()));
        assert!(background_keywords.contains(&"job".to_string()));
        assert!(background_keywords.contains(&"queue".to_string()));
        assert!(background_keywords.contains(&"worker".to_string()));

        let authorization_keywords = task_keywords("understand authorization permissions");
        assert!(authorization_keywords.contains(&"authorization".to_string()));
        assert!(authorization_keywords.contains(&"authz".to_string()));
        assert!(authorization_keywords.contains(&"permission".to_string()));
        assert!(authorization_keywords.contains(&"permissions".to_string()));
        assert!(!authorization_keywords.contains(&"auth".to_string()));

        let access_control_keywords = task_keywords("understand access control rules");
        assert!(access_control_keywords.contains(&"access".to_string()));
        assert!(access_control_keywords.contains(&"authorization".to_string()));
        assert!(access_control_keywords.contains(&"permission".to_string()));
        assert!(access_control_keywords.contains(&"permissions".to_string()));

        let feature_flag_keywords = task_keywords("understand feature flag rollout experiments");
        assert!(feature_flag_keywords.contains(&"flag".to_string()));
        assert!(feature_flag_keywords.contains(&"toggle".to_string()));
        assert!(feature_flag_keywords.contains(&"rollout".to_string()));
        assert!(feature_flag_keywords.contains(&"experiment".to_string()));
        assert!(feature_flag_keywords.contains(&"variant".to_string()));

        let network_keywords = task_keywords("understand proxy redirect transport behavior");
        assert!(network_keywords.contains(&"proxy".to_string()));
        assert!(network_keywords.contains(&"redirect".to_string()));
        assert!(network_keywords.contains(&"transport".to_string()));
        assert!(network_keywords.contains(&"adapter".to_string()));
        assert!(network_keywords.contains(&"network".to_string()));
        assert!(!auto_seed_response_redirect_task(&network_keywords));

        let response_redirect_keywords =
            task_keywords("understand redirect response status location behavior");
        assert!(response_redirect_keywords.contains(&"redirect".to_string()));
        assert!(response_redirect_keywords.contains(&"response".to_string()));
        assert!(response_redirect_keywords.contains(&"status".to_string()));
        assert!(response_redirect_keywords.contains(&"location".to_string()));
        assert!(auto_seed_response_redirect_task(
            &response_redirect_keywords
        ));

        let response_header_keywords = task_keywords("understand response header behavior");
        assert!(response_header_keywords.contains(&"response".to_string()));
        assert!(response_header_keywords.contains(&"header".to_string()));
        assert!(response_header_keywords.contains(&"headers".to_string()));
        assert!(auto_seed_response_headers_task(&response_header_keywords));

        let request_header_binding_keywords =
            task_keywords("understand request header binding behavior");
        assert!(request_header_binding_keywords.contains(&"request".to_string()));
        assert!(request_header_binding_keywords.contains(&"header".to_string()));
        assert!(request_header_binding_keywords.contains(&"binding".to_string()));
        assert!(!auto_seed_response_headers_task(
            &request_header_binding_keywords
        ));

        let client_headers_keywords =
            task_keywords("understand requests headers case insensitive behavior");
        assert!(client_headers_keywords.contains(&"requests".to_string()));
        assert!(client_headers_keywords.contains(&"headers".to_string()));
        assert!(!auto_seed_response_headers_task(&client_headers_keywords));

        let response_cookie_keywords = task_keywords("understand response cookie behavior");
        assert!(response_cookie_keywords.contains(&"response".to_string()));
        assert!(response_cookie_keywords.contains(&"cookie".to_string()));
        assert!(auto_seed_response_cookies_task(&response_cookie_keywords));

        let client_cookie_keywords = task_keywords("understand requests cookie jar behavior");
        assert!(client_cookie_keywords.contains(&"requests".to_string()));
        assert!(client_cookie_keywords.contains(&"cookie".to_string()));
        assert!(client_cookie_keywords.contains(&"jar".to_string()));
        assert!(!auto_seed_response_cookies_task(&client_cookie_keywords));

        let validation_keywords = task_keywords("understand json binding validation behavior");
        assert!(validation_keywords.contains(&"json".to_string()));
        assert!(validation_keywords.contains(&"binding".to_string()));
        assert!(validation_keywords.contains(&"validation".to_string()));
        assert!(validation_keywords.contains(&"schema".to_string()));
        assert!(validation_keywords.contains(&"deserialize".to_string()));

        let request_body_validation_keywords =
            task_keywords("understand fastapi request body validation behavior");
        assert!(auto_seed_request_body_parsing_task(
            &request_body_validation_keywords
        ));
        assert!(!auto_seed_binding_validation_task(
            &request_body_validation_keywords
        ));

        let tls_keywords = task_keywords("understand ssl certificate verification behavior");
        assert!(tls_keywords.contains(&"ssl".to_string()));
        assert!(tls_keywords.contains(&"certificate".to_string()));
        assert!(tls_keywords.contains(&"verification".to_string()));
        assert!(tls_keywords.contains(&"tls".to_string()));
        assert!(tls_keywords.contains(&"verify".to_string()));

        let rbac_keywords = task_keywords("audit rbac acl policy");
        assert!(rbac_keywords.contains(&"rbac".to_string()));
        assert!(rbac_keywords.contains(&"acl".to_string()));
        assert!(rbac_keywords.contains(&"authorization".to_string()));
        assert!(rbac_keywords.contains(&"permission".to_string()));

        let frontend_keywords = task_keywords("understand frontend component rendering");
        assert!(frontend_keywords.contains(&"frontend".to_string()));
        assert!(frontend_keywords.contains(&"component".to_string()));
        assert!(frontend_keywords.contains(&"components".to_string()));
        assert!(frontend_keywords.contains(&"ui".to_string()));
        assert!(frontend_keywords.contains(&"rendering".to_string()));
        assert!(!frontend_keywords.contains(&"render".to_string()));
        assert!(!auto_seed_response_rendering_task(&frontend_keywords));

        let response_rendering_keywords = task_keywords("understand response rendering behavior");
        assert!(response_rendering_keywords.contains(&"response".to_string()));
        assert!(response_rendering_keywords.contains(&"rendering".to_string()));
        assert!(auto_seed_response_rendering_task(
            &response_rendering_keywords
        ));

        let static_file_keywords = task_keywords("understand static file serving behavior");
        assert!(static_file_keywords.contains(&"static".to_string()));
        assert!(static_file_keywords.contains(&"file".to_string()));
        assert!(static_file_keywords.contains(&"serving".to_string()));
        assert!(auto_seed_static_file_serving_task(&static_file_keywords));

        let static_asset_keywords = task_keywords("understand static assets behavior");
        assert!(static_asset_keywords.contains(&"static".to_string()));
        assert!(static_asset_keywords.contains(&"assets".to_string()));
        assert!(auto_seed_static_file_serving_task(&static_asset_keywords));

        let body_parsing_keywords = task_keywords("understand request body parsing behavior");
        assert!(body_parsing_keywords.contains(&"request".to_string()));
        assert!(body_parsing_keywords.contains(&"body".to_string()));
        assert!(body_parsing_keywords.contains(&"parsing".to_string()));
        assert!(auto_seed_request_body_parsing_task(&body_parsing_keywords));

        let payload_binding_keywords = task_keywords("understand payload binding behavior");
        assert!(payload_binding_keywords.contains(&"payload".to_string()));
        assert!(payload_binding_keywords.contains(&"binding".to_string()));
        assert!(auto_seed_request_body_parsing_task(
            &payload_binding_keywords
        ));

        let query_param_keywords = task_keywords("understand query parameter parsing behavior");
        assert!(query_param_keywords.contains(&"query".to_string()));
        assert!(query_param_keywords.contains(&"parameter".to_string()));
        assert!(query_param_keywords.contains(&"params".to_string()));
        assert!(auto_seed_request_query_params_task(&query_param_keywords));
        assert!(!auto_seed_route_parameters_task(&query_param_keywords));

        let sql_query_keywords = task_keywords("understand sql query performance");
        assert!(sql_query_keywords.contains(&"query".to_string()));
        assert!(!auto_seed_request_query_params_task(&sql_query_keywords));

        let route_param_keywords = task_keywords("understand route parameter behavior");
        assert!(route_param_keywords.contains(&"route".to_string()));
        assert!(route_param_keywords.contains(&"parameter".to_string()));
        assert!(route_param_keywords.contains(&"param".to_string()));
        assert!(auto_seed_route_parameters_task(&route_param_keywords));

        let path_param_keywords = task_keywords("understand path parameter behavior");
        assert!(path_param_keywords.contains(&"path".to_string()));
        assert!(path_param_keywords.contains(&"parameter".to_string()));
        assert!(auto_seed_route_parameters_task(&path_param_keywords));

        let route_variable_keywords = task_keywords("understand route variable behavior");
        assert!(route_variable_keywords.contains(&"route".to_string()));
        assert!(route_variable_keywords.contains(&"variable".to_string()));
        assert!(auto_seed_route_parameters_task(&route_variable_keywords));

        let url_building_keywords = task_keywords("understand url building behavior");
        assert!(url_building_keywords.contains(&"url".to_string()));
        assert!(url_building_keywords.contains(&"building".to_string()));
        assert!(auto_seed_url_building_task(&url_building_keywords));
        assert!(!auto_seed_request_query_params_task(&url_building_keywords));
        assert!(!auto_seed_route_parameters_task(&url_building_keywords));

        let path_joining_keywords = task_keywords("understand route path joining behavior");
        assert!(path_joining_keywords.contains(&"path".to_string()));
        assert!(path_joining_keywords.contains(&"joining".to_string()));
        assert!(auto_seed_url_building_task(&path_joining_keywords));

        let url_parameter_keywords = task_keywords("understand url parameter behavior");
        assert!(url_parameter_keywords.contains(&"url".to_string()));
        assert!(url_parameter_keywords.contains(&"parameter".to_string()));
        assert!(!auto_seed_url_building_task(&url_parameter_keywords));

        let blueprint_keywords = task_keywords("understand flask blueprint routing behavior");
        assert!(blueprint_keywords.contains(&"blueprint".to_string()));
        assert!(blueprint_keywords.contains(&"route".to_string()));
        assert!(auto_seed_route_grouping_task(&blueprint_keywords));
        assert!(!auto_seed_url_building_task(&blueprint_keywords));
        assert!(!auto_seed_route_parameters_task(&blueprint_keywords));

        let mounted_router_keywords =
            task_keywords("understand express mounted app router behavior");
        assert!(mounted_router_keywords.contains(&"mounted".to_string()));
        assert!(mounted_router_keywords.contains(&"router".to_string()));
        assert!(auto_seed_route_grouping_task(&mounted_router_keywords));
        assert!(!auto_seed_http_method_routing_task(
            &mounted_router_keywords
        ));

        let route_group_keywords = task_keywords("understand gin route group behavior");
        assert!(route_group_keywords.contains(&"group".to_string()));
        assert!(route_group_keywords.contains(&"route".to_string()));
        assert!(auto_seed_route_grouping_task(&route_group_keywords));

        let route_miss_keywords = task_keywords("understand gin no route no method behavior");
        assert!(route_miss_keywords.contains(&"route".to_string()));
        assert!(route_miss_keywords.contains(&"method".to_string()));
        assert!(auto_seed_route_miss_handling_task(&route_miss_keywords));
        assert!(!auto_seed_http_method_routing_task(&route_miss_keywords));

        let final_handler_keywords =
            task_keywords("understand express 404 not found final handler behavior");
        assert!(final_handler_keywords.contains(&"404".to_string()));
        assert!(final_handler_keywords.contains(&"final".to_string()));
        assert!(auto_seed_route_miss_handling_task(&final_handler_keywords));

        let template_not_found_keywords = task_keywords("understand template not found behavior");
        assert!(template_not_found_keywords.contains(&"template".to_string()));
        assert!(!auto_seed_route_miss_handling_task(
            &template_not_found_keywords
        ));

        let http_method_keywords = task_keywords("understand HTTP method routing behavior");
        assert!(http_method_keywords.contains(&"http".to_string()));
        assert!(http_method_keywords.contains(&"method".to_string()));
        assert!(auto_seed_http_method_routing_task(&http_method_keywords));

        let adapter_method_keywords = task_keywords("understand adapter method behavior");
        assert!(adapter_method_keywords.contains(&"adapter".to_string()));
        assert!(adapter_method_keywords.contains(&"method".to_string()));
        assert!(!auto_seed_http_method_routing_task(
            &adapter_method_keywords
        ));

        let billing_keywords = task_keywords("understand checkout subscription payment");
        assert!(billing_keywords.contains(&"checkout".to_string()));
        assert!(billing_keywords.contains(&"subscription".to_string()));
        assert!(billing_keywords.contains(&"billing".to_string()));
        assert!(billing_keywords.contains(&"payment".to_string()));

        let performance_keywords = task_keywords("understand cache performance latency");
        assert!(performance_keywords.contains(&"cache".to_string()));
        assert!(performance_keywords.contains(&"performance".to_string()));
        assert!(performance_keywords.contains(&"latency".to_string()));
        assert!(performance_keywords.contains(&"optimization".to_string()));

        let observability_keywords = task_keywords("understand observability telemetry logs");
        assert!(observability_keywords.contains(&"observability".to_string()));
        assert!(observability_keywords.contains(&"telemetry".to_string()));
        assert!(observability_keywords.contains(&"logging".to_string()));
        assert!(observability_keywords.contains(&"metrics".to_string()));

        let cookie_keywords = task_keywords("understand cookie jar behavior");
        assert!(cookie_keywords.contains(&"cookie".to_string()));
        assert!(cookie_keywords.contains(&"cookies".to_string()));
        assert!(cookie_keywords.contains(&"cookiejar".to_string()));
        assert!(cookie_keywords.contains(&"jar".to_string()));

        let header_keywords = task_keywords("understand headers case insensitive behavior");
        assert!(header_keywords.contains(&"headers".to_string()));
        assert!(header_keywords.contains(&"header".to_string()));
        assert!(header_keywords.contains(&"case".to_string()));
        assert!(header_keywords.contains(&"insensitive".to_string()));

        let security_keywords = task_keywords("understand security sanitization vulnerabilities");
        assert!(security_keywords.contains(&"security".to_string()));
        assert!(security_keywords.contains(&"sanitize".to_string()));
        assert!(security_keywords.contains(&"sanitization".to_string()));
        assert!(security_keywords.contains(&"vulnerability".to_string()));

        let runtime_keywords = task_keywords("understand script runner lifecycle");
        assert!(runtime_keywords.contains(&"script".to_string()));
        assert!(runtime_keywords.contains(&"runner".to_string()));
        assert!(runtime_keywords.contains(&"runtime".to_string()));
        assert!(runtime_keywords.contains(&"lifecycle".to_string()));

        let upload_keywords = task_keywords("understand uploaded file manager behavior");
        assert!(upload_keywords.contains(&"uploaded".to_string()));
        assert!(upload_keywords.contains(&"upload".to_string()));
        assert!(upload_keywords.contains(&"uploader".to_string()));
        assert!(upload_keywords.contains(&"file".to_string()));

        let websocket_keywords = task_keywords("understand websocket connection behavior");
        assert!(websocket_keywords.contains(&"websocket".to_string()));
        assert!(websocket_keywords.contains(&"socket".to_string()));
        assert!(websocket_keywords.contains(&"connection".to_string()));
        assert!(!websocket_keywords.contains(&"session".to_string()));
    }

    #[test]
    fn context_text_mentions_matches_words_and_phrases_without_substrings() {
        assert!(context_text_mentions("understand log behavior", &["log"]));
        assert!(context_text_mentions(
            "understand access-control rules",
            &["access control"]
        ));
        assert!(context_text_mentions(
            "understand feature-flag rollout",
            &["feature flag"]
        ));
        assert!(!context_text_mentions(
            "understand catalog behavior",
            &["log"]
        ));
        assert!(!context_text_mentions(
            "understand asyncatalog behavior",
            &["async"]
        ));
    }

    #[test]
    fn request_session_flow_uses_network_signal_without_auth_session() {
        let requests_session =
            ContextTaskSignals::from_task("understand requests session request flow");
        assert!(requests_session.network_http);
        assert!(!requests_session.auth_session);
        assert!(context_seed_file_focus(requests_session).contains("network client"));
        assert!(
            context_seed_file_question("understand requests session request flow")
                .contains("network requests")
        );

        let secure_session =
            ContextTaskSignals::from_task("understand flask session cookie security behavior");
        assert!(secure_session.auth_session);
        assert!(!secure_session.network_http);
        assert!(context_seed_file_focus(secure_session).contains("authentication"));
    }

    #[test]
    fn streamlit_runtime_tasks_use_specific_context_prompts() {
        let script_runner =
            ContextTaskSignals::from_task("understand streamlit script runner lifecycle");
        assert!(script_runner.runtime_lifecycle);
        assert!(!script_runner.request_lifecycle);
        assert!(context_seed_file_focus(script_runner).contains("script runner"));
        assert!(
            context_seed_file_question("understand streamlit script runner lifecycle")
                .contains("coordinate reruns")
        );
        assert!(context_symbol_definition_focus(script_runner).contains("runtime execution"));
        assert!(context_call_graph_focus(script_runner).contains("script runner"));
        assert!(context_reference_focus(script_runner).contains("coordinate reruns"));
        assert!(context_dependency_focus(script_runner).contains("runtime execution"));
        assert!(
            context_semantic_question("understand streamlit script runner lifecycle")
                .contains("script runner lifecycle")
        );

        let uploaded_file =
            ContextTaskSignals::from_task("understand streamlit uploaded file manager behavior");
        assert!(uploaded_file.file_upload);
        assert!(context_seed_file_focus(uploaded_file).contains("uploaded file"));
        assert!(
            context_seed_file_question("understand streamlit uploaded file manager behavior")
                .contains("uploaded files stored")
        );
        assert!(context_symbol_definition_focus(uploaded_file).contains("uploaded file"));
        assert!(context_call_graph_focus(uploaded_file).contains("uploaded file"));
        assert!(context_reference_focus(uploaded_file).contains("uploaded files"));
        assert!(
            context_dependency_question("understand streamlit uploaded file manager behavior")
                .contains("uploaded file storage")
        );
        assert!(context_semantic_focus(uploaded_file).contains("uploaded file storage"));

        let websocket =
            ContextTaskSignals::from_task("understand streamlit websocket connection behavior");
        assert!(websocket.websocket_connection);
        assert!(context_seed_file_focus(websocket).contains("WebSocket connection"));
        assert!(
            context_seed_file_question("understand streamlit websocket connection behavior")
                .contains("WebSocket connections opened")
        );
        assert!(context_symbol_definition_focus(websocket).contains("WebSocket connection"));
        assert!(
            context_call_graph_question("understand streamlit websocket connection behavior")
                .contains("WebSocket connections")
        );
        assert!(
            context_reference_question("understand streamlit websocket connection behavior")
                .contains("WebSocket connections")
        );
        assert!(context_dependency_focus(websocket).contains("WebSocket connection"));
        assert!(
            context_semantic_question("understand streamlit websocket connection behavior")
                .contains("WebSocket connections")
        );
    }

    #[test]
    fn dependency_injection_tasks_use_dependency_prompts_without_security_override() {
        let signals =
            ContextTaskSignals::from_task("understand fastapi dependency injection behavior");
        assert!(signals.dependency_injection);
        assert!(!signals.security_safety);
        assert!(context_seed_file_focus(signals).contains("dependency injection"));
        assert!(
            context_seed_file_question("understand fastapi dependency injection behavior")
                .contains("dependencies declared")
        );
        assert!(
            context_call_graph_question("understand fastapi dependency injection behavior")
                .contains("inject")
        );
    }

    #[test]
    fn websocket_tasks_prioritize_source_files_over_docs_examples() {
        let keywords = task_keywords("understand fastapi websocket behavior");
        assert!(auto_seed_websocket_connection_task(&keywords));
        assert!(
            auto_seed_websocket_file_priority("fastapi/websockets.py")
                > auto_seed_websocket_file_priority("docs_src/websockets_/tutorial001_py310.py")
        );
    }

    #[test]
    fn request_lifecycle_tasks_prioritize_handler_source_files() {
        let keywords = task_keywords("understand django request response handler lifecycle");
        assert!(auto_seed_request_lifecycle_task(&keywords));
        assert!(
            auto_seed_request_lifecycle_file_priority("django/core/handlers/base.py")
                > auto_seed_request_lifecycle_file_priority(
                    "django/db/models/fields/related_descriptors.py"
                )
        );
    }

    #[test]
    fn middleware_tasks_prioritize_framework_middleware_sources() {
        let keywords = task_keywords("understand django middleware execution behavior");
        assert!(auto_seed_middleware_task(&keywords));
        assert!(
            auto_seed_middleware_file_priority("django/core/handlers/base.py")
                > auto_seed_middleware_file_priority(
                    "django/db/models/fields/related_descriptors.py"
                )
        );
        assert!(
            auto_seed_middleware_file_priority("django/core/handlers/base.py")
                > auto_seed_middleware_file_priority("django/contrib/admindocs/middleware.py")
        );
        assert!(
            auto_seed_middleware_file_priority("fastapi/applications.py")
                > auto_seed_middleware_file_priority("fastapi/middleware/__init__.py")
        );
        assert!(
            auto_seed_middleware_file_priority("django/middleware/security.py")
                > auto_seed_middleware_file_priority("tests/middleware/test_security.py")
        );
    }

    #[test]
    fn http_response_operation_tasks_prioritize_framework_context_files() {
        let body_keywords = task_keywords("understand gin request body parsing behavior");
        assert!(auto_seed_request_body_parsing_task(&body_keywords));
        assert!(
            auto_seed_request_body_parsing_file_priority("context.go")
                > auto_seed_request_body_parsing_file_priority("binding/binding.go")
        );
        assert!(
            auto_seed_request_body_parsing_file_priority("lib/express.js")
                > auto_seed_request_body_parsing_file_priority("lib/application.js")
        );

        let redirect_keywords = task_keywords("understand gin redirect response behavior");
        assert!(auto_seed_response_redirect_task(&redirect_keywords));
        assert!(
            auto_seed_response_redirect_file_priority("context.go")
                > auto_seed_response_redirect_file_priority("render/redirect.go")
        );

        let header_keywords = task_keywords("understand gin response header behavior");
        assert!(auto_seed_response_headers_task(&header_keywords));
        assert!(
            auto_seed_response_headers_file_priority("context.go")
                > auto_seed_response_headers_file_priority("render/render.go")
        );

        let cookie_keywords = task_keywords("understand gin response cookie behavior");
        assert!(auto_seed_response_cookies_task(&cookie_keywords));
        assert!(
            auto_seed_response_cookies_file_priority("context.go")
                > auto_seed_response_cookies_file_priority("response_writer.go")
        );
    }

    #[test]
    fn binding_validation_tasks_prioritize_specific_binding_files() {
        let validation_keywords = task_keywords("understand binding validation behavior");
        assert!(auto_seed_binding_validation_task(&validation_keywords));
        assert!(
            auto_seed_binding_validation_file_priority_for_task(
                "binding/default_validator.go",
                &validation_keywords
            ) > auto_seed_binding_validation_file_priority_for_task(
                "binding/binding.go",
                &validation_keywords
            )
        );

        let json_keywords = task_keywords("understand json binding behavior");
        assert!(auto_seed_binding_validation_task(&json_keywords));
        assert!(
            auto_seed_binding_validation_file_priority_for_task("binding/json.go", &json_keywords)
                > auto_seed_binding_validation_file_priority_for_task(
                    "binding/default_validator.go",
                    &json_keywords
                )
        );

        let coverage_keywords = task_keywords("understand binding validation test coverage");
        assert!(auto_seed_binding_validation_task(&coverage_keywords));
        assert!(
            auto_seed_binding_validation_file_priority_for_task(
                "binding/default_validator_test.go",
                &coverage_keywords
            ) > auto_seed_binding_validation_file_priority_for_task(
                "binding/default_validator.go",
                &coverage_keywords
            )
        );
    }

    #[test]
    fn request_context_handler_chain_tasks_use_request_lifecycle_routing() {
        let keywords = task_keywords("understand gin request context handler chain behavior");
        assert!(auto_seed_request_lifecycle_task(&keywords));
    }

    #[test]
    fn generic_routing_uses_route_dispatch_signal_without_overriding_specific_route_tasks() {
        let app_routing =
            ContextTaskSignals::from_task("understand express application routing behavior");
        assert!(!app_routing.agent_first_read);
        assert!(app_routing.route_dispatch);
        assert!(!app_routing.http_method_routing);
        assert!(!app_routing.route_grouping);
        assert!(!app_routing.route_miss_handling);
        assert!(context_seed_file_focus(app_routing).contains("route registration"));
        assert!(
            context_seed_file_question("understand express application routing behavior")
                .contains("routes registered")
        );

        let method_routing =
            ContextTaskSignals::from_task("understand express HTTP method routing behavior");
        assert!(!method_routing.agent_first_read);
        assert!(method_routing.http_method_routing);
        assert!(!method_routing.route_dispatch);

        let route_miss =
            ContextTaskSignals::from_task("understand gin no route no method behavior");
        assert!(!route_miss.agent_first_read);
        assert!(route_miss.route_miss_handling);
        assert!(!route_miss.route_dispatch);

        let route_group = ContextTaskSignals::from_task("understand gin route group behavior");
        assert!(!route_group.agent_first_read);
        assert!(route_group.route_grouping);
        assert!(!route_group.route_dispatch);

        let static_route =
            ContextTaskSignals::from_task("understand streamlit static route serving behavior");
        assert!(static_route.static_file_serving);
        assert!(!static_route.route_dispatch);
        assert!(!auto_seed_route_dispatch_task(&task_keywords(
            "understand streamlit static route serving behavior"
        )));

        let django_url_routing =
            ContextTaskSignals::from_task("understand django URL routing behavior");
        assert!(django_url_routing.route_dispatch);
        assert!(auto_seed_route_dispatch_task(&task_keywords(
            "understand django URL routing behavior"
        )));
        let django_keywords = task_keywords("understand django URL routing behavior");
        let express_keywords = task_keywords("understand express routing behavior");
        let routing_keywords = task_keywords("understand routing behavior");
        assert!(
            auto_seed_route_dispatch_file_priority("django/urls/resolvers.py", &django_keywords)
                > auto_seed_route_dispatch_file_priority(
                    "django/core/checks/urls.py",
                    &django_keywords
                )
        );
        assert!(
            auto_seed_route_dispatch_file_priority("lib/express.js", &express_keywords)
                > auto_seed_route_dispatch_file_priority("lib/router/index.js", &express_keywords)
        );
        assert!(
            auto_seed_route_dispatch_file_priority("src/router.ts", &routing_keywords)
                > auto_seed_route_dispatch_file_priority("src/application.ts", &routing_keywords)
        );
    }

    #[test]
    fn agent_first_read_tasks_use_context_routing_prompt() {
        let first_read =
            ContextTaskSignals::from_task("improve AI agent first-read routing quality evidence");
        assert!(first_read.agent_first_read);
        assert!(!first_read.route_dispatch);
        assert!(context_seed_file_focus(first_read).contains("first-read handoff"));
        assert!(
            context_seed_file_question("improve AI agent first-read routing quality evidence")
                .contains("agent first-read workflow")
        );
        assert!(
            ContextTaskSignals::from_task("understand reading plan suggested tool handoff")
                .agent_first_read
        );
        assert!(auto_seed_agent_first_read_task(&task_keywords(
            "understand reading plan suggested tool handoff"
        )));
        assert!(
            ContextTaskSignals::from_task("understand omitted candidate follow up")
                .agent_first_read
        );
        assert!(auto_seed_agent_first_read_task(&task_keywords(
            "understand omitted candidate follow up"
        )));
        assert!(
            ContextTaskSignals::from_task("understand source line reduction metrics")
                .agent_first_read
        );
        assert!(auto_seed_agent_first_read_task(&task_keywords(
            "understand source line reduction metrics"
        )));
        let current_step = ContextTaskSignals::from_task("understand current reading step mirror");
        assert!(current_step.current_reading_step_contract);
        assert!(current_step.agent_first_read);
        assert!(auto_seed_agent_first_read_task(&task_keywords(
            "understand current reading step mirror"
        )));

        let keywords = task_keywords("improve AI agent first-read routing quality evidence");
        assert!(auto_seed_agent_first_read_task(&keywords));
        assert!(
            auto_seed_task_match_score(
                "src/agent_workflow.ts",
                Some("routeAgentFirstReadWorkflow"),
                &keywords
            ) + auto_seed_task_focus_boost(
                "src/agent_workflow.ts",
                Some("routeAgentFirstReadWorkflow"),
                &keywords,
                false,
            ) > auto_seed_task_match_score("src/router.ts", Some("createRouter"), &keywords)
                + auto_seed_task_focus_boost(
                    "src/router.ts",
                    Some("createRouter"),
                    &keywords,
                    false
                )
        );
    }

    #[test]
    fn semantic_provider_fallback_tasks_use_provider_prompt() {
        let fallback =
            ContextTaskSignals::from_task("understand semantic provider disabled fallback");
        assert!(fallback.semantic_provider_fallback);
        assert!(!fallback.semantic_index_explain);
        assert!(context_seed_file_focus(fallback).contains("semantic provider fallback"));
        assert!(
            context_seed_file_question("understand semantic provider disabled fallback")
                .contains("disabled semantic provider")
        );
        assert!(auto_seed_semantic_provider_fallback_task(&task_keywords(
            "understand semantic provider disabled fallback"
        )));
        assert!(!auto_seed_semantic_provider_fallback_task(&task_keywords(
            "understand embedding provider status reporting"
        )));
    }

    #[test]
    fn context_range_reason_merge_caps_repeated_signal_noise() {
        let mut ranges_by_file = BTreeMap::new();
        push_context_range(
            &mut ranges_by_file,
            "src/tools.rs".to_string(),
            1,
            40,
            "Call graph target of seed via first".to_string(),
            "call_graph",
            75,
        );

        for index in 0..80 {
            push_context_range(
                &mut ranges_by_file,
                "src/tools.rs".to_string(),
                1,
                40,
                format!("Call graph target of seed via helper_{index}"),
                "call_graph",
                75,
            );
        }

        let reason = &ranges_by_file["src/tools.rs"][0].reason;
        assert!(reason.len() <= CONTEXT_RANGE_REASON_MAX_BYTES + 64);
        assert!(reason.contains(CONTEXT_RANGE_REASON_OMITTED));
    }

    #[test]
    fn prefers_graph_rich_context_candidates_over_semantic_only_candidates() {
        let call_graph_ranges = vec![ContextCandidateRange {
            start_line: 1,
            end_line: 2,
            reason: "call graph evidence".to_string(),
            source: "call_graph".to_string(),
            score: 70,
        }];
        let semantic_ranges = vec![ContextCandidateRange {
            start_line: 1,
            end_line: 2,
            reason: "semantic evidence".to_string(),
            source: "semantic".to_string(),
            score: 70,
        }];
        let call_graph_candidate = ContextFileCandidate {
            seed_order: None,
            file: "src/call.ts".to_string(),
            max_score: 70,
            source_mix_score: context_range_source_mix_score(&call_graph_ranges),
            recent_edit_score: 0,
            total_score: 70,
            ranges: call_graph_ranges,
        };
        let semantic_candidate = ContextFileCandidate {
            seed_order: None,
            file: "src/semantic.ts".to_string(),
            max_score: 70,
            source_mix_score: context_range_source_mix_score(&semantic_ranges),
            recent_edit_score: 0,
            total_score: 70,
            ranges: semantic_ranges,
        };

        assert_eq!(
            compare_context_file_candidates(&call_graph_candidate, &semantic_candidate),
            Ordering::Less
        );
        assert_eq!(
            compare_context_file_candidates(&semantic_candidate, &call_graph_candidate),
            Ordering::Greater
        );
    }

    #[test]
    fn semantic_embeddings_for_chunks_batches_provider_requests() {
        let provider = RecordingEmbeddingProvider::default();
        let chunks = (1..=5)
            .map(|id| SemanticChunk {
                id,
                file: format!("src/file{id}.rs"),
                start_line: 1,
                end_line: 1,
                token_estimate: 1,
                text: format!("chunk {id}"),
            })
            .collect::<Vec<_>>();

        let embeddings = semantic_embeddings_for_chunks(&provider, &chunks, 2).unwrap();

        assert_eq!(
            provider.calls.lock().unwrap().as_slice(),
            &[
                vec!["chunk 1".to_string(), "chunk 2".to_string()],
                vec!["chunk 3".to_string(), "chunk 4".to_string()],
                vec!["chunk 5".to_string()],
            ]
        );
        assert_eq!(
            embeddings
                .iter()
                .map(|embedding| embedding.chunk_id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
        assert_eq!(embeddings[4].vector, vec![7.0]);
    }

    #[test]
    fn semantic_embeddings_for_chunks_reports_batch_mismatch() {
        let provider = ShortEmbeddingProvider;
        let chunks = (1..=3)
            .map(|id| SemanticChunk {
                id,
                file: format!("src/file{id}.rs"),
                start_line: 1,
                end_line: 1,
                token_estimate: 1,
                text: format!("chunk {id}"),
            })
            .collect::<Vec<_>>();

        let error = semantic_embeddings_for_chunks(&provider, &chunks, 2).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("returned 1 vectors for 2 chunks in batch 1")
        );
    }

    #[derive(Default)]
    struct RecordingEmbeddingProvider {
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl embedding::EmbeddingProvider for RecordingEmbeddingProvider {
        fn provider_name(&self) -> &str {
            "recording"
        }

        fn model_name(&self) -> &str {
            "recording-v1"
        }

        fn embed(&self, inputs: &[String]) -> Result<Vec<embedding::Embedding>> {
            self.calls.lock().unwrap().push(inputs.to_vec());
            Ok(inputs
                .iter()
                .map(|input| embedding::Embedding {
                    values: vec![input.len() as f32],
                })
                .collect())
        }
    }

    struct ShortEmbeddingProvider;

    impl embedding::EmbeddingProvider for ShortEmbeddingProvider {
        fn provider_name(&self) -> &str {
            "short"
        }

        fn model_name(&self) -> &str {
            "short-v1"
        }

        fn embed(&self, _inputs: &[String]) -> Result<Vec<embedding::Embedding>> {
            Ok(vec![embedding::Embedding { values: vec![1.0] }])
        }
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

#[derive(Debug, Default)]
struct ReferenceLineScanner {
    in_block_comment: bool,
}

impl ReferenceLineScanner {
    fn code_mask(&mut self, line: &str) -> Vec<bool> {
        let bytes = line.as_bytes();
        let mut mask = vec![false; bytes.len()];
        let mut index = 0;
        let mut quote: Option<u8> = None;
        let mut escaped = false;

        while index < bytes.len() {
            let byte = bytes[index];
            let next = bytes.get(index + 1).copied();

            if self.in_block_comment {
                if byte == b'*' && next == Some(b'/') {
                    self.in_block_comment = false;
                    index += 2;
                } else {
                    index += 1;
                }
                continue;
            }

            if let Some(active_quote) = quote {
                if escaped {
                    escaped = false;
                } else if byte == b'\\' {
                    escaped = true;
                } else if byte == active_quote {
                    quote = None;
                }
                index += 1;
                continue;
            }

            if byte == b'/' && next == Some(b'*') {
                self.in_block_comment = true;
                index += 2;
                continue;
            }
            if byte == b'/' && next == Some(b'/') {
                break;
            }
            if byte == b'#' && !is_code_hash_line(line) {
                break;
            }
            if matches!(byte, b'\'' | b'"' | b'`') {
                quote = Some(byte);
                escaped = false;
                index += 1;
                continue;
            }

            mask[index] = true;
            index += 1;
        }

        mask
    }
}

fn is_code_hash_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("#define ")
        || trimmed.starts_with("#include ")
        || trimmed.starts_with("#[")
        || trimmed.starts_with("#![")
}

fn is_code_reference_column(code_mask: &[bool], column: usize, symbol_len: usize) -> bool {
    column < code_mask.len()
        && column
            .checked_add(symbol_len)
            .is_some_and(|end| end <= code_mask.len())
        && code_mask[column..column + symbol_len]
            .iter()
            .any(|is_code| *is_code)
}

fn looks_like_definition(line: &str, symbol: &str) -> bool {
    let trimmed = strip_definition_modifiers(line.trim_start());
    [
        "#define",
        "fn",
        "def",
        "function",
        "class",
        "struct",
        "interface",
        "type",
        "const",
        "let",
        "var",
    ]
    .iter()
    .any(|keyword| starts_with_declaration(trimmed, keyword, symbol))
}

fn strip_definition_modifiers(mut line: &str) -> &str {
    loop {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pub ") {
            line = rest;
        } else if let Some(rest) = trimmed
            .strip_prefix("pub(")
            .and_then(|rest| rest.split_once(')').map(|(_, after)| after))
        {
            line = rest;
        } else if let Some(rest) = trimmed.strip_prefix("export ") {
            line = rest;
        } else if let Some(rest) = trimmed.strip_prefix("default ") {
            line = rest;
        } else if let Some(rest) = trimmed.strip_prefix("async ") {
            line = rest;
        } else if let Some(rest) = trimmed.strip_prefix("unsafe ") {
            line = rest;
        } else if let Some(rest) = trimmed.strip_prefix("extern ") {
            line = rest;
        } else if let Some(rest) = trimmed.strip_prefix("inline ") {
            line = rest;
        } else {
            return trimmed;
        }
    }
}

fn starts_with_declaration(line: &str, keyword: &str, symbol: &str) -> bool {
    let Some(rest) = line.strip_prefix(keyword) else {
        return false;
    };
    let rest = rest.trim_start();
    let Some(after_symbol) = rest.strip_prefix(symbol) else {
        return false;
    };
    after_symbol
        .chars()
        .next()
        .is_none_or(|ch| !is_identifier_char(ch) || matches!(ch, '<' | '(' | ':' | '=' | '{' | '['))
}

fn classify_reference(line: &str, symbol: &str) -> &'static str {
    let trimmed = line.trim_start();
    if looks_like_definition(line, symbol) {
        "definition"
    } else if trimmed.starts_with("import ")
        || trimmed.starts_with("from ")
        || trimmed.starts_with("use ")
        || trimmed.starts_with("#include ")
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

fn confidence_for_reference(line: &str, symbol: &str, file: &str, reference_kind: &str) -> f64 {
    let base = match reference_kind {
        "call" => 0.8,
        "import" => 0.7,
        "definition" => 0.6,
        _ => confidence_for_line(line, symbol),
    };
    if is_low_value_reference_file(file) {
        (base - 0.2_f64).max(0.1)
    } else {
        base
    }
}

fn reference_candidate_score(file: &str, reference_kind: &str, confidence: f64) -> i32 {
    let kind_score = match reference_kind {
        "definition" => 90,
        "call" => 80,
        "import" => 70,
        _ => 40,
    };
    let file_penalty = if is_low_value_reference_file(file) {
        25
    } else {
        0
    };
    kind_score + (confidence * 100.0).round() as i32 - file_penalty
}

fn compare_reference_candidates(left: &ReferenceCandidate, right: &ReferenceCandidate) -> Ordering {
    right
        .score
        .cmp(&left.score)
        .then_with(|| left.reference.file.cmp(&right.reference.file))
        .then_with(|| left.reference.line.cmp(&right.reference.line))
        .then_with(|| left.reference.column.cmp(&right.reference.column))
}

fn is_low_value_reference_file(file: &str) -> bool {
    let normalized = file.replace('\\', "/").to_ascii_lowercase();
    normalized == "test"
        || normalized == "tests"
        || normalized == "spec"
        || normalized == "specs"
        || normalized.starts_with("test/")
        || normalized.starts_with("tests/")
        || normalized.starts_with("spec/")
        || normalized.starts_with("specs/")
        || normalized.contains("/test/")
        || normalized.contains("/tests/")
        || normalized.contains("/spec/")
        || normalized.contains("/specs/")
        || normalized.contains("/__tests__/")
        || normalized.contains("/fixture/")
        || normalized.contains("/fixtures/")
        || normalized.ends_with("_test.go")
        || normalized.ends_with("_test.py")
        || normalized.ends_with("_test.rb")
        || normalized.ends_with("_test.php")
        || normalized.ends_with("_test.rs")
        || normalized.ends_with("_smoke.sh")
        || normalized.ends_with("-smoke.sh")
        || normalized.ends_with(".smoke.sh")
        || normalized.ends_with("_spec.rb")
        || normalized.ends_with("test.java")
        || normalized.ends_with("test.cs")
        || normalized.ends_with("tests.cs")
        || normalized.ends_with(".test.js")
        || normalized.ends_with(".test.jsx")
        || normalized.ends_with(".test.ts")
        || normalized.ends_with(".test.tsx")
        || normalized.ends_with(".spec.js")
        || normalized.ends_with(".spec.jsx")
        || normalized.ends_with(".spec.ts")
        || normalized.ends_with(".spec.tsx")
}

fn print_json<T: serde::Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
