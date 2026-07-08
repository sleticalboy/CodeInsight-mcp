use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};

use crate::{
    embedding, index,
    language::detect_language,
    model::{
        CallEdge, ContextFile, ContextPack, ContextRange, ContextSemanticStatus, Dependency,
        DependencyGraph, EmbeddingProviderStatus, ImpactAnalysisReport, ImpactCounts, ImpactFile,
        ImpactPath, IndexError, Language, OllamaEmbeddingStatus, OpenAiEmbeddingStatus,
        ProjectIndexReport, ProjectOverview, ReferenceMatch, SemanticChunk, SemanticChunkInput,
        SemanticEmbeddingInput, SemanticEmbeddingMatch, SemanticIndexReport, SemanticIndexStatus,
        SemanticSearchResult, SuggestedCheck, Symbol, SymbolKind, VersionInfo,
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
const CONTEXT_MAX_SYMBOL_LINES: usize = 80;
const CONTEXT_MAX_MERGED_RANGE_LINES: usize = 80;

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

    let summary = format!(
        "Impact analysis found {} impacted files from {} symbol seeds and {} file seeds.",
        impacted_files.len(),
        seed_symbols.len(),
        normalized_seed_files.len()
    );
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
    let top_reasons = impact_top_reasons(&impacted_files, 8);
    let suggested_checks =
        impact_suggested_checks(&root, &risk_level, &impacted_files, &paths, &errors);

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
        let mut selected_line_ranges = Vec::new();
        let mut selected_max_score = 0;

        for range in candidate.ranges {
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
                selected_max_score = selected_max_score.max(range.score);
                selected_line_ranges.push((start_line, end_line));
                context_ranges.push(ContextRange {
                    start_line,
                    end_line,
                    importance: importance_for_score(range.score).to_string(),
                    reason: range.reason.clone(),
                    excerpt,
                });
            }
        }

        if !context_ranges.is_empty() {
            context_ranges.sort_by_key(|range| (range.start_line, range.end_line));
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

    semantic_status.status.selected_vector_ranges =
        count_selected_ranges_with_reason(&files, "Semantic vector match");
    semantic_status.status.selected_fallback_ranges =
        count_selected_ranges_with_reason(&files, "Semantic chunk match");
    semantic_status.status.recommendation =
        context_semantic_recommendation(&semantic_status.status);

    Ok(ContextPack {
        task,
        summary,
        semantic_status: semantic_status.status,
        files,
        symbols,
        references,
        estimated_tokens,
        truncated,
    })
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

fn impact_suggested_checks(
    root: &Path,
    risk_level: &str,
    impacted_files: &[ImpactFile],
    paths: &[ImpactPath],
    errors: &[IndexError],
) -> Vec<SuggestedCheck> {
    let languages = impacted_files
        .iter()
        .filter_map(|file| detect_language(Path::new(&file.file)))
        .map(Language::as_str)
        .collect::<BTreeSet<_>>();
    let mut checks = Vec::new();
    let mut seen_commands = BTreeSet::new();

    if languages.contains("rust") && root.join("Cargo.toml").exists() {
        push_command_check(
            &mut checks,
            &mut seen_commands,
            "cargo test --locked",
            "Rust files are impacted and Cargo.toml is present.",
        );
    }

    if languages
        .iter()
        .any(|language| matches!(*language, "javascript" | "typescript" | "tsx"))
    {
        if root.join("pnpm-lock.yaml").exists() {
            push_command_check(
                &mut checks,
                &mut seen_commands,
                "pnpm test",
                "JavaScript or TypeScript files are impacted and pnpm-lock.yaml is present.",
            );
        } else if root.join("yarn.lock").exists() {
            push_command_check(
                &mut checks,
                &mut seen_commands,
                "yarn test",
                "JavaScript or TypeScript files are impacted and yarn.lock is present.",
            );
        } else if root.join("package-lock.json").exists() || root.join("package.json").exists() {
            push_command_check(
                &mut checks,
                &mut seen_commands,
                "npm test",
                "JavaScript or TypeScript files are impacted and package metadata is present.",
            );
        }
    }

    if languages.contains("python")
        && any_root_file_exists(
            root,
            &[
                "pyproject.toml",
                "pytest.ini",
                "setup.cfg",
                "setup.py",
                "tox.ini",
                "requirements.txt",
            ],
        )
    {
        push_command_check(
            &mut checks,
            &mut seen_commands,
            "pytest",
            "Python files are impacted and Python test metadata is present.",
        );
    }

    if languages.contains("go") && root.join("go.mod").exists() {
        push_command_check(
            &mut checks,
            &mut seen_commands,
            "go test ./...",
            "Go files are impacted and go.mod is present.",
        );
    }

    if languages.contains("java") {
        if root.join("pom.xml").exists() {
            push_command_check(
                &mut checks,
                &mut seen_commands,
                "mvn test",
                "Java files are impacted and pom.xml is present.",
            );
        } else if root.join("gradlew").exists() {
            push_command_check(
                &mut checks,
                &mut seen_commands,
                "./gradlew --no-daemon test",
                "Java files are impacted and a Gradle wrapper is present.",
            );
        } else if root.join("build.gradle").exists() || root.join("build.gradle.kts").exists() {
            push_command_check(
                &mut checks,
                &mut seen_commands,
                "gradle test",
                "Java files are impacted and Gradle metadata is present.",
            );
        }
    }

    if languages.contains("csharp") && has_root_child_extension(root, "csproj") {
        push_command_check(
            &mut checks,
            &mut seen_commands,
            "dotnet test",
            "C# files are impacted and a .csproj file is present.",
        );
    }

    if languages.contains("ruby") && root.join("Gemfile").exists() {
        push_command_check(
            &mut checks,
            &mut seen_commands,
            "bundle exec rspec",
            "Ruby files are impacted and Gemfile is present.",
        );
    }

    if languages.contains("php") && root.join("composer.json").exists() {
        push_command_check(
            &mut checks,
            &mut seen_commands,
            "composer test",
            "PHP files are impacted and composer.json is present.",
        );
    }

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

    checks.truncate(8);
    checks
}

fn push_command_check(
    checks: &mut Vec<SuggestedCheck>,
    seen_commands: &mut BTreeSet<String>,
    command: &str,
    reason: &str,
) {
    if seen_commands.insert(command.to_string()) {
        checks.push(SuggestedCheck {
            kind: "command".to_string(),
            command: Some(command.to_string()),
            file: None,
            reason: reason.to_string(),
        });
    }
}

fn any_root_file_exists(root: &Path, files: &[&str]) -> bool {
    files.iter().any(|file| root.join(file).exists())
}

fn has_root_child_extension(root: &Path, extension: &str) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value == extension)
    })
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
    let ranges = ranges_by_file.entry(file).or_default();
    if let Some(existing) = ranges
        .iter_mut()
        .find(|range| range.start_line == start_line && range.end_line == end_line)
    {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn uncovered_segments_keeps_ranges_after_selected_overlap() {
        assert_eq!(uncovered_segments(1, 10, &[(4, 6)]), vec![(1, 3), (7, 10)]);
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
