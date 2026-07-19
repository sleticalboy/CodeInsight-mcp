use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{
    config::{
        ConfiguredSuggestedCheck, init_project_config, load_project_config, project_config_path,
        suggested_test_commands_for_root,
    },
    embedding, index,
    language::detect_language,
    model::{
        AgentRouteExecutionStep, AgentRouteReport, AgentRouteStep, CallEdge, ConfigInitReport,
        ConfigStatusReport, ContextBudget, ContextContinuationSummary, ContextFile,
        ContextOmittedCandidate, ContextPack, ContextRange, ContextReadingRange,
        ContextReadingStep, ContextSeed, ContextSemanticStatus, ContextSuggestedTool, Dependency,
        DependencyGraph, EmbeddingProviderStatus, ImpactAnalysisReport, ImpactBreakdown,
        ImpactCounts, ImpactFile, ImpactPath, IndexError, Language, OllamaEmbeddingStatus,
        OpenAiEmbeddingStatus, ProjectIndexReport, ProjectOverview, ReferenceMatch, SemanticChunk,
        SemanticChunkInput, SemanticEmbeddingInput, SemanticEmbeddingMatch, SemanticIndexReport,
        SemanticIndexStatus, SemanticSearchResult, SuggestedCheck, Symbol, SymbolKind, VersionInfo,
    },
    storage::Store,
};

const CONTEXT_SCORE_SEED_FILE: i32 = 130;
const CONTEXT_SCORE_SEED_HEADER: i32 = 140;
const CONTEXT_SCORE_SYMBOL_DEFINITION: i32 = 90;
const CONTEXT_SCORE_CALL_GRAPH: i32 = 75;
const CONTEXT_SCORE_REFERENCE_BASE: i32 = 60;
const CONTEXT_SCORE_SEMANTIC_CHUNK: i32 = 50;
const CONTEXT_SCORE_SEMANTIC_VECTOR: i32 = 70;
const CONTEXT_SCORE_LOCAL_DEPENDENCY: i32 = 40;
const CONTEXT_SCORE_TASK_MATCH_BOOST: i32 = 30;
const CONTEXT_SCORE_SEED_SYMBOL_TASK_MATCH_BOOST: i32 = 5;
const CONTEXT_SCORE_LOW_VALUE_FILE_PENALTY: i32 = 35;
const CONTEXT_SCORE_LOW_VALUE_FILE_TEST_BOOST: i32 = 35;
const CONTEXT_MAX_SYMBOL_LINES: usize = 80;
const CONTEXT_MAX_MERGED_RANGE_LINES: usize = 80;
const CONTEXT_OMITTED_CANDIDATE_LIMIT: usize = 8;
const CONTEXT_OMITTED_CANDIDATE_RANGE_LIMIT: usize = 4;
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
    limit: usize,
    offset: usize,
) -> Result<()> {
    let graph = dependency_graph_value(root, files, languages, limit, offset)?;
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
    )?;
    print_json(&report)
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
) -> Result<AgentRouteReport> {
    let root = root.canonicalize()?;
    let token_budget = token_budget.max(500);
    let index_report = index_project_value(root.clone(), force_index)?;
    let overview = project_overview_value(root.clone())?;
    let context_pack = context_pack_value(
        root.clone(),
        task.clone(),
        symbols.clone(),
        files.clone(),
        token_budget,
    )?;

    let mut impact_seed_files = files
        .iter()
        .map(|file| normalize_seed_file(&root, file))
        .collect::<Result<Vec<_>>>()?;
    if impact_seed_files.is_empty() {
        if let Some(first_file) = context_pack.files.first() {
            impact_seed_files.push(first_file.file.clone());
        }
    }
    impact_seed_files.sort();
    impact_seed_files.dedup();

    let mut impact_seed_symbols = symbols;
    impact_seed_symbols.sort();
    impact_seed_symbols.dedup();

    let (impact_status, impact_analysis) =
        if impact_seed_files.is_empty() && impact_seed_symbols.is_empty() {
            ("skipped_no_seed".to_string(), None)
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
                "found {} entrypoints and {} recommended next tools",
                overview.entrypoints.len(),
                overview.recommended_next_tools.len()
            ),
        },
        AgentRouteStep {
            order: 3,
            tool: "context_pack".to_string(),
            status: "complete".to_string(),
            reason: agent_route_context_reason(&context_pack),
        },
        AgentRouteStep {
            order: 4,
            tool: "impact_analysis".to_string(),
            status: impact_status.clone(),
            reason: match &impact_analysis {
                Some(report) => agent_route_impact_reason(report),
                None => "skipped because no context file or symbol seed was available".to_string(),
            },
        },
    ];
    let execution_plan =
        agent_route_execution_plan(&context_pack, &impact_status, impact_analysis.as_ref());
    let current_reading_step = context_pack.reading_plan.first().cloned();

    Ok(AgentRouteReport {
        root: root.display().to_string(),
        task,
        token_budget,
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

fn agent_route_execution_plan(
    context_pack: &ContextPack,
    impact_status: &str,
    impact_analysis: Option<&ImpactAnalysisReport>,
) -> Vec<AgentRouteExecutionStep> {
    let reading_files = context_pack
        .reading_plan
        .iter()
        .map(|step| step.file.clone())
        .collect::<Vec<_>>();
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
            Some(step) => format!(
                "Read context_pack.files[] in reading_plan[] order, starting with {} (candidate rank {}) with focus: {} Answer: {} Treat reading_plan[].reason as the current-step instruction and selection_reason as evidence for why each file was selected.",
                step.file, step.selection_rank, step.focus, step.question
            ),
            None => {
                "No reading_plan was produced; narrow the task or provide seed files before broad reading."
                    .to_string()
            }
        },
        files: reading_files,
        suggested_tool: None,
    }];

    if let Some(step) = first_step {
        plan.push(AgentRouteExecutionStep {
            order: 2,
            action: "use_current_reading_step_suggested_tool".to_string(),
            status: "available_after_current_file".to_string(),
            instruction: format!(
                "After reading {}, call {} only if deeper evidence is needed for {} with focus: {} Answer: {}",
                step.file, step.suggested_tool.tool, step.next_action, step.focus, step.question
            ),
            files: vec![step.file.clone()],
            suggested_tool: Some(step.suggested_tool.clone()),
        });
    }

    let continuation = &context_pack.continuation_summary;
    plan.push(AgentRouteExecutionStep {
        order: plan.len() + 1,
        action: "use_continuation_if_needed".to_string(),
        status: match continuation.suggested_tool {
            Some(_) => "available_after_selected_context".to_string(),
            None if continuation.status == "complete" => "complete".to_string(),
            None => "manual_after_selected_context".to_string(),
        },
        instruction: agent_route_continuation_instruction(context_pack),
        files: continuation
            .first_omitted_file
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        suggested_tool: continuation.suggested_tool.clone(),
    });

    plan.push(AgentRouteExecutionStep {
        order: plan.len() + 1,
        action: "review_impact_before_edits".to_string(),
        status: impact_status.to_string(),
        instruction: match impact_analysis {
            Some(report) => format!(
                "Before editing, review impact_analysis: {} impacted files at {} risk.",
                report.impact_counts.impacted_files, report.risk_level
            ),
            None => "Impact analysis was skipped because no file or symbol seed was available."
                .to_string(),
        },
        files: Vec::new(),
        suggested_tool: None,
    });

    plan
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
                "{summary}; no reading_plan step was produced, so narrow the task or seed files"
            )
        }
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

    format!(
        "Use continuation_summary only after selected context has been read. Current continuation status is {} with next_action {}.",
        continuation.status, continuation.next_action
    )
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
    ) = if exists {
        match load_project_config(&root) {
            Ok(Some(config)) => (
                true,
                None,
                config.impact_analysis.test_commands,
                config.impact_analysis.suggested_checks.len(),
                config.javascript.package_conditions,
            ),
            Ok(None) => (false, None, Vec::new(), 0, Vec::new()),
            Err(error) => (false, Some(error.to_string()), Vec::new(), 0, Vec::new()),
        }
    } else {
        (false, None, Vec::new(), 0, Vec::new())
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

pub fn dependency_graph_value(
    root: PathBuf,
    files: Vec<String>,
    languages: Vec<String>,
    limit: usize,
    offset: usize,
) -> Result<DependencyGraph> {
    let root = root.canonicalize()?;
    let files = files
        .iter()
        .map(|file| normalize_seed_file(&root, file))
        .collect::<Result<Vec<_>>>()?;
    let languages = normalize_dependency_languages(&languages)?;
    let store = Store::open(&root)?;
    store.dependency_graph(&root, limit, offset, &files, &languages)
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

    let file_symbols = store.symbols_for_files(&normalized_seed_files, limit)?;
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

    let mut impacted_files = impact
        .into_iter()
        .map(|(file, (score, reasons))| ImpactFile {
            file,
            score,
            reasons: reasons.into_iter().take(8).collect(),
        })
        .collect::<Vec<_>>();
    impacted_files.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.file.cmp(&right.file))
    });
    impacted_files.truncate(limit);

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
    let impact_breakdown = impact_breakdown(&impacted_files, &paths, errors.len());
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
    let mut selected_seeds = explicit_context_seeds(&seed_symbols, &seed_files);
    if auto_seeded {
        let auto_selection = auto_context_seed_files(&store, &root, &task_keywords)?;
        seed_strategy = auto_selection.strategy;
        seed_files = auto_selection.files;
        selected_seeds = auto_selection.seeds;
        if seed_files.is_empty() {
            bail!(
                "context_pack could not infer source seed files from the current index; run index or provide --symbol/--file"
            );
        }
    }

    let mut symbols = Vec::new();
    let mut references = Vec::new();
    let seed_files = seed_files
        .iter()
        .map(|file| normalize_seed_file(&root, file))
        .collect::<Result<Vec<_>>>()?;
    if !auto_seeded {
        selected_seeds = explicit_context_seeds(&seed_symbols, &seed_files);
    }
    let seed_file_set = seed_files.iter().cloned().collect::<BTreeSet<_>>();
    let seed_file_order = seed_files
        .iter()
        .enumerate()
        .map(|(index, file)| (file.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let scoring_policy = ContextScoringPolicy {
        prefer_low_value_files: context_prefers_low_value_files(&task_keywords, &seed_files),
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
        for range in seed_file_ranges(&root, file, &symbols, &task_keywords) {
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

    let mut candidates = ranges_by_file
        .into_iter()
        .map(|(file, ranges)| {
            let total_score = ranges.iter().map(|range| range.score).sum();
            let mut ranges = merge_ranges(ranges);
            ranges.sort_by(compare_context_ranges_for_budget);
            let max_score = ranges.iter().map(|range| range.score).max().unwrap_or(0);
            ContextFileCandidate {
                seed_order: seed_file_order.get(&file).copied(),
                file,
                ranges,
                max_score,
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
                if selected_source.is_none() || range.score > selected_max_score {
                    selected_source = Some(range.source.clone());
                    selected_reason = Some(range.reason.clone());
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
                    "Selected for {} relevance via {}: {}",
                    importance_for_score(selected_max_score),
                    source,
                    selected_reason
                        .unwrap_or_else(|| "selected range matched the task".to_string())
                ),
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
    let reading_plan = context_reading_plan(&root, &task, &files);
    let selected_files = files.len();
    let selected_ranges = files.iter().map(|file| file.ranges.len()).sum::<usize>();
    let omitted_candidates = context_omitted_candidates(
        &root,
        &task,
        &candidates,
        &files,
        truncated,
        CONTEXT_OMITTED_CANDIDATE_LIMIT,
    );
    let budget_summary = ContextBudget {
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
    let continuation_summary = context_continuation_summary(&budget_summary, &omitted_candidates);

    Ok(ContextPack {
        task,
        summary,
        seed_strategy,
        selected_seeds,
        reading_plan,
        semantic_status: semantic_status.status,
        budget: budget_summary,
        continuation_summary,
        omitted_candidates,
        files,
        symbols,
        references,
        estimated_tokens,
        truncated,
    })
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
    if let Some(candidate) = omitted_candidates.first() {
        return ContextContinuationSummary {
            status: "omitted_candidates_available".to_string(),
            message: format!(
                "{} selected files fit the context budget; {} candidate files were omitted. Continue with {} if more context is needed.",
                budget.selected_files, budget.omitted_files, candidate.file
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
                "{} selected files fit the context budget, but some ranges were truncated. Increase token_budget or narrow the task for deeper context.",
                budget.selected_files
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
                "{} lower-ranked files and {} ranges were omitted; use a narrower seed if those signals are needed.",
                budget.omitted_files, budget.omitted_ranges
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

fn context_reading_plan(root: &Path, task: &str, files: &[ContextFile]) -> Vec<ContextReadingStep> {
    files
        .iter()
        .take(8)
        .enumerate()
        .map(|(index, file)| {
            let next_action = context_reading_next_action(file).to_string();
            let question = context_reading_question(file, task);
            let suggested_tool = context_reading_suggested_tool(root, task, file);
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
                focus: context_reading_focus(file, task),
                next_action,
                question: question.clone(),
                reason: context_reading_reason(&question, &suggested_tool, file),
                suggested_tool,
                selection_reason: file.reason.clone(),
                source: file.source.clone(),
                score: file.score,
                ranges,
            }
        })
        .collect()
}

fn context_reading_reason(
    question: &str,
    suggested_tool: &ContextSuggestedTool,
    file: &ContextFile,
) -> String {
    format!(
        "Read this step to answer: {question} If deeper evidence is needed, call {}. Selection reason: {}",
        suggested_tool.tool, file.reason
    )
}

fn context_reading_suggested_tool(
    root: &Path,
    task: &str,
    file: &ContextFile,
) -> ContextSuggestedTool {
    let root_arg = root.display().to_string();
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
    if signals.auth_session {
        "Start with seed file authentication and session boundaries.".to_string()
    } else if signals.configuration {
        "Start with seed file configuration defaults and inputs.".to_string()
    } else if signals.startup {
        "Start with seed file startup and initialization flow.".to_string()
    } else if signals.middleware {
        "Start with seed file middleware and handler boundaries.".to_string()
    } else if signals.request_lifecycle {
        "Start with seed file request lifecycle, dispatch, and response finalization flow."
            .to_string()
    } else if signals.performance_cache {
        "Start with seed file cache, performance, latency, or optimization boundaries.".to_string()
    } else if signals.observability_logging {
        "Start with seed file logging, telemetry, metrics, or tracing boundaries.".to_string()
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
    } else if signals.error_recovery {
        "Start with seed file error handling, retry, and recovery boundaries.".to_string()
    } else if signals.test_coverage {
        "Start with seed file test, spec, or regression coverage.".to_string()
    } else if signals.impact_flow {
        "Start with seed file calls, callees, and impact paths.".to_string()
    } else {
        "Start with seed file context and primary symbols.".to_string()
    }
}

fn context_symbol_definition_focus(signals: ContextTaskSignals) -> String {
    if signals.auth_session {
        "Read symbol definitions that establish authentication or session behavior.".to_string()
    } else if signals.configuration {
        "Read symbol definitions that establish configuration behavior.".to_string()
    } else if signals.startup {
        "Read symbol definitions that establish startup behavior.".to_string()
    } else if signals.middleware {
        "Read symbol definitions that establish middleware boundaries.".to_string()
    } else if signals.request_lifecycle {
        "Read symbol definitions that establish request lifecycle or response finalization behavior."
            .to_string()
    } else if signals.performance_cache {
        "Read symbol definitions that establish cache, performance, or optimization behavior."
            .to_string()
    } else if signals.observability_logging {
        "Read symbol definitions that establish logging, telemetry, metrics, or tracing behavior."
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
    } else if signals.test_coverage {
        "Read symbol definitions that establish test coverage or regression behavior.".to_string()
    } else if signals.impact_flow {
        "Read symbol definitions that anchor call and impact paths.".to_string()
    } else {
        "Read symbol definitions that anchor the requested task.".to_string()
    }
}

fn context_call_graph_focus(signals: ContextTaskSignals) -> String {
    if signals.auth_session {
        "Follow call graph evidence for authentication and session flow.".to_string()
    } else if signals.configuration {
        "Follow call graph evidence for configuration propagation.".to_string()
    } else if signals.startup {
        "Follow call graph evidence for startup and initialization order.".to_string()
    } else if signals.middleware {
        "Follow call graph evidence for middleware and handler boundaries.".to_string()
    } else if signals.request_lifecycle {
        "Follow call graph evidence for request dispatch, hooks, and response finalization."
            .to_string()
    } else if signals.performance_cache {
        "Follow call graph evidence for cache lookups, latency, or optimization flow.".to_string()
    } else if signals.observability_logging {
        "Follow call graph evidence for logs, metrics, telemetry, or trace spans.".to_string()
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
    } else if signals.data_persistence {
        "Follow call graph evidence for database, repository, or storage flow.".to_string()
    } else if signals.error_recovery {
        "Follow call graph evidence for error propagation, retries, and recovery.".to_string()
    } else if signals.test_coverage {
        "Follow call graph evidence from tests, specs, or regression coverage.".to_string()
    } else if signals.impact_flow {
        "Follow call graph evidence for callers, callees, and impact paths.".to_string()
    } else {
        "Follow static call graph evidence around the seed flow.".to_string()
    }
}

fn context_reference_focus(signals: ContextTaskSignals) -> String {
    if signals.auth_session {
        "Inspect references that consume authentication or session state.".to_string()
    } else if signals.configuration {
        "Inspect references that read or pass configuration values.".to_string()
    } else if signals.startup {
        "Inspect references that register or trigger startup behavior.".to_string()
    } else if signals.middleware {
        "Inspect references that attach or call middleware boundaries.".to_string()
    } else if signals.request_lifecycle {
        "Inspect references that enter, hook into, or finalize request lifecycle flow.".to_string()
    } else if signals.performance_cache {
        "Inspect references that read, write, invalidate, or optimize cached work.".to_string()
    } else if signals.observability_logging {
        "Inspect references that emit, record, or propagate logs, metrics, telemetry, or traces."
            .to_string()
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
    } else if signals.test_coverage {
        "Inspect references that exercise behavior in tests, specs, or regression cases."
            .to_string()
    } else if signals.impact_flow {
        "Inspect references that show production usage and impact paths.".to_string()
    } else {
        "Inspect references that show how the seed is used.".to_string()
    }
}

fn context_semantic_focus(signals: ContextTaskSignals) -> String {
    if signals.auth_session {
        "Review semantic matches for authentication, cookie, or session behavior.".to_string()
    } else if signals.configuration {
        "Review semantic matches for configuration and environment behavior.".to_string()
    } else if signals.startup {
        "Review semantic matches for startup and initialization behavior.".to_string()
    } else if signals.middleware {
        "Review semantic matches for middleware or handler behavior.".to_string()
    } else if signals.request_lifecycle {
        "Review semantic matches for request lifecycle, dispatch, hooks, or response finalization."
            .to_string()
    } else if signals.performance_cache {
        "Review semantic matches for cache behavior, performance, latency, or optimization."
            .to_string()
    } else if signals.observability_logging {
        "Review semantic matches for logging, telemetry, metrics, tracing, or monitoring."
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
    } else if signals.test_coverage {
        "Review semantic matches for test, spec, or regression coverage.".to_string()
    } else {
        "Review semantic matches related to the task wording.".to_string()
    }
}

fn context_dependency_focus(signals: ContextTaskSignals) -> String {
    if signals.auth_session {
        "Check local dependencies that affect authentication or session boundaries.".to_string()
    } else if signals.configuration {
        "Check local dependencies that supply configuration behavior.".to_string()
    } else if signals.startup {
        "Check local dependencies that participate in startup behavior.".to_string()
    } else if signals.middleware {
        "Check local dependencies that shape middleware or handler dispatch.".to_string()
    } else if signals.request_lifecycle {
        "Check local dependencies that shape request dispatch, hooks, or response finalization."
            .to_string()
    } else if signals.performance_cache {
        "Check local dependencies that shape cache, performance, or optimization behavior."
            .to_string()
    } else if signals.observability_logging {
        "Check local dependencies that shape logging, metrics, telemetry, or tracing behavior."
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
    } else if signals.data_persistence {
        "Check local dependencies that supply database or storage behavior.".to_string()
    } else if signals.error_recovery {
        "Check local dependencies that shape failure handling or recovery behavior.".to_string()
    } else if signals.test_coverage {
        "Check local dependencies that support test setup, fixtures, or assertions.".to_string()
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
        "follow_call_graph" => context_call_graph_question(task),
        "inspect_references" => context_reference_question(task),
        "review_semantic_matches" => context_semantic_question(task),
        "inspect_dependency" => context_dependency_question(task),
        _ => "What task-relevant context is present in these selected ranges?".to_string(),
    }
}

fn context_seed_file_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.auth_session {
        "Where are authentication decisions, credentials, or session boundaries handled here?"
            .to_string()
    } else if signals.configuration {
        "Which configuration options, defaults, or environment inputs control the requested behavior?".to_string()
    } else if signals.startup {
        "What startup entrypoint or initialization sequence creates the requested flow?".to_string()
    } else if signals.middleware {
        "Which middleware or handler boundaries shape the requested flow here?".to_string()
    } else if signals.request_lifecycle {
        "Where do request lifecycle hooks, dispatch, and response finalization happen here?"
            .to_string()
    } else if signals.performance_cache {
        "Where are cache reads, invalidation, latency, or optimization decisions handled here?"
            .to_string()
    } else if signals.observability_logging {
        "Where are logs, metrics, telemetry, or trace spans emitted here?".to_string()
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
    } else if signals.error_recovery {
        "Where are errors, retries, timeouts, or recovery decisions handled here?".to_string()
    } else if signals.test_coverage {
        "Which behavior, assertions, fixtures, or regression cases are covered here?".to_string()
    } else if signals.impact_flow {
        "Which local callers, callees, or impact paths in this seed file explain the requested flow?".to_string()
    } else {
        "What entrypoints, exported symbols, or setup code define the main flow here?".to_string()
    }
}

fn context_symbol_definition_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.auth_session {
        "What authentication decisions, credentials, or session boundaries does this definition establish?".to_string()
    } else if signals.configuration {
        "What configuration defaults, inputs, or environment behavior does this definition establish?".to_string()
    } else if signals.startup {
        "What startup or initialization role does this definition establish?".to_string()
    } else if signals.middleware {
        "What middleware or handler boundary does this definition establish?".to_string()
    } else if signals.request_lifecycle {
        "What request lifecycle, dispatch, or response finalization behavior does this definition establish?".to_string()
    } else if signals.performance_cache {
        "What cache, performance, latency, or optimization behavior does this definition establish?"
            .to_string()
    } else if signals.observability_logging {
        "What logging, telemetry, metrics, or tracing behavior does this definition establish?"
            .to_string()
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
    } else if signals.test_coverage {
        "What test behavior, assertion, fixture, or regression case does this definition establish?"
            .to_string()
    } else if signals.impact_flow {
        "What callers, callees, or impact paths does this definition anchor?".to_string()
    } else {
        "What behavior or contract does this definition establish for the task?".to_string()
    }
}

fn context_call_graph_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.auth_session {
        "Which callers or callees carry authentication decisions, credentials, or session state through this flow?".to_string()
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
    } else if signals.performance_cache {
        "Which callers or callees read, write, invalidate, or optimize cached work?".to_string()
    } else if signals.observability_logging {
        "Which callers or callees emit logs, record metrics, or propagate telemetry and traces?"
            .to_string()
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
    } else if signals.data_persistence {
        "Which callers or callees read, write, or persist data through this flow?".to_string()
    } else if signals.error_recovery {
        "Which callers or callees propagate errors, trigger retries, or recover from failures?"
            .to_string()
    } else if signals.test_coverage {
        "Which callers or callees exercise behavior through tests, specs, or regression cases?"
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
    if signals.auth_session {
        "What imported local dependency behavior affects authentication or session boundaries here?"
            .to_string()
    } else if signals.configuration {
        "What imported local dependency behavior supplies configuration defaults, inputs, or environment handling?".to_string()
    } else if signals.startup {
        "What imported local dependency behavior participates in startup or initialization?"
            .to_string()
    } else if signals.middleware {
        "What imported local dependency behavior shapes middleware or handler dispatch?".to_string()
    } else if signals.request_lifecycle {
        "What imported local dependency behavior shapes request lifecycle, dispatch, or response finalization?".to_string()
    } else if signals.performance_cache {
        "What imported local dependency behavior shapes cache, latency, or optimization flow?"
            .to_string()
    } else if signals.observability_logging {
        "What imported local dependency behavior shapes logging, metrics, telemetry, or tracing?"
            .to_string()
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
    } else if signals.data_persistence {
        "What imported local dependency behavior supplies database, repository, or storage access?"
            .to_string()
    } else if signals.error_recovery {
        "What imported local dependency behavior supplies error handling, retry, or timeout behavior?"
            .to_string()
    } else if signals.test_coverage {
        "What imported local dependency behavior supplies test setup, fixtures, or assertions?"
            .to_string()
    } else {
        "What imported local dependency behavior is required to understand this file?".to_string()
    }
}

fn context_reference_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.auth_session {
        "Which references consume authentication decisions, credentials, or session state?"
            .to_string()
    } else if signals.configuration {
        "Which references read, override, or pass configuration values?".to_string()
    } else if signals.startup {
        "Which references register or trigger startup and initialization behavior?".to_string()
    } else if signals.middleware {
        "Which references attach, order, or call middleware and handler boundaries?".to_string()
    } else if signals.request_lifecycle {
        "Which references enter, hook into, or finalize the request lifecycle?".to_string()
    } else if signals.performance_cache {
        "Which references read, write, invalidate, measure, or optimize cache behavior?".to_string()
    } else if signals.observability_logging {
        "Which references emit logs, record metrics, attach spans, or propagate telemetry?"
            .to_string()
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
    } else if signals.test_coverage {
        "Which references exercise behavior through tests, specs, fixtures, or regression cases?"
            .to_string()
    } else if signals.impact_flow {
        "Which references show production usage or impact paths for this seed?".to_string()
    } else {
        "How is the seed symbol used by nearby production code?".to_string()
    }
}

fn context_semantic_question(task: &str) -> String {
    let signals = ContextTaskSignals::from_task(task);
    if signals.auth_session {
        "Which semantic matches describe authentication, credential, cookie, or session behavior?"
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
    } else if signals.performance_cache {
        "Which semantic matches describe cache behavior, performance, latency, or optimization?"
            .to_string()
    } else if signals.observability_logging {
        "Which semantic matches describe logs, metrics, telemetry, tracing, or monitoring?"
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
    } else if signals.test_coverage {
        "Which semantic matches describe tests, specs, fixtures, or regression coverage?"
            .to_string()
    } else {
        "Which task terms are reflected in this semantically related code?".to_string()
    }
}

#[derive(Debug, Clone, Copy)]
struct ContextTaskSignals {
    impact_flow: bool,
    auth_session: bool,
    configuration: bool,
    startup: bool,
    middleware: bool,
    performance_cache: bool,
    observability_logging: bool,
    security_safety: bool,
    billing_payment: bool,
    frontend_ui: bool,
    background_jobs: bool,
    request_lifecycle: bool,
    api_handler: bool,
    documentation: bool,
    data_persistence: bool,
    error_recovery: bool,
    test_coverage: bool,
}

impl ContextTaskSignals {
    fn from_task(task: &str) -> Self {
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

        Self {
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
                    "login",
                    "signin",
                    "permission",
                    "permissions",
                    "session",
                    "cookie",
                    "credential",
                    "credentials",
                    "token",
                    "tokens",
                    "oauth",
                    "jwt",
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
            ),
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
    let text = text.to_ascii_lowercase();
    terms.iter().any(|term| text.contains(term))
}

fn context_reading_sources(file: &ContextFile) -> BTreeSet<&str> {
    let sources = file
        .ranges
        .iter()
        .map(|range| range.source.as_str())
        .collect::<BTreeSet<_>>();
    sources
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

fn impact_breakdown(
    impacted_files: &[ImpactFile],
    paths: &[ImpactPath],
    errors: usize,
) -> ImpactBreakdown {
    let mut seed_files = 0;
    let mut symbol_definition_files = 0;
    let mut reference_files = 0;
    let mut call_related_files = 0;
    let mut dependency_related_files = 0;

    for file in impacted_files {
        if file.reasons.iter().any(|reason| reason == "seed_file") {
            seed_files += 1;
        }
        if file
            .reasons
            .iter()
            .any(|reason| reason.starts_with("symbol_definition:"))
        {
            symbol_definition_files += 1;
        }
        if file
            .reasons
            .iter()
            .any(|reason| reason.starts_with("reference:"))
        {
            reference_files += 1;
        }
        if file.reasons.iter().any(|reason| {
            reason.starts_with("caller:")
                || reason.starts_with("caller_depth_")
                || reason.starts_with("callee_source:")
                || reason.starts_with("callee_target:")
        }) {
            call_related_files += 1;
        }
        if file.reasons.iter().any(|reason| {
            reason.starts_with("dependency_source:")
                || reason.starts_with("dependency_target:")
                || reason.starts_with("dependency_importer_depth_")
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
            .filter(|path| path.kind == "dependency")
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
        push_builtin_impact_command_checks(root, &languages, &mut checks, &mut seen_commands);
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
                    .any(|file| file.file == prefix || file.file.starts_with(prefix))
            });
    matches_language && matches_file
}

fn push_builtin_impact_command_checks(
    root: &Path,
    languages: &BTreeSet<&'static str>,
    checks: &mut Vec<SuggestedCheck>,
    seen_commands: &mut BTreeSet<String>,
) {
    for command in suggested_test_commands_for_root(root) {
        let Some(reason) = builtin_impact_command_reason(&command, languages) else {
            continue;
        };
        push_command_check(checks, seen_commands, &command, reason);
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
    total_score: i32,
}

#[derive(Debug, Clone, Copy)]
struct ContextScoringPolicy {
    prefer_low_value_files: bool,
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
    primary_symbols.sort_by_key(|symbol| (symbol.start_line, symbol.end_line));

    for symbol in primary_symbols.into_iter().take(12) {
        let matched_keywords = auto_seed_file_matched_keywords(
            root,
            file,
            Some(&symbol.qualified_name),
            task_keywords,
        );
        ranges.push(ContextCandidateRange {
            start_line: symbol.start_line.saturating_sub(2).max(1),
            end_line: (capped_symbol_end_line(symbol) + 2).min(line_count),
            reason: seed_range_reason(
                &format!("Seed file defines symbol {}", symbol.qualified_name),
                &matched_keywords,
                &seed_request_lifecycle_reasons(file, Some(&symbol.qualified_name), task_keywords),
            ),
            source: "seed_file".to_string(),
            score: CONTEXT_SCORE_SEED_FILE + seed_symbol_task_boost(symbol, task_keywords),
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
        reasons.push("request lifecycle task matched app/application seed file".to_string());
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
        if !existing.reason.contains(&reason) {
            existing.reason.push_str("; ");
            existing.reason.push_str(&reason);
        }
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
            .then_with(|| right.total_score.cmp(&left.total_score))
            .then_with(|| left.file.cmp(&right.file)),
    }
    .then_with(|| {
        right
            .max_score
            .cmp(&left.max_score)
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
    if is_low_value_reference_file(file) && policy.prefer_low_value_files {
        score.saturating_add(CONTEXT_SCORE_LOW_VALUE_FILE_TEST_BOOST)
    } else if is_low_value_reference_file(file) {
        score.saturating_sub(CONTEXT_SCORE_LOW_VALUE_FILE_PENALTY)
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
}

#[derive(Debug, Clone)]
struct AutoSeedCandidate {
    file: String,
    role: String,
    source: String,
    score: i32,
    matched_keywords: Vec<String>,
}

fn auto_context_seed_files(
    store: &Store,
    root: &Path,
    task_keywords: &[String],
) -> Result<AutoContextSeedSelection> {
    let overview = store.overview(root)?;
    let indexed_files = store.indexed_files()?;
    let mut candidates = BTreeMap::<String, AutoSeedCandidate>::new();

    for entrypoint in overview
        .entrypoints
        .iter()
        .filter(|entrypoint| auto_seed_role_allowed(&entrypoint.role, task_keywords))
    {
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
            },
        );
    }

    for file in indexed_files
        .iter()
        .filter(|file| auto_seed_role_allowed(auto_seed_file_role(file), task_keywords))
    {
        let symbols = store.symbols_for_files(std::slice::from_ref(file), 12)?;
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
        upsert_auto_seed_candidate(
            &mut candidates,
            AutoSeedCandidate {
                file: file.clone(),
                role: auto_seed_file_role(file).to_string(),
                source: "task_match".to_string(),
                score: 60 + task_score,
                matched_keywords,
            },
        );
    }

    let mut candidates = candidates.into_values().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.file.cmp(&right.file))
    });

    let selected_candidate = if auto_seed_prefers_entrypoint(task_keywords) {
        candidates.first()
    } else {
        candidates
            .iter()
            .find(|candidate| candidate.source == "task_match")
            .or_else(|| candidates.first())
    };

    if let Some(candidate) = selected_candidate {
        let file = candidate.file.clone();
        let source = candidate.source.clone();
        let strategy = if source == "task_match" {
            "auto_task_match"
        } else {
            "auto_entrypoint"
        };
        let companion_entrypoint = (source == "task_match")
            .then(|| {
                candidates
                    .iter()
                    .find(|entrypoint| {
                        entrypoint.source == "overview_entrypoint"
                            && entrypoint.role == "source"
                            && entrypoint.file != file
                    })
                    .cloned()
            })
            .flatten();
        let mut files = vec![file.clone()];
        let mut seeds = vec![ContextSeed {
            kind: "file".to_string(),
            value: file,
            source,
            role: Some(candidate.role.clone()),
            matched_keywords: candidate.matched_keywords.clone(),
        }];
        if let Some(entrypoint) = companion_entrypoint {
            files.push(entrypoint.file.clone());
            seeds.push(ContextSeed {
                kind: "file".to_string(),
                value: entrypoint.file,
                source: entrypoint.source,
                role: Some(entrypoint.role),
                matched_keywords: entrypoint.matched_keywords,
            });
        }
        return Ok(AutoContextSeedSelection {
            strategy: strategy.to_string(),
            files,
            seeds,
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
            role: Some(auto_seed_file_role(file).to_string()),
            matched_keywords: Vec::new(),
        })
        .collect::<Vec<_>>();

    Ok(AutoContextSeedSelection {
        strategy: "auto_source_fallback".to_string(),
        files,
        seeds,
    })
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
        return;
    }
    if candidate.score > entry.score
        || (candidate.score == entry.score && candidate.source == "task_match")
    {
        *entry = candidate;
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

fn auto_seed_task_focus_boost(
    file: &str,
    symbol: Option<&str>,
    task_keywords: &[String],
    overview_entrypoint: bool,
) -> i32 {
    let mut score = 0;

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

fn auto_seed_request_lifecycle_task(task_keywords: &[String]) -> bool {
    let request_or_response = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "request" | "requests" | "response" | "responses"
        )
    });
    let lifecycle = task_keywords.iter().any(|keyword| {
        matches!(
            keyword.as_str(),
            "lifecycle"
                | "before"
                | "after"
                | "dispatch"
                | "handling"
                | "handle"
                | "handler"
                | "handlers"
        )
    });

    request_or_response && lifecycle
}

fn auto_seed_request_lifecycle_file_matches(file: &str) -> bool {
    auto_seed_file_stem_matches(file, "app") || auto_seed_file_stem_matches(file, "application")
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

fn auto_seed_prefers_entrypoint(task_keywords: &[String]) -> bool {
    (task_keywords
        .iter()
        .any(|keyword| auto_seed_lifecycle_keyword(keyword))
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

fn explicit_context_seeds(seed_symbols: &[String], seed_files: &[String]) -> Vec<ContextSeed> {
    let mut seeds = seed_symbols
        .iter()
        .map(|symbol| ContextSeed {
            kind: "symbol".to_string(),
            value: symbol.clone(),
            source: "explicit".to_string(),
            role: None,
            matched_keywords: Vec::new(),
        })
        .collect::<Vec<_>>();
    seeds.extend(seed_files.iter().map(|file| ContextSeed {
        kind: "file".to_string(),
        value: file.clone(),
        source: "explicit".to_string(),
        role: Some(auto_seed_file_role(file).to_string()),
        matched_keywords: Vec::new(),
    }));
    seeds
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

fn task_keywords(task: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    for word in task
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .map(str::to_ascii_lowercase)
        .filter(|word| word.len() >= 3 && !is_task_stop_word(word))
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
        "url" => &["urls"],
        "urls" => &["url"],
        "startup" => &["start", "boot", "program"],
        "start" => &["startup", "boot"],
        "boot" => &["startup", "start"],
        "authentication" => &["auth", "login"],
        "authenticate" => &["auth", "login"],
        "authz" => &["authorization", "permission"],
        "authorization" => &["authorize", "authz", "permission", "permissions"],
        "authorize" => &["authorization", "authz", "permission"],
        "permission" => &["permissions", "authorization", "authz"],
        "permissions" => &["permission", "authorization", "authz"],
        "token" => &["tokens", "credential", "session"],
        "tokens" => &["token", "credential", "session"],
        "oauth" => &["token", "credential"],
        "jwt" => &["token", "credential"],
        "login" => &["auth", "authentication"],
        "signin" => &["auth", "login"],
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
        "query" => &["queries", "database", "sql"],
        "queries" => &["query", "database", "sql"],
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

fn normalize_dependency_languages(languages: &[String]) -> Result<Vec<String>> {
    languages
        .iter()
        .map(|language| normalize_dependency_language(language))
        .collect()
}

fn normalize_dependency_language(language: &str) -> Result<String> {
    let normalized = language.trim().to_ascii_lowercase();
    let normalized = match normalized.as_str() {
        "js" => "javascript",
        "ts" => "typescript",
        "c++" => "cpp",
        "c#" => "csharp",
        value => value,
    };
    let allowed = [
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn uncovered_segments_keeps_ranges_after_selected_overlap() {
        assert_eq!(uncovered_segments(1, 10, &[(4, 6)]), vec![(1, 3), (7, 10)]);
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

        let coverage_keywords = task_keywords("find regression coverage");
        assert!(coverage_keywords.contains(&"regression".to_string()));
        assert!(coverage_keywords.contains(&"coverage".to_string()));
        assert!(coverage_keywords.contains(&"test".to_string()));

        let docs_keywords = task_keywords("understand documentation usage");
        assert!(docs_keywords.contains(&"docs".to_string()));
        assert!(docs_keywords.contains(&"documentation".to_string()));
        assert!(docs_keywords.contains(&"guide".to_string()));

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

        let frontend_keywords = task_keywords("understand frontend component rendering");
        assert!(frontend_keywords.contains(&"frontend".to_string()));
        assert!(frontend_keywords.contains(&"component".to_string()));
        assert!(frontend_keywords.contains(&"components".to_string()));
        assert!(frontend_keywords.contains(&"ui".to_string()));

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

        let security_keywords = task_keywords("understand security sanitization vulnerabilities");
        assert!(security_keywords.contains(&"security".to_string()));
        assert!(security_keywords.contains(&"sanitize".to_string()));
        assert!(security_keywords.contains(&"sanitization".to_string()));
        assert!(security_keywords.contains(&"vulnerability".to_string()));
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
        format!("#define {symbol}"),
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
        || normalized.starts_with("test/")
        || normalized.starts_with("tests/")
        || normalized.contains("/test/")
        || normalized.contains("/tests/")
        || normalized.contains("/__tests__/")
        || normalized.contains("/fixture/")
        || normalized.contains("/fixtures/")
        || normalized.ends_with("_test.go")
        || normalized.ends_with("_test.py")
        || normalized.ends_with("_test.rb")
        || normalized.ends_with("_test.php")
        || normalized.ends_with("_test.rs")
        || normalized.ends_with("test.java")
        || normalized.ends_with("test.cs")
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
