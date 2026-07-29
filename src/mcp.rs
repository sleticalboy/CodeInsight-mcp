use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{
    cli::Transport,
    model::{
        AgentRouteBackendCandidate, AgentRouteBackendEvidence, AgentRouteBackendStatus,
        ContextSeedLocation,
    },
    tools,
};

const AGENT_FIRST_READ_RESPONSE_TOKEN_BUDGET: usize = 8_000;
const AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT: usize = 8;
const AGENT_FIRST_READ_BACKEND_EVIDENCE_SOURCE_LIMIT: usize = 6;
const AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER: &str = "codebase-memory-mcp";
const CODEBASE_MEMORY_BINARY_ENV: &str = "CODEINSIGHT_CODEBASE_MEMORY_BIN";
const CODEBASE_MEMORY_TIMEOUT_MS: u64 = 60_000;
const CODEBASE_MEMORY_MIN_TIMEOUT_MS: u64 = 1_000;
const CODEBASE_MEMORY_MAX_TIMEOUT_MS: u64 = 300_000;
const CODEBASE_MEMORY_MISSING_PROJECT_ERROR: &str = "project not found or not indexed";
const CODEBASE_MEMORY_EMPTY_SEARCH_ERROR: &str =
    "agent_first_read backend_candidates.search_graph contained no candidate files";

type CodebaseMemoryIndexCacheKey = (PathBuf, String, OsString);

static CODEBASE_MEMORY_INDEX_FINGERPRINTS: OnceLock<
    Mutex<HashMap<CodebaseMemoryIndexCacheKey, String>>,
> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepositoryFingerprint {
    value: String,
    head: String,
    dirty: bool,
}

#[derive(Debug)]
struct CodebaseMemoryAgentFirstReadResult {
    evidence: Option<AgentRouteBackendEvidence>,
    index_status: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct CodebaseMemoryDeadline {
    deadline: Instant,
    budget: Duration,
}

impl CodebaseMemoryDeadline {
    fn new(budget: Duration) -> Self {
        Self {
            deadline: Instant::now() + budget,
            budget,
        }
    }

    fn remaining(self, tool: &str) -> Result<Duration> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or_else(|| self.timeout_error(tool))
    }

    fn exhausted(self) -> bool {
        Instant::now() >= self.deadline
    }

    fn timeout_error(self, tool: &str) -> anyhow::Error {
        anyhow!(
            "codebase-memory-mcp backend timeout budget of {} ms exhausted before or during {tool}",
            self.budget.as_millis()
        )
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentFirstReadBackendCandidates {
    provider: String,
    #[serde(default)]
    candidate_files: Vec<String>,
    #[serde(default)]
    candidates: Vec<AgentFirstReadBackendCandidate>,
    search_graph: Option<Value>,
    #[serde(default)]
    evidence_sources: Vec<String>,
    confidence: Option<f64>,
    latency_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentFirstReadBackendConfig {
    provider: String,
    project: Option<String>,
    #[serde(default)]
    refresh_index: bool,
    #[serde(default = "default_codebase_memory_timeout_ms")]
    timeout_ms: u64,
    #[serde(default)]
    on_failure: AgentFirstReadBackendFailurePolicy,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum AgentFirstReadBackendFailurePolicy {
    Error,
    #[default]
    FallbackLocal,
}

impl AgentFirstReadBackendFailurePolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::FallbackLocal => "fallback_local",
        }
    }
}

impl AgentFirstReadBackendConfig {
    fn validate(&self) -> Result<()> {
        if self.provider != AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER {
            bail!(
                "unsupported agent_first_read backend provider: {}; expected {}",
                self.provider,
                AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER
            );
        }
        if !(CODEBASE_MEMORY_MIN_TIMEOUT_MS..=CODEBASE_MEMORY_MAX_TIMEOUT_MS)
            .contains(&self.timeout_ms)
        {
            bail!(
                "agent_first_read backend.timeout_ms must be between {CODEBASE_MEMORY_MIN_TIMEOUT_MS} and {CODEBASE_MEMORY_MAX_TIMEOUT_MS}"
            );
        }
        if self
            .project
            .as_deref()
            .is_some_and(|project| project.trim().is_empty())
        {
            bail!("agent_first_read backend.project must not be empty");
        }
        Ok(())
    }
}

fn default_codebase_memory_timeout_ms() -> u64 {
    CODEBASE_MEMORY_TIMEOUT_MS
}

#[derive(Debug, Deserialize)]
struct AgentFirstReadBackendCandidate {
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    file_path: Option<String>,
    #[serde(default)]
    symbol: Option<String>,
    #[serde(default)]
    qualified_name: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    end_line: Option<usize>,
}

impl AgentFirstReadBackendCandidate {
    fn into_route_candidate(self) -> Result<AgentRouteBackendCandidate> {
        let normalize = |value: Option<String>| {
            value
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        };
        let file = normalize(self.file)
            .or_else(|| normalize(self.file_path))
            .context("agent_first_read backend candidate must contain file or file_path")?;
        let qualified_name = normalize(self.qualified_name);
        let symbol = normalize(self.symbol)
            .or_else(|| normalize(self.name))
            .or_else(|| {
                qualified_name.as_deref().and_then(|qualified_name| {
                    qualified_name
                        .rsplit(|character: char| ['.', ':', '#'].contains(&character))
                        .find(|part| !part.is_empty())
                        .map(str::to_string)
                })
            });
        let locations = match self.start_line {
            Some(start_line) if start_line > 0 => {
                let end_line = self.end_line.unwrap_or(start_line);
                if end_line < start_line {
                    bail!("agent_first_read backend candidate end_line must be >= start_line");
                }
                vec![ContextSeedLocation {
                    start_line,
                    end_line,
                }]
            }
            Some(_) => bail!("agent_first_read backend candidate start_line must be >= 1"),
            None if self.end_line.is_some() => {
                bail!("agent_first_read backend candidate end_line requires start_line")
            }
            None => Vec::new(),
        };

        Ok(AgentRouteBackendCandidate {
            file,
            symbol,
            locations,
            source: None,
            score: None,
            reason: None,
            evidence: Vec::new(),
        })
    }
}

fn agent_first_read_search_graph_candidates(
    search_graph: &Value,
) -> Result<(Vec<AgentRouteBackendCandidate>, usize, Option<u64>)> {
    let payload = agent_first_read_backend_payload(search_graph);

    let semantic_results = payload
        .get("semantic_results")
        .and_then(Value::as_array)
        .filter(|results| !results.is_empty());
    let results = semantic_results
        .or_else(|| payload.get("results").and_then(Value::as_array))
        .context(
            "agent_first_read backend_candidates.search_graph must contain results or semantic_results",
        )?;
    let candidates = results
        .iter()
        .filter_map(|result| {
            serde_json::from_value::<AgentFirstReadBackendCandidate>(result.clone()).ok()
        })
        .filter_map(|candidate| candidate.into_route_candidate().ok())
        .take(AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        bail!("agent_first_read backend_candidates.search_graph contained no candidate files");
    }
    let evidence_count = candidates.len();
    let latency_ms = payload.get("elapsed_ms").and_then(Value::as_u64);
    Ok((candidates, evidence_count, latency_ms))
}

fn agent_first_read_backend_payload(mut payload: &Value) -> &Value {
    loop {
        if let Some(result) = payload.get("result") {
            payload = result;
            continue;
        }
        if let Some(structured_content) = payload.get("structuredContent") {
            payload = structured_content;
            continue;
        }
        break;
    }
    payload
}

fn codebase_memory_prefers_architecture_entrypoints(task: &str) -> bool {
    let normalized = task.to_lowercase();
    if [
        "入口",
        "启动",
        "架构",
        "代码库",
        "项目结构",
        "代码结构",
        "从哪开始",
        "从哪里开始",
        "从哪里入手",
        "先读哪个文件",
        "先看哪个文件",
        "项目概览",
        "了解项目",
        "理解项目",
    ]
    .iter()
    .any(|phrase| normalized.contains(phrase))
    {
        return true;
    }

    let query = codebase_memory_search_query(task);
    let mut terms = query
        .split_whitespace()
        .map(|term| term.to_ascii_lowercase())
        .peekable();
    terms.peek().is_some() && terms.all(|term| codebase_memory_architecture_term(&term))
}

fn codebase_memory_architecture_term(term: &str) -> bool {
    matches!(
        term,
        "architecture"
            | "begin"
            | "codebase"
            | "entry"
            | "entrypoint"
            | "first"
            | "main"
            | "overview"
            | "project"
            | "read"
            | "repo"
            | "repository"
            | "should"
            | "start"
            | "startup"
            | "structure"
            | "where"
            | "which"
    )
}

fn codebase_memory_search_code_pattern(task: &str) -> Option<String> {
    codebase_memory_search_query(task)
        .split_whitespace()
        .filter(|term| !codebase_memory_architecture_term(&term.to_ascii_lowercase()))
        .max_by_key(|term| {
            let has_identifier_separator = term.contains('_') || term.contains('-');
            let has_case_signal = term.chars().any(char::is_uppercase);
            (has_identifier_separator, has_case_signal, term.len())
        })
        .map(str::to_string)
}

fn agent_first_read_search_code_evidence(search_code: &Value) -> Option<AgentRouteBackendEvidence> {
    let payload = agent_first_read_backend_payload(search_code);
    let mut candidates = payload
        .get("results")?
        .as_array()?
        .iter()
        .filter_map(|result| {
            serde_json::from_value::<AgentFirstReadBackendCandidate>(result.clone()).ok()
        })
        .filter_map(|candidate| candidate.into_route_candidate().ok())
        .take(AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    for candidate in &mut candidates {
        candidate.source = Some("search_code".to_string());
        candidate.reason = Some("graph-augmented code search match".to_string());
    }
    let candidate_files = candidates
        .iter()
        .map(|candidate| candidate.file.clone())
        .collect::<Vec<_>>();
    Some(AgentRouteBackendEvidence {
        provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
        use_as_fallback: false,
        prefer_for_context: true,
        evidence_count: candidates.len(),
        candidate_files,
        candidates,
        evidence_sources: vec!["search_code".to_string()],
        latency_ms: payload.get("elapsed_ms").and_then(Value::as_u64),
        confidence: None,
        notes: Vec::new(),
        tool_results: None,
        normalization: None,
    })
}

fn agent_first_read_architecture_evidence(
    architecture: &Value,
    task: &str,
) -> Option<AgentRouteBackendEvidence> {
    let payload = agent_first_read_backend_payload(architecture);
    let mut candidates = payload
        .get("entry_points")?
        .as_array()?
        .iter()
        .filter_map(|entrypoint| {
            serde_json::from_value::<AgentFirstReadBackendCandidate>(entrypoint.clone()).ok()
        })
        .filter_map(|candidate| candidate.into_route_candidate().ok())
        .take(AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return None;
    }

    for candidate in &mut candidates {
        candidate.source = Some("get_architecture.entry_points".to_string());
        candidate.reason = Some("graph architecture entry point".to_string());
    }
    let candidate_files = candidates
        .iter()
        .map(|candidate| candidate.file.clone())
        .collect::<Vec<_>>();
    let prefer_for_context = codebase_memory_prefers_architecture_entrypoints(task);
    Some(AgentRouteBackendEvidence {
        provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
        use_as_fallback: !prefer_for_context,
        prefer_for_context,
        evidence_count: candidates.len(),
        candidate_files,
        candidates,
        evidence_sources: vec!["get_architecture.entry_points".to_string()],
        latency_ms: payload.get("elapsed_ms").and_then(Value::as_u64),
        confidence: None,
        notes: Vec::new(),
        tool_results: None,
        normalization: None,
    })
}

impl AgentFirstReadBackendCandidates {
    fn into_evidence(self) -> Result<AgentRouteBackendEvidence> {
        let candidate_count = self.candidate_files.len() + self.candidates.len();
        if candidate_count == 0 && self.search_graph.is_none() {
            bail!(
                "agent_first_read backend_candidates must contain candidate_files, candidates, or search_graph"
            );
        }
        if candidate_count > 0 && self.search_graph.is_some() {
            bail!(
                "agent_first_read backend_candidates.search_graph cannot be combined with candidate_files or candidates"
            );
        }
        if candidate_count > AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT {
            bail!(
                "agent_first_read backend_candidates must contain at most {} candidates",
                AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT
            );
        }
        if self.evidence_sources.len() > AGENT_FIRST_READ_BACKEND_EVIDENCE_SOURCE_LIMIT {
            bail!(
                "agent_first_read backend_candidates must contain at most {} evidence sources",
                AGENT_FIRST_READ_BACKEND_EVIDENCE_SOURCE_LIMIT
            );
        }

        let mut candidates = self
            .candidates
            .into_iter()
            .map(AgentFirstReadBackendCandidate::into_route_candidate)
            .collect::<Result<Vec<_>>>()?;
        let mut evidence_count = 0;
        let mut latency_ms = self.latency_ms;
        let mut evidence_sources = self.evidence_sources;
        if let Some(search_graph) = self.search_graph {
            let (search_candidates, search_evidence_count, search_latency_ms) =
                agent_first_read_search_graph_candidates(&search_graph)?;
            candidates = search_candidates;
            evidence_count = search_evidence_count;
            latency_ms = latency_ms.or(search_latency_ms);
            if !evidence_sources
                .iter()
                .any(|source| source == "search_graph")
                && evidence_sources.len() < AGENT_FIRST_READ_BACKEND_EVIDENCE_SOURCE_LIMIT
            {
                evidence_sources.push("search_graph".to_string());
            }
        }
        let mut candidate_files = self.candidate_files;
        for candidate in &candidates {
            if !candidate_files.contains(&candidate.file) {
                candidate_files.push(candidate.file.clone());
            }
        }

        Ok(AgentRouteBackendEvidence {
            provider: self.provider,
            use_as_fallback: false,
            prefer_for_context: true,
            candidate_files,
            candidates,
            evidence_sources,
            evidence_count,
            latency_ms,
            confidence: self.confidence,
            notes: Vec::new(),
            tool_results: None,
            normalization: None,
        })
    }
}

fn agent_first_read_backend_evidence(value: &Value) -> Result<AgentRouteBackendEvidence> {
    if value.get("provider").is_some() {
        return serde_json::from_value::<AgentFirstReadBackendCandidates>(value.clone())
            .context("invalid agent_first_read backend_candidates")?
            .into_evidence();
    }

    AgentFirstReadBackendCandidates {
        provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
        candidate_files: Vec::new(),
        candidates: Vec::new(),
        search_graph: Some(value.clone()),
        evidence_sources: Vec::new(),
        confidence: None,
        latency_ms: None,
    }
    .into_evidence()
}

fn optional_agent_first_read_backend_evidence(
    arguments: &Value,
) -> Result<Option<AgentRouteBackendEvidence>> {
    match arguments.get("backend_candidates") {
        Some(value) if value.is_object() => agent_first_read_backend_evidence(value).map(Some),
        Some(_) => bail!("invalid object argument: backend_candidates"),
        None => Ok(None),
    }
}

fn codebase_memory_cli_value(
    binary: &OsStr,
    tool: &str,
    args: &[OsString],
    timeout: Duration,
) -> Result<Value> {
    let mut child = Command::new(binary)
        .arg("cli")
        .arg(tool)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| {
            format!(
                "failed to run codebase-memory-mcp; install it or set {CODEBASE_MEMORY_BINARY_ENV}"
            )
        })?;
    let stdout = child
        .stdout
        .take()
        .context("failed to capture codebase-memory-mcp stdout")?;
    let stderr = child
        .stderr
        .take()
        .context("failed to capture codebase-memory-mcp stderr")?;
    let stdout_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut stdout = stdout;
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let stderr_reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let mut stderr = stderr;
        stderr.read_to_end(&mut bytes).map(|_| bytes)
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .with_context(|| format!("failed to wait for codebase-memory-mcp {tool}"))?
        {
            break status;
        }
        let now = Instant::now();
        if now >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            let _ = stdout_reader.join();
            let _ = stderr_reader.join();
            bail!(
                "codebase-memory-mcp {tool} timed out after {} ms",
                timeout.as_millis()
            );
        }
        thread::sleep((deadline - now).min(Duration::from_millis(10)));
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| anyhow!("codebase-memory-mcp {tool} stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| anyhow!("codebase-memory-mcp {tool} stderr reader panicked"))??;

    if !status.success() {
        let stdout = String::from_utf8_lossy(&stdout);
        let stderr = String::from_utf8_lossy(&stderr);
        let details = if stdout.trim().is_empty() {
            stderr.trim()
        } else {
            stdout.trim()
        }
        .chars()
        .take(800)
        .collect::<String>();
        bail!(
            "codebase-memory-mcp {tool} failed with status {}{}",
            status,
            if details.is_empty() {
                String::new()
            } else {
                format!(": {details}")
            }
        );
    }
    serde_json::from_slice(&stdout)
        .with_context(|| format!("codebase-memory-mcp {tool} returned invalid JSON"))
}

fn codebase_memory_cli_value_with_deadline(
    binary: &OsStr,
    tool: &str,
    args: &[OsString],
    deadline: CodebaseMemoryDeadline,
) -> Result<Value> {
    let timeout = deadline.remaining(tool)?;
    match codebase_memory_cli_value(binary, tool, args, timeout) {
        Ok(value) => Ok(value),
        Err(_) if deadline.exhausted() => Err(deadline.timeout_error(tool)),
        Err(error) => Err(error),
    }
}

fn optional_codebase_memory_value(
    result: Result<Value>,
    deadline: CodebaseMemoryDeadline,
    tool: &str,
) -> Result<Option<Value>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error)
            if deadline.exhausted() || error.to_string().contains("backend timeout budget of") =>
        {
            Err(deadline.timeout_error(tool))
        }
        Err(_) => Ok(None),
    }
}

fn codebase_memory_project_missing(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .to_string()
            .contains(CODEBASE_MEMORY_MISSING_PROJECT_ERROR)
    })
}

fn git_output(root: &Path, args: &[&str]) -> Result<std::process::Output> {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .with_context(|| format!("failed to inspect Git repository at {}", root.display()))
}

fn successful_git_stdout(root: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = git_output(root, args)?;
    if !output.status.success() {
        bail!(
            "git {} failed with status {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

fn repository_fingerprint(root: &Path) -> Result<Option<RepositoryFingerprint>> {
    let head = git_output(root, &["rev-parse", "--verify", "HEAD"])?;
    if !head.status.success() {
        return Ok(None);
    }
    let head = String::from_utf8(head.stdout)
        .context("git rev-parse HEAD returned non-UTF-8 output")?
        .trim()
        .to_string();
    let tracked_diff = successful_git_stdout(root, &["diff", "--binary", "HEAD", "--"])?;
    let untracked =
        successful_git_stdout(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    let untracked_paths = untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter(|path| *path != b".codeinsight" && !path.starts_with(b".codeinsight/"))
        .collect::<Vec<_>>();

    let mut hasher = Sha256::new();
    hasher.update(head.as_bytes());
    hasher.update([0]);
    hasher.update(&tracked_diff);
    for raw_path in &untracked_paths {
        hasher.update([0]);
        hasher.update(raw_path);
        let relative = PathBuf::from(String::from_utf8_lossy(raw_path).into_owned());
        let contents = std::fs::read(root.join(&relative)).with_context(|| {
            format!(
                "failed to fingerprint untracked file {}",
                relative.display()
            )
        })?;
        hasher.update((contents.len() as u64).to_le_bytes());
        hasher.update(contents);
    }

    Ok(Some(RepositoryFingerprint {
        value: format!("{:x}", hasher.finalize()),
        head,
        dirty: !tracked_diff.is_empty() || !untracked_paths.is_empty(),
    }))
}

fn codebase_memory_index_cache_key(
    root: &Path,
    project: &str,
    binary: &OsStr,
) -> CodebaseMemoryIndexCacheKey {
    (
        root.canonicalize().unwrap_or_else(|_| root.to_path_buf()),
        project.to_string(),
        binary.to_os_string(),
    )
}

fn codebase_memory_index_fingerprint_path(key: &CodebaseMemoryIndexCacheKey) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(key.1.as_bytes());
    hasher.update([0]);
    hasher.update(key.2.to_string_lossy().as_bytes());
    let cache_name = format!("codebase-memory-index-{:x}.fingerprint", hasher.finalize());
    crate::storage::cache_dir(&key.0).join(cache_name)
}

fn valid_repository_fingerprint(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn codebase_memory_cached_index_fingerprint(key: &CodebaseMemoryIndexCacheKey) -> Option<String> {
    let in_memory = CODEBASE_MEMORY_INDEX_FINGERPRINTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .get(key)
        .cloned();
    if in_memory.is_some() {
        return in_memory;
    }

    let persisted = std::fs::read_to_string(codebase_memory_index_fingerprint_path(key))
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| valid_repository_fingerprint(value));
    if let Some(fingerprint) = persisted.as_ref() {
        CODEBASE_MEMORY_INDEX_FINGERPRINTS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(key.clone(), fingerprint.clone());
    }
    persisted
}

fn codebase_memory_cache_index(
    key: CodebaseMemoryIndexCacheKey,
    fingerprint: &RepositoryFingerprint,
) {
    CODEBASE_MEMORY_INDEX_FINGERPRINTS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(key.clone(), fingerprint.value.clone());
    let path = codebase_memory_index_fingerprint_path(&key);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(path, &fingerprint.value);
}

fn codebase_memory_indexed_heads(
    binary: &OsStr,
    project: &str,
    deadline: CodebaseMemoryDeadline,
) -> Result<Vec<String>> {
    let value = codebase_memory_cli_value_with_deadline(
        binary,
        "query_graph",
        &[
            OsString::from("--project"),
            OsString::from(project),
            OsString::from("--query"),
            OsString::from("MATCH (b:Branch) RETURN b.head_sha LIMIT 32"),
        ],
        deadline,
    )?;
    let mut payload = &value;
    loop {
        if let Some(result) = payload.get("result") {
            payload = result;
            continue;
        }
        if let Some(structured_content) = payload.get("structuredContent") {
            payload = structured_content;
            continue;
        }
        break;
    }
    let columns = payload
        .get("columns")
        .and_then(Value::as_array)
        .context("codebase-memory-mcp query_graph response omitted columns")?;
    let head_index = columns
        .iter()
        .position(|column| {
            column
                .as_str()
                .is_some_and(|column| column == "head_sha" || column.ends_with(".head_sha"))
        })
        .context("codebase-memory-mcp query_graph response omitted head_sha")?;
    let rows = payload
        .get("rows")
        .and_then(Value::as_array)
        .context("codebase-memory-mcp query_graph response omitted rows")?;
    Ok(rows
        .iter()
        .filter_map(Value::as_array)
        .filter_map(|row| row.get(head_index))
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect())
}

fn codebase_memory_project(root: &Path, configured: Option<String>) -> Result<String> {
    if let Some(project) = configured {
        let project = project.trim();
        if project.is_empty() {
            bail!("agent_first_read backend.project must not be empty");
        }
        return Ok(project.to_string());
    }
    root.file_name()
        .and_then(OsStr::to_str)
        .filter(|name| !name.trim().is_empty())
        .map(str::to_string)
        .context("agent_first_read backend.project is required when root has no file name")
}

fn codebase_memory_search_query(task: &str) -> String {
    let is_noise = |keyword: &str| {
        matches!(
            keyword,
            "the"
                | "and"
                | "for"
                | "with"
                | "from"
                | "into"
                | "this"
                | "that"
                | "find"
                | "inspect"
                | "understand"
                | "locate"
                | "analyze"
                | "review"
                | "explain"
                | "identify"
                | "show"
                | "code"
                | "file"
                | "module"
                | "implementation"
                | "behavior"
        )
    };
    let mut keywords = task
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter_map(|word| {
            let normalized = word.to_ascii_lowercase();
            (normalized.len() >= 3 && !is_noise(&normalized)).then(|| word.to_string())
        })
        .take(8)
        .collect::<Vec<_>>();
    if !task.is_ascii() && keywords.len() < 2 {
        for keyword in tools::task_keywords(task) {
            if !is_noise(&keyword)
                && !keywords
                    .iter()
                    .any(|existing| existing.eq_ignore_ascii_case(&keyword))
            {
                keywords.push(keyword);
                if keywords.len() == 8 {
                    break;
                }
            }
        }
    }
    if keywords.is_empty() {
        task.trim().to_string()
    } else {
        keywords.join(" ")
    }
}

fn codebase_memory_agent_first_read_result(
    root: &Path,
    task: &str,
    config: AgentFirstReadBackendConfig,
) -> Result<CodebaseMemoryAgentFirstReadResult> {
    let binary = std::env::var_os(CODEBASE_MEMORY_BINARY_ENV)
        .unwrap_or_else(|| OsString::from(AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER));
    codebase_memory_agent_first_read_result_with_binary(root, task, config, &binary)
}

fn codebase_memory_agent_first_read_result_with_binary(
    root: &Path,
    task: &str,
    config: AgentFirstReadBackendConfig,
    binary: &OsStr,
) -> Result<CodebaseMemoryAgentFirstReadResult> {
    config.validate()?;
    let project = codebase_memory_project(root, config.project)?;
    let timeout = Duration::from_millis(config.timeout_ms);
    let deadline = CodebaseMemoryDeadline::new(timeout);
    let search_query = codebase_memory_search_query(task);
    let search_code_pattern = codebase_memory_search_code_pattern(task);
    let prefers_architecture = codebase_memory_prefers_architecture_entrypoints(task);
    let fingerprint = repository_fingerprint(root).ok().flatten();
    let cache_key = codebase_memory_index_cache_key(root, &project, binary);

    let index_project = || {
        codebase_memory_cli_value_with_deadline(
            binary,
            "index_repository",
            &[
                OsString::from("--repo-path"),
                root.as_os_str().to_os_string(),
                OsString::from("--name"),
                OsString::from(&project),
                OsString::from("--mode"),
                OsString::from("fast"),
            ],
            deadline,
        )
    };
    let search_graph = || {
        codebase_memory_cli_value_with_deadline(
            binary,
            "search_graph",
            &[
                OsString::from("--project"),
                OsString::from(&project),
                OsString::from("--query"),
                OsString::from(&search_query),
                OsString::from("--limit"),
                OsString::from(AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT.to_string()),
            ],
            deadline,
        )
    };
    let get_architecture = || {
        codebase_memory_cli_value_with_deadline(
            binary,
            "get_architecture",
            &[
                OsString::from("--project"),
                OsString::from(&project),
                OsString::from("--aspects"),
                OsString::from("entry_points"),
            ],
            deadline,
        )
    };
    let search_code = || -> Result<Option<Value>> {
        let Some(pattern) = search_code_pattern.as_ref() else {
            return Ok(None);
        };
        codebase_memory_cli_value_with_deadline(
            binary,
            "search_code",
            &[
                OsString::from("--project"),
                OsString::from(&project),
                OsString::from("--pattern"),
                OsString::from(pattern),
                OsString::from("--mode"),
                OsString::from("compact"),
                OsString::from("--limit"),
                OsString::from(AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT.to_string()),
            ],
            deadline,
        )
        .map(Some)
    };
    let primary_lookup = || {
        if prefers_architecture {
            get_architecture()
        } else {
            search_graph()
        }
    };
    let cached_fingerprint = codebase_memory_cached_index_fingerprint(&cache_key);
    let cache_matches = fingerprint.as_ref().is_some_and(|fingerprint| {
        cached_fingerprint.as_deref() == Some(fingerprint.value.as_str())
    });
    let (should_refresh, mut index_status) = if config.refresh_index {
        (true, "refreshed_forced")
    } else if cache_matches {
        (false, "reused_cached_fingerprint")
    } else if cached_fingerprint.is_some() {
        (true, "refreshed_repository_changed")
    } else if let Some(fingerprint) = fingerprint.as_ref() {
        if fingerprint.dirty {
            (true, "refreshed_dirty_worktree")
        } else {
            match codebase_memory_indexed_heads(binary, &project, deadline) {
                Ok(heads) => {
                    let current = heads.iter().any(|head| head == &fingerprint.head);
                    if current {
                        codebase_memory_cache_index(cache_key.clone(), fingerprint);
                        (false, "reused_graph_head")
                    } else {
                        (true, "refreshed_stale_graph")
                    }
                }
                Err(error) if codebase_memory_project_missing(&error) => {
                    (true, "refreshed_missing_project")
                }
                Err(_) if deadline.exhausted() => {
                    return Err(deadline.timeout_error("query_graph"));
                }
                Err(_) => (false, "reused_unverified_graph"),
            }
        }
    } else {
        (false, "not_checked_non_git")
    };

    let primary_result = if should_refresh {
        index_project()?;
        if let Some(fingerprint) = fingerprint.as_ref() {
            codebase_memory_cache_index(cache_key.clone(), fingerprint);
        }
        primary_lookup()?
    } else {
        match primary_lookup() {
            Ok(value) => value,
            Err(error) if codebase_memory_project_missing(&error) => {
                index_project()?;
                index_status = "refreshed_missing_project";
                if let Some(fingerprint) = fingerprint.as_ref() {
                    codebase_memory_cache_index(cache_key.clone(), fingerprint);
                }
                primary_lookup()?
            }
            Err(error) => return Err(error),
        }
    };
    let extract_evidence = |primary_result: &Value| -> Result<Option<_>> {
        if prefers_architecture {
            return match agent_first_read_architecture_evidence(primary_result, task) {
                Some(evidence) => Ok(Some(evidence)),
                None => optional_codebase_memory_value(search_graph(), deadline, "search_graph")
                    .map(|search_graph| {
                        search_graph.and_then(|search_graph| {
                            agent_first_read_backend_evidence(&search_graph).ok()
                        })
                    }),
            };
        }

        match agent_first_read_backend_evidence(primary_result) {
            Ok(evidence) => Ok(Some(evidence)),
            Err(error) if error.to_string() == CODEBASE_MEMORY_EMPTY_SEARCH_ERROR => {
                let search_code = match search_code() {
                    Ok(value) => value,
                    Err(error) => {
                        optional_codebase_memory_value(Err(error), deadline, "search_code")?
                    }
                };
                match search_code
                    .as_ref()
                    .and_then(agent_first_read_search_code_evidence)
                {
                    Some(evidence) => Ok(Some(evidence)),
                    None => optional_codebase_memory_value(
                        get_architecture(),
                        deadline,
                        "get_architecture",
                    )
                    .map(|architecture| {
                        architecture.and_then(|architecture| {
                            agent_first_read_architecture_evidence(&architecture, task)
                        })
                    }),
                }
            }
            Err(error) => Err(error),
        }
    };
    let mut evidence = extract_evidence(&primary_result)?;
    if !should_refresh
        && evidence.as_ref().is_some_and(|evidence| {
            !evidence.candidate_files.is_empty()
                && evidence
                    .candidate_files
                    .iter()
                    .all(|file| !root.join(file).is_file())
        })
    {
        index_project()?;
        if let Some(fingerprint) = fingerprint.as_ref() {
            codebase_memory_cache_index(cache_key, fingerprint);
        }
        index_status = "refreshed_stale_candidates";
        let refreshed_primary = primary_lookup()?;
        evidence = extract_evidence(&refreshed_primary)?;
    }
    Ok(CodebaseMemoryAgentFirstReadResult {
        evidence,
        index_status,
    })
}

pub async fn serve(transport: Transport) -> Result<()> {
    match transport {
        Transport::Stdio => serve_stdio().await,
    }
}

async fn serve_stdio() -> Result<()> {
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut lines = BufReader::new(stdin).lines();

    while let Some(line) = lines.next_line().await? {
        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = serde_json::from_str(&line)?;
        let id = request
            .get("id")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        let method = request
            .get("method")
            .and_then(|method| method.as_str())
            .unwrap_or_default();

        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {
                        "tools": {}
                    },
                    "serverInfo": {
                        "name": "codeinsight",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                }
            }),
            "tools/list" => json_success(id, json!({ "tools": tool_definitions() })),
            "tools/call" => {
                match handle_tool_call(request.get("params").cloned().unwrap_or_default()) {
                    Ok(result) => json_success(id, result),
                    Err(error) => json_error(id, -32602, error.to_string()),
                }
            }
            "notifications/initialized" => continue,
            _ => json_error(id, -32601, format!("method not found: {method}")),
        };

        stdout.write_all(response.to_string().as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

fn handle_tool_call(params: Value) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .context("missing tool name")?;
    let arguments = optional_object(&params, "arguments")?;

    let result = match name {
        "index_project" => {
            let root = required_path(&arguments, "root")?;
            let force = optional_bool(&arguments, "force", false)?;
            serde_json::to_value(tools::index_project_value(root, force)?)?
        }
        "config_status" => {
            let root = required_path(&arguments, "root")?;
            serde_json::to_value(tools::config_status_value(root)?)?
        }
        "project_overview" => {
            let root = required_path(&arguments, "root")?;
            serde_json::to_value(tools::project_overview_value(root)?)?
        }
        "symbol_search" => {
            let root = required_path(&arguments, "root")?;
            let query = required_str(&arguments, "query")?;
            let limit = optional_positive_usize(&arguments, "limit", 20)?;
            serde_json::to_value(tools::symbol_search_value(root, query, limit)?)?
        }
        "file_outline" => {
            let path = required_path(&arguments, "path")?;
            let locations = optional_locations(&arguments, "locations")?;
            serde_json::to_value(tools::file_outline_for_locations_value(path, &locations)?)?
        }
        "dependency_graph" => {
            let root = required_path(&arguments, "root")?;
            let files = optional_string_array(&arguments, "files")?;
            let languages = optional_string_array(&arguments, "languages")?;
            let kinds = optional_string_array(&arguments, "kinds")?;
            let limit = optional_positive_usize(&arguments, "limit", 500)?;
            let offset = optional_min_usize(&arguments, "offset", 0, 0)?;
            serde_json::to_value(tools::dependency_graph_value(
                root, files, languages, kinds, limit, offset,
            )?)?
        }
        "impact_analysis" => {
            let root = required_path(&arguments, "root")?;
            let symbols = optional_string_array(&arguments, "symbols")?;
            let files = optional_string_array(&arguments, "files")?;
            let limit = optional_positive_usize(&arguments, "limit", 50)?;
            let depth = optional_positive_usize(&arguments, "depth", 1)?;
            let format = optional_str(&arguments, "format", "full")?.to_string();
            let evidence_limit = optional_positive_usize(&arguments, "evidence_limit", 20)?;
            serde_json::to_value(tools::impact_analysis_value(
                root,
                symbols,
                files,
                limit,
                depth,
                format,
                evidence_limit,
            )?)?
        }
        "find_references" => {
            let root = required_path(&arguments, "root")?;
            let symbol = required_str(&arguments, "symbol")?;
            let limit = optional_positive_usize(&arguments, "limit", 100)?;
            let include_definitions = optional_bool(&arguments, "include_definitions", false)?;
            serde_json::to_value(tools::find_references_value(
                root,
                symbol,
                limit,
                include_definitions,
            )?)?
        }
        "semantic_search" => {
            let root = required_path(&arguments, "root")?;
            let query = required_str(&arguments, "query")?;
            let limit = optional_positive_usize(&arguments, "limit", 20)?;
            serde_json::to_value(tools::semantic_search_value(root, query, limit)?)?
        }
        "semantic_index" => {
            let root = required_path(&arguments, "root")?;
            let chunk_lines = optional_positive_usize(&arguments, "chunk_lines", 80)?;
            let explain = optional_bool(&arguments, "explain", false)?;
            serde_json::to_value(tools::semantic_index_value(root, chunk_lines, explain)?)?
        }
        "embedding_status" => {
            let root = optional_path(&arguments, "root")?;
            serde_json::to_value(tools::embedding_status_value(root)?)?
        }
        "version" => serde_json::to_value(tools::version_value())?,
        "context_pack" => {
            let root = required_path(&arguments, "root")?;
            let task = required_str(&arguments, "task")?.to_string();
            let symbols = optional_string_array(&arguments, "symbols")?;
            let files = optional_string_array(&arguments, "files")?;
            let token_budget = optional_min_usize(&arguments, "token_budget", 6000, 500)?;
            serde_json::to_value(tools::context_pack_value(
                root,
                task,
                symbols,
                files,
                token_budget,
            )?)?
        }
        "agent_first_read" => {
            let root = required_path(&arguments, "root")?;
            let task = required_str(&arguments, "task")?.to_string();
            let symbols = optional_string_array(&arguments, "symbols")?;
            let files = optional_string_array(&arguments, "files")?;
            let token_budget = optional_min_usize(&arguments, "token_budget", 6000, 500)?;
            let force_index = optional_bool(&arguments, "force_index", false)?;
            let response_token_budget = optional_min_usize(
                &arguments,
                "response_token_budget",
                AGENT_FIRST_READ_RESPONSE_TOKEN_BUDGET,
                500,
            )?;
            let backend_config: Option<AgentFirstReadBackendConfig> =
                optional_json_object(&arguments, "backend")?;
            if backend_config.is_some() && arguments.get("backend_candidates").is_some() {
                bail!("agent_first_read backend cannot be combined with backend_candidates");
            }
            let has_explicit_seed = !symbols.is_empty() || !files.is_empty();
            let has_task_path_seed = tools::task_has_existing_path(&root, &task);
            let (backend_evidence, backend_status) = match backend_config {
                Some(config) => {
                    config.validate()?;
                    let provider = config.provider.clone();
                    let failure_policy = config.on_failure;
                    if has_explicit_seed || has_task_path_seed {
                        let (status, reason) = if has_explicit_seed {
                            ("skipped_explicit_seed", None)
                        } else {
                            (
                                "skipped_task_path",
                                Some("task contains an existing local file path".to_string()),
                            )
                        };
                        (
                            None,
                            Some(AgentRouteBackendStatus {
                                provider,
                                status: status.to_string(),
                                failure_policy: failure_policy.as_str().to_string(),
                                index_status: None,
                                reason,
                            }),
                        )
                    } else {
                        match codebase_memory_agent_first_read_result(&root, &task, config) {
                            Ok(result) => {
                                let status = if result.evidence.is_some() {
                                    "used"
                                } else {
                                    "no_candidates"
                                };
                                (
                                    result.evidence,
                                    Some(AgentRouteBackendStatus {
                                        provider,
                                        status: status.to_string(),
                                        failure_policy: failure_policy.as_str().to_string(),
                                        index_status: Some(result.index_status.to_string()),
                                        reason: None,
                                    }),
                                )
                            }
                            Err(error)
                                if failure_policy
                                    == AgentFirstReadBackendFailurePolicy::FallbackLocal =>
                            {
                                let reason = error.to_string().chars().take(240).collect();
                                (
                                    None,
                                    Some(AgentRouteBackendStatus {
                                        provider,
                                        status: "fallback_local".to_string(),
                                        failure_policy: failure_policy.as_str().to_string(),
                                        index_status: None,
                                        reason: Some(reason),
                                    }),
                                )
                            }
                            Err(error) => return Err(error),
                        }
                    }
                }
                None => (
                    optional_agent_first_read_backend_evidence(&arguments)?,
                    None,
                ),
            };
            let mut report = tools::agent_route_value(
                root,
                task,
                symbols,
                files,
                token_budget,
                force_index,
                50,
                1,
                20,
                false,
                backend_evidence,
                backend_status
                    .as_ref()
                    .and_then(|status| status.index_status.as_deref()),
            )?;
            report.backend_status = backend_status;
            tools::agent_route_response_value(&report, true, Some(response_token_budget))?
        }
        "agent_route" => {
            let root = required_path(&arguments, "root")?;
            let task = required_str(&arguments, "task")?.to_string();
            let symbols = optional_string_array(&arguments, "symbols")?;
            let files = optional_string_array(&arguments, "files")?;
            let token_budget = optional_min_usize(&arguments, "token_budget", 6000, 500)?;
            let force_index = optional_bool(&arguments, "force_index", false)?;
            let impact_limit = optional_positive_usize(&arguments, "impact_limit", 50)?;
            let impact_depth = optional_positive_usize(&arguments, "impact_depth", 1)?;
            let impact_evidence_limit =
                optional_positive_usize(&arguments, "impact_evidence_limit", 20)?;
            let include_impact = optional_bool(&arguments, "include_impact", true)?;
            let compact = match optional_str(&arguments, "response_mode", "full")? {
                "full" => false,
                "compact" => true,
                _ => bail!("invalid response_mode; expected full or compact"),
            };
            let response_token_budget = arguments
                .get("response_token_budget")
                .map(|_| optional_min_usize(&arguments, "response_token_budget", 0, 500))
                .transpose()?;
            if response_token_budget.is_some() && !compact {
                bail!("response_token_budget requires compact response_mode");
            }
            let backend_evidence: Option<AgentRouteBackendEvidence> =
                optional_json_object(&arguments, "backend_evidence")?;
            let report = tools::agent_route_value(
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
                None,
            )?;
            tools::agent_route_response_value(&report, compact, response_token_budget)?
        }
        "callers" => {
            let root = required_path(&arguments, "root")?;
            let symbol = required_str(&arguments, "symbol")?;
            let limit = optional_positive_usize(&arguments, "limit", 50)?;
            serde_json::to_value(tools::callers_value(root, symbol, limit)?)?
        }
        "callees" => {
            let root = required_path(&arguments, "root")?;
            let symbol = required_str(&arguments, "symbol")?;
            let limit = optional_positive_usize(&arguments, "limit", 50)?;
            serde_json::to_value(tools::callees_value(root, symbol, limit)?)?
        }
        _ => bail!("unknown tool: {name}"),
    };

    let text_content = tool_text_content(name, &result)?;
    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": text_content
            }
        ],
        "structuredContent": result
    }))
}

fn tool_text_content(name: &str, result: &Value) -> Result<String> {
    if !matches!(name, "agent_first_read" | "agent_route")
        || result.get("response_mode").and_then(Value::as_str) != Some("compact")
    {
        return Ok(serde_json::to_string_pretty(result)?);
    }

    let first_file = result
        .pointer("/routing_decision/first_file")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let selected_files = result
        .pointer("/context_pack/budget/selected_files")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let selected_ranges = result
        .pointer("/context_pack/budget/selected_ranges")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let next_action = result
        .pointer("/routing_decision/route_quality/recommended_action")
        .and_then(Value::as_str)
        .unwrap_or("inspect_structured_content");
    let backend_status = result
        .pointer("/routing_decision/backend_route_agreement/status")
        .and_then(Value::as_str)
        .unwrap_or("no_backend");
    let backend_provider = result
        .pointer("/routing_decision/backend_route_agreement/provider")
        .and_then(Value::as_str)
        .unwrap_or("-");
    let backend_symbol = result
        .pointer("/routing_decision/backend_selected_candidate/symbol")
        .and_then(Value::as_str)
        .or_else(|| {
            result
                .pointer(
                    "/routing_decision/backend_route_agreement/candidate_dispositions/0/symbol",
                )
                .and_then(Value::as_str)
        })
        .unwrap_or("-");
    let backend_candidate_recovery_handoff = result
        .pointer(
            "/routing_decision/backend_route_agreement/candidate_dispositions/0/context_suggested_tool/tool",
        )
        .and_then(Value::as_str)
        .map(|tool| format!("; candidate_recovery_tool={tool}"))
        .unwrap_or_default();
    let backend_symbol_handoff = result
        .pointer(
            "/routing_decision/backend_route_agreement/candidate_dispositions/0/symbol_status",
        )
        .and_then(Value::as_str)
        .filter(|status| *status == "stale")
        .map(|status| {
            let recovery_tool = result
                .pointer(
                    "/routing_decision/backend_route_agreement/candidate_dispositions/0/symbol_suggested_tool/tool",
                )
                .and_then(Value::as_str)
                .unwrap_or("-");
            format!("; backend_symbol_status={status}; symbol_recovery_tool={recovery_tool}")
        })
        .unwrap_or_default();
    let backend_location_handoff = result
        .pointer(
            "/routing_decision/backend_route_agreement/candidate_dispositions/0/location_status",
        )
        .and_then(Value::as_str)
        .filter(|status| *status == "ambiguous")
        .map(|status| {
            let alternatives = result
                .pointer(
                    "/routing_decision/backend_route_agreement/candidate_dispositions/0/location_alternatives",
                )
                .and_then(Value::as_array)
                .map(Vec::len)
                .unwrap_or(0);
            let omitted = result
                .pointer(
                    "/routing_decision/backend_route_agreement/candidate_dispositions/0/omitted_location_alternatives",
                )
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let first_context_pack_file = result
                .pointer(
                    "/routing_decision/backend_route_agreement/candidate_dispositions/0/location_alternatives/0/context_pack_file",
                )
                .and_then(Value::as_str)
                .unwrap_or("-");
            let omitted_alternatives_tool = result
                .pointer(
                    "/routing_decision/backend_route_agreement/candidate_dispositions/0/location_suggested_tool/tool",
                )
                .and_then(Value::as_str)
                .unwrap_or("-");
            format!(
                "; backend_location_status={status}; backend_location_alternatives={alternatives}; backend_location_alternatives_omitted={omitted}; first_context_pack_file={first_context_pack_file}; omitted_alternatives_tool={omitted_alternatives_tool}"
            )
        })
        .unwrap_or_default();
    let backend_stale_location_handoff = result
        .pointer(
            "/routing_decision/backend_route_agreement/candidate_dispositions/0/location_status",
        )
        .and_then(Value::as_str)
        .filter(|status| *status == "stale")
        .map(|status| {
            let recovery_tool = result
                .pointer(
                    "/routing_decision/backend_route_agreement/candidate_dispositions/0/location_suggested_tool/tool",
                )
                .and_then(Value::as_str)
                .unwrap_or("-");
            format!("; backend_location_status={status}; location_recovery_tool={recovery_tool}")
        })
        .unwrap_or_default();
    let impact_status = result
        .get("impact_status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    Ok(format!(
        "Compact agent route: first_file={first_file}; selected_files={selected_files}; selected_ranges={selected_ranges}; backend_status={backend_status}; backend_provider={backend_provider}; backend_symbol={backend_symbol}{backend_candidate_recovery_handoff}{backend_symbol_handoff}{backend_location_handoff}{backend_stale_location_handoff}; next_action={next_action}; impact_status={impact_status}. Read structuredContent for selected excerpts and execution_plan."
    ))
}

fn tool_definitions() -> Value {
    let backend_tool_result_schema = json!({
        "oneOf": [
            {"type": "object"},
            {
                "type": "array",
                "maxItems": 16,
                "items": {"type": "object"}
            }
        ]
    });
    let backend_evidence_schema = json!({
        "type": "object",
        "description": "Optional advisory evidence from an external code graph backend. CodeInsight uses it to explain route confidence; set use_as_fallback to seed bounded context only when local routing is blocked.",
        "properties": {
            "provider": {"type": "string", "maxLength": 128},
            "use_as_fallback": {"type": "boolean", "default": false},
            "prefer_for_context": {
                "type": "boolean",
                "default": false,
                "description": "Prefer backend-ranked candidates for bounded context when the caller did not provide explicit file or symbol seeds."
            },
            "candidate_files": {
                "type": "array",
                "maxItems": 16,
                "items": {"type": "string", "maxLength": 512}
            },
            "candidates": {
                "type": "array",
                "maxItems": 16,
                "description": "Ranked structured candidates. When present, these lead candidate_files and preserve symbol, score, and routing evidence.",
                "items": {
                    "type": "object",
                    "properties": {
                        "file": {"type": "string", "maxLength": 512},
                        "symbol": {"type": "string", "maxLength": 160},
                        "locations": {
                            "type": "array",
                            "maxItems": 16,
                            "items": {
                                "type": "object",
                                "properties": {
                                    "start_line": {"type": "integer", "minimum": 1},
                                    "end_line": {"type": "integer", "minimum": 1}
                                },
                                "required": ["start_line", "end_line"],
                                "additionalProperties": false
                            }
                        },
                        "source": {"type": "string", "maxLength": 160},
                        "score": {"type": "number"},
                        "reason": {"type": "string", "maxLength": 320},
                        "evidence": {
                            "type": "array",
                            "maxItems": 6,
                            "items": {"type": "string", "maxLength": 160}
                        }
                    },
                    "required": ["file"]
                }
            },
            "evidence_sources": {
                "type": "array",
                "maxItems": 12,
                "items": {"type": "string", "maxLength": 160}
            },
            "evidence_count": {"type": "integer", "minimum": 0},
            "latency_ms": {"type": "integer", "minimum": 0},
            "confidence": {"type": "number"},
            "notes": {
                "type": "array",
                "maxItems": 6,
                "items": {"type": "string", "maxLength": 320}
            },
            "tool_results": {
                "type": "object",
                "description": "Raw code graph tool results. Each tool accepts one response object or an ordered array of paginated responses. CodeInsight reads at most 64 items across all pages per tool, extracts bounded candidates, and omits the raw payload from its response.",
                "properties": {
                    "get_code_snippet": backend_tool_result_schema.clone(),
                    "search_graph": backend_tool_result_schema.clone(),
                    "search_code": backend_tool_result_schema.clone(),
                    "query_graph": backend_tool_result_schema.clone(),
                    "trace_path": backend_tool_result_schema.clone(),
                    "get_architecture": backend_tool_result_schema
                },
                "minProperties": 1
            }
        },
        "required": ["provider"]
    });
    let agent_first_read_backend_candidates_schema = json!({
        "type": "object",
        "description": "Optional external code-graph ranking. Pass a complete search_graph response directly, or use provider with candidate_files, candidates, or search_graph. When no explicit seed is provided, CodeInsight prefers this ranking as bounded context seeds.",
        "properties": {
            "provider": {"type": "string", "maxLength": 128},
            "candidate_files": {
                "type": "array",
                "maxItems": 8,
                "items": {"type": "string", "maxLength": 512}
            },
            "candidates": {
                "type": "array",
                "maxItems": 8,
                "description": "Ranked compact candidates. Accepts CodeInsight file/symbol fields or codebase-memory search_graph file_path/name/qualified_name fields; other search_graph metadata is ignored.",
                "items": {
                    "type": "object",
                    "properties": {
                        "file": {"type": "string", "maxLength": 512},
                        "file_path": {"type": "string", "maxLength": 512},
                        "symbol": {"type": "string", "maxLength": 512},
                        "qualified_name": {"type": "string", "maxLength": 512},
                        "name": {"type": "string", "maxLength": 512},
                        "start_line": {"type": "integer", "minimum": 1},
                        "end_line": {"type": "integer", "minimum": 1}
                    },
                    "anyOf": [
                        {"required": ["file"]},
                        {"required": ["file_path"]}
                    ]
                }
            },
            "search_graph": {
                "type": "object",
                "description": "Complete structured search_graph response. Accepts a direct payload, an MCP structuredContent wrapper, or a JSON-RPC result wrapper. CodeInsight consumes at most 8 valid candidates; use a backend search limit of 8 when possible to keep input tokens bounded."
            },
            "results": {"type": "array"},
            "semantic_results": {"type": "array"},
            "structuredContent": {"type": "object"},
            "result": {"type": "object"},
            "evidence_sources": {
                "type": "array",
                "maxItems": 6,
                "items": {"type": "string", "maxLength": 160}
            },
            "confidence": {"type": "number"},
            "latency_ms": {"type": "integer", "minimum": 0}
        },
        "anyOf": [
            {"required": ["provider", "candidate_files"]},
            {"required": ["provider", "candidates"]},
            {"required": ["provider", "search_graph"]},
            {"required": ["results"]},
            {"required": ["semantic_results"]},
            {"required": ["structuredContent"]},
            {"required": ["result"]}
        ]
    });
    json!([
        {
            "name": "index_project",
            "description": "Index a local repository for code intelligence.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "force": {"type": "boolean"}
                },
                "required": ["root"]
            }
        },
        {
            "name": "config_status",
            "description": "Return project configuration status, configured index scope, impact-analysis checks, and detected test commands.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"}
                },
                "required": ["root"]
            }
        },
        {
            "name": "project_overview",
            "description": "Return an indexed repository overview with language stats, key directories, role hints, symbol kinds, dependency/call summaries, entrypoint candidates, confidence scores, and index metadata.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"}
                },
                "required": ["root"]
            }
        },
        {
            "name": "symbol_search",
            "description": "Search symbols in an indexed repository.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1}
                },
                "required": ["root", "query"]
            }
        },
        {
            "name": "file_outline",
            "description": "Parse one source file and return a symbol outline.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "locations": {
                        "type": "array",
                        "maxItems": 16,
                        "description": "Optional one-based line ranges. Only symbols overlapping at least one range are returned.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "start_line": {"type": "integer", "minimum": 1},
                                "end_line": {"type": "integer", "minimum": 1}
                            },
                            "required": ["start_line", "end_line"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "dependency_graph",
            "description": "Return module-level dependencies extracted during indexing.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "files": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "languages": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "kinds": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "limit": {"type": "integer", "minimum": 1},
                    "offset": {"type": "integer", "minimum": 0}
                },
                "required": ["root"]
            }
        },
        {
            "name": "impact_analysis",
            "description": "Estimate local impact radius from seed symbols or files using references, call graph, and resolved dependencies.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "symbols": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "files": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "limit": {"type": "integer", "minimum": 1},
                    "depth": {"type": "integer", "minimum": 1},
                    "format": {"type": "string", "enum": ["summary", "full"]},
                    "evidence_limit": {"type": "integer", "minimum": 1}
                },
                "required": ["root"]
            }
        },
        {
            "name": "find_references",
            "description": "Find text references for a symbol across indexed files.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "symbol": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1},
                    "include_definitions": {"type": "boolean"}
                },
                "required": ["root", "symbol"]
            }
        },
        {
            "name": "semantic_search",
            "description": "Preview semantic code search through a configured embedding provider.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "query": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1}
                },
                "required": ["root", "query"]
            }
        },
        {
            "name": "semantic_index",
            "description": "Build local semantic text chunks for a previously indexed repository, optionally returning per-chunk change details.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "chunk_lines": {"type": "integer", "minimum": 1},
                    "explain": {"type": "boolean"}
                },
                "required": ["root"]
            }
        },
        {
            "name": "embedding_status",
            "description": "Return the configured embedding provider, embedding batch size, and optional local semantic index status without making network requests.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"}
                }
            }
        },
        {
            "name": "version",
            "description": "Return CodeInsight package version and target platform information.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
        {
            "name": "context_pack",
            "description": "Build an agent-ready context pack from seed symbols, seed files, or inferred source entrypoints and a token budget.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "task": {"type": "string"},
                    "symbols": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "description": "Seed file path, optionally with :line[:column] or #Lstart-Lend."
                        }
                    },
                    "token_budget": {"type": "integer", "minimum": 500}
                },
                "required": ["root", "task"]
            }
        },
        {
            "name": "agent_first_read",
            "description": "Preferred first call for AI coding agents. Refresh the local index, optionally route external backend candidates into bounded code excerpts, return a compact reading/execution plan, and defer impact analysis until before edits.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "task": {"type": "string"},
                    "symbols": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "description": "Seed file path, optionally with :line[:column] or #Lstart-Lend."
                        }
                    },
                    "token_budget": {"type": "integer", "minimum": 500, "default": 6000},
                    "response_token_budget": {
                        "type": "integer",
                        "minimum": 500,
                        "default": 8000,
                        "description": "Hard cap for the compact structured route payload. The MCP envelope and concise text summary are excluded."
                    },
                    "backend": {
                        "type": "object",
                        "description": "Optionally invoke a local code graph backend before routing. This is opt-in; without it agent_first_read remains standalone.",
                        "properties": {
                            "provider": {
                                "type": "string",
                                "enum": ["codebase-memory-mcp"]
                            },
                            "project": {
                                "type": "string",
                                "maxLength": 256,
                                "description": "Indexed backend project name. Defaults to the root directory name."
                            },
                            "refresh_index": {
                                "type": "boolean",
                                "default": false,
                                "description": "Force a backend fast-index refresh before searching. When false, a current graph is reused while stale commits, changed worktree content, and missing projects are refreshed automatically."
                            },
                            "timeout_ms": {
                                "type": "integer",
                                "minimum": 1000,
                                "maximum": 300000,
                                "default": 60000,
                                "description": "Total time budget shared by backend freshness checks, indexing, and candidate lookup fallbacks."
                            },
                            "on_failure": {
                                "type": "string",
                                "enum": ["fallback_local", "error"],
                                "default": "fallback_local",
                                "description": "Continue with the standalone local route when the backend command fails, or return an MCP error. Invalid backend configuration always returns an error. Automatic backend invocation is skipped when files or symbols provide an explicit seed, or when the task names an existing local file path."
                            }
                        },
                        "required": ["provider"],
                        "additionalProperties": false
                    },
                    "backend_candidates": agent_first_read_backend_candidates_schema,
                    "force_index": {"type": "boolean", "default": false}
                },
                "required": ["root", "task"]
            }
        },
        {
            "name": "agent_route",
            "description": "Advanced configurable route analysis. Use agent_first_read for the default token-efficient first read; use this tool when full overview, backend evidence, or synchronous impact preview is required.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "task": {"type": "string"},
                    "symbols": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "description": "Seed file path, optionally with :line[:column] or #Lstart-Lend."
                        }
                    },
                    "token_budget": {"type": "integer", "minimum": 500},
                    "force_index": {"type": "boolean"},
                    "impact_limit": {"type": "integer", "minimum": 1},
                    "impact_depth": {"type": "integer", "minimum": 1},
                    "impact_evidence_limit": {"type": "integer", "minimum": 1},
                    "include_impact": {
                        "type": "boolean",
                        "default": true,
                        "description": "Set false for a fast first read; agent_route returns a deferred impact_analysis suggestion that remains required before edits."
                    },
                    "response_mode": {
                        "type": "string",
                        "enum": ["full", "compact"],
                        "default": "full",
                        "description": "Use compact to keep selected excerpts and the execution contract while omitting duplicate overview and raw evidence arrays."
                    },
                    "response_token_budget": {
                        "type": "integer",
                        "minimum": 500,
                        "description": "Optional hard cap for the compact structured route payload. The MCP envelope and concise text summary are excluded. Requires response_mode=compact."
                    },
                    "backend_evidence": backend_evidence_schema
                },
                "required": ["root", "task"]
            }
        },
        {
            "name": "callers",
            "description": "Return static call sites that call a function or method, including imported target file hints when available.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "symbol": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1}
                },
                "required": ["root", "symbol"]
            }
        },
        {
            "name": "callees",
            "description": "Return static callees for a function or method, including imported target file hints when available.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "symbol": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1}
                },
                "required": ["root", "symbol"]
            }
        }
    ])
}

fn required_path(arguments: &Value, key: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(required_str(arguments, key)?))
}

fn optional_path(arguments: &Value, key: &str) -> Result<Option<PathBuf>> {
    match arguments.get(key) {
        Some(value) => Ok(Some(PathBuf::from(
            value
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .with_context(|| format!("invalid path argument: {key}"))?,
        ))),
        None => Ok(None),
    }
}

fn optional_object(value: &Value, key: &str) -> Result<Value> {
    match value.get(key) {
        Some(arguments) if arguments.is_object() => Ok(arguments.clone()),
        Some(_) => bail!("invalid object argument: {key}"),
        None => Ok(json!({})),
    }
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("missing or invalid string argument: {key}"))
}

fn optional_str<'a>(arguments: &'a Value, key: &str, default: &'a str) -> Result<&'a str> {
    match arguments.get(key) {
        Some(value) => value
            .as_str()
            .filter(|value| !value.trim().is_empty())
            .with_context(|| format!("invalid string argument: {key}")),
        None => Ok(default),
    }
}

fn optional_string_array(arguments: &Value, key: &str) -> Result<Vec<String>> {
    match arguments.get(key) {
        Some(value) if value.is_array() => value
            .as_array()
            .unwrap()
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| !value.trim().is_empty())
                    .map(ToOwned::to_owned)
                    .with_context(|| format!("invalid non-string value in argument: {key}"))
            })
            .collect::<Result<Vec<_>>>(),
        Some(_) => bail!("invalid string array argument: {key}"),
        None => Ok(Vec::new()),
    }
}

fn optional_locations(arguments: &Value, key: &str) -> Result<Vec<ContextSeedLocation>> {
    let Some(value) = arguments.get(key) else {
        return Ok(Vec::new());
    };
    let Some(locations) = value.as_array() else {
        bail!("invalid location array argument: {key}");
    };
    if locations.len() > 16 {
        bail!("location array argument {key} must contain at most 16 items");
    }

    locations
        .iter()
        .map(|location| {
            let Some(location) = location.as_object() else {
                bail!("invalid location object in argument: {key}");
            };
            let start_line = location
                .get("start_line")
                .and_then(Value::as_u64)
                .and_then(|line| usize::try_from(line).ok())
                .filter(|line| *line > 0)
                .with_context(|| format!("invalid start_line in argument: {key}"))?;
            let end_line = location
                .get("end_line")
                .and_then(Value::as_u64)
                .and_then(|line| usize::try_from(line).ok())
                .filter(|line| *line >= start_line)
                .with_context(|| format!("invalid end_line in argument: {key}"))?;
            Ok(ContextSeedLocation {
                start_line,
                end_line,
            })
        })
        .collect()
}

fn optional_json_object<T>(arguments: &Value, key: &str) -> Result<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    match arguments.get(key) {
        Some(value) if value.is_object() => serde_json::from_value(value.clone())
            .with_context(|| format!("invalid object argument: {key}"))
            .map(Some),
        Some(_) => bail!("invalid object argument: {key}"),
        None => Ok(None),
    }
}

fn optional_bool(arguments: &Value, key: &str, default: bool) -> Result<bool> {
    match arguments.get(key) {
        Some(value) => value
            .as_bool()
            .with_context(|| format!("invalid boolean argument: {key}")),
        None => Ok(default),
    }
}

fn optional_positive_usize(arguments: &Value, key: &str, default: usize) -> Result<usize> {
    optional_min_usize(arguments, key, default, 1)
}

fn optional_min_usize(arguments: &Value, key: &str, default: usize, min: usize) -> Result<usize> {
    match arguments.get(key) {
        Some(value) => {
            let value = value
                .as_u64()
                .with_context(|| format!("invalid integer argument: {key}"))?
                as usize;
            if value < min {
                bail!("integer argument {key} must be >= {min}");
            }
            Ok(value)
        }
        None => Ok(default),
    }
}

fn json_success(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn json_error(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

#[allow(dead_code)]
fn unsupported(message: &str) -> Result<()> {
    bail!("{message}")
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn calls_index_and_symbol_search_tools() {
        let dir = TempDir::new().unwrap();
        let source_path = dir.path().join("auth.py");
        std::fs::write(
            &source_path,
            r#"
import os

class AuthService:
    def login(self):
        return helper()

def helper():
    return "ok"
"#,
        )
        .unwrap();

        let index_result = handle_tool_call(json!({
            "name": "index_project",
            "arguments": {
                "root": dir.path(),
                "force": true
            }
        }))
        .unwrap();
        assert_eq!(
            index_result["structuredContent"]["indexed_files"].as_u64(),
            Some(1)
        );

        let overview_result = handle_tool_call(json!({
            "name": "project_overview",
            "arguments": {
                "root": dir.path()
            }
        }))
        .unwrap();
        assert!(
            overview_result["structuredContent"]["recommended_next_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["tool"] == "context_pack"
                    && tool["suggested_arguments"]["root"].as_str().is_some())
        );
        assert!(
            overview_result["structuredContent"]["recommended_next_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["tool"] == "dependency_graph"
                    && tool["reason"]
                        .as_str()
                        .is_some_and(|reason| reason.contains("os")))
        );
        assert!(
            overview_result["structuredContent"]["recommended_next_tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|tool| tool["tool"] == "config_status")
        );

        let config_status_result = handle_tool_call(json!({
            "name": "config_status",
            "arguments": {
                "root": dir.path()
            }
        }))
        .unwrap();
        assert_eq!(
            config_status_result["structuredContent"]["exists"].as_bool(),
            Some(false)
        );
        assert_eq!(
            config_status_result["structuredContent"]["loaded"].as_bool(),
            Some(false)
        );

        let search_result = handle_tool_call(json!({
            "name": "symbol_search",
            "arguments": {
                "root": dir.path(),
                "query": "AuthService",
                "limit": 5
            }
        }))
        .unwrap();
        assert_eq!(
            search_result["structuredContent"][0]["name"].as_str(),
            Some("AuthService")
        );

        let located_outline_result = handle_tool_call(json!({
            "name": "file_outline",
            "arguments": {
                "path": source_path,
                "locations": [{"start_line": 9, "end_line": 9}]
            }
        }))
        .unwrap();
        assert_eq!(
            located_outline_result["structuredContent"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            located_outline_result["structuredContent"][0]["name"].as_str(),
            Some("helper")
        );

        let graph_result = handle_tool_call(json!({
            "name": "dependency_graph",
            "arguments": {
                "root": dir.path(),
                "limit": 10
            }
        }))
        .unwrap();
        assert_eq!(graph_result["structuredContent"]["edges"].as_u64(), Some(1));

        let impact_result = handle_tool_call(json!({
            "name": "impact_analysis",
            "arguments": {
                "root": dir.path(),
                "symbols": ["helper"],
                "files": ["auth.py"],
                "limit": 10,
                "depth": 2,
                "format": "summary",
                "evidence_limit": 1
            }
        }))
        .unwrap();
        assert_eq!(
            impact_result["structuredContent"]["depth"].as_u64(),
            Some(2)
        );
        assert_eq!(
            impact_result["structuredContent"]["format"].as_str(),
            Some("summary")
        );
        assert_eq!(
            impact_result["structuredContent"]["evidence_limit"].as_u64(),
            Some(1)
        );
        assert!(
            impact_result["structuredContent"]["risk_level"]
                .as_str()
                .is_some_and(|risk| ["low", "medium", "high"].contains(&risk))
        );
        assert!(
            impact_result["structuredContent"]["impact_counts"]["impacted_files"]
                .as_u64()
                .unwrap()
                >= 1
        );
        assert!(
            impact_result["structuredContent"]["impact_breakdown"]["call_related_files"]
                .as_u64()
                .unwrap()
                >= 1
        );
        assert!(
            impact_result["structuredContent"]["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("call-related files"))
        );
        assert!(
            !impact_result["structuredContent"]["top_reasons"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert!(
            !impact_result["structuredContent"]["suggested_checks"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            impact_result["structuredContent"]["seed_symbols"][0].as_str(),
            Some("helper")
        );
        assert!(
            impact_result["structuredContent"]["impacted_files"]
                .as_array()
                .unwrap()
                .iter()
                .any(|file| file["file"] == "auth.py")
        );
        assert!(
            impact_result["structuredContent"]["callers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|call| call["caller"] == "AuthService.login")
        );
        assert!(
            impact_result["structuredContent"]["paths"]
                .as_array()
                .unwrap()
                .iter()
                .any(|path| path["kind"] == "call")
        );

        let refs_result = handle_tool_call(json!({
            "name": "find_references",
            "arguments": {
                "root": dir.path(),
                "symbol": "AuthService",
                "include_definitions": true
            }
        }))
        .unwrap();
        assert_eq!(
            refs_result["structuredContent"][0]["file"].as_str(),
            Some("auth.py")
        );

        let semantic_error = handle_tool_call(json!({
            "name": "semantic_search",
            "arguments": {
                "root": dir.path(),
                "query": "authentication flow",
                "limit": 5
            }
        }))
        .unwrap_err();
        assert!(
            semantic_error
                .to_string()
                .contains("CODEINSIGHT_EMBEDDING_PROVIDER=local-hash")
        );

        let semantic_index_result = handle_tool_call(json!({
            "name": "semantic_index",
            "arguments": {
                "root": dir.path(),
                "chunk_lines": 20,
                "explain": true
            }
        }))
        .unwrap();
        assert_eq!(
            semantic_index_result["structuredContent"]["vector_status"].as_str(),
            Some("chunks_indexed_without_embeddings")
        );
        assert_eq!(
            semantic_index_result["structuredContent"]["chunks"].as_u64(),
            Some(1)
        );
        assert_eq!(
            semantic_index_result["structuredContent"]["chunks_added"].as_u64(),
            Some(1)
        );
        assert_eq!(
            semantic_index_result["structuredContent"]["changes"][0]["change"].as_str(),
            Some("added")
        );
        assert_eq!(
            semantic_index_result["structuredContent"]["changes"][0]["file"].as_str(),
            Some("auth.py")
        );

        let embedding_status_result = handle_tool_call(json!({
            "name": "embedding_status",
            "arguments": {
                "root": dir.path()
            }
        }))
        .unwrap();
        assert_eq!(
            embedding_status_result["structuredContent"]["provider"].as_str(),
            Some("disabled")
        );
        assert_eq!(
            embedding_status_result["structuredContent"]["batch_size"].as_u64(),
            Some(64)
        );
        assert_eq!(
            embedding_status_result["structuredContent"]["batch_size_env"].as_str(),
            Some("CODEINSIGHT_EMBEDDING_BATCH_SIZE")
        );
        assert_eq!(
            embedding_status_result["structuredContent"]["index"]["chunks"].as_u64(),
            Some(1)
        );
        assert_eq!(
            embedding_status_result["structuredContent"]["index"]["vector_status"].as_str(),
            Some("provider_not_configured")
        );

        let version_result = handle_tool_call(json!({
            "name": "version",
            "arguments": {}
        }))
        .unwrap();
        assert_eq!(
            version_result["structuredContent"]["name"].as_str(),
            Some("codeinsight")
        );
        assert_eq!(
            version_result["structuredContent"]["version"].as_str(),
            Some(env!("CARGO_PKG_VERSION"))
        );

        let context_result = handle_tool_call(json!({
            "name": "context_pack",
            "arguments": {
                "root": dir.path(),
                "task": "understand auth service",
                "symbols": ["AuthService"],
                "token_budget": 1200
            }
        }))
        .unwrap();
        assert_eq!(
            context_result["structuredContent"]["seed_strategy"].as_str(),
            Some("explicit")
        );
        assert_eq!(
            context_result["structuredContent"]["selected_seeds"][0]["kind"].as_str(),
            Some("symbol")
        );
        assert_eq!(
            context_result["structuredContent"]["selected_seeds"][0]["value"].as_str(),
            Some("AuthService")
        );
        assert_eq!(
            context_result["structuredContent"]["files"][0]["file"].as_str(),
            Some("auth.py")
        );
        assert_eq!(
            context_result["structuredContent"]["reading_plan"][0]["order"].as_u64(),
            Some(1)
        );
        assert_eq!(
            context_result["structuredContent"]["reading_plan"][0]["file"].as_str(),
            Some("auth.py")
        );
        assert_eq!(
            context_result["structuredContent"]["reading_plan"][0]["next_action"].as_str(),
            Some("inspect_symbol_definition")
        );
        assert!(
            context_result["structuredContent"]["reading_plan"][0]["question"]
                .as_str()
                .is_some_and(|question| question.contains("definition"))
        );
        assert_eq!(
            context_result["structuredContent"]["reading_plan"][0]["suggested_tool"]["tool"]
                .as_str(),
            Some("file_outline")
        );
        assert_eq!(
            context_result["structuredContent"]["reading_plan"][0]["suggested_tool"]["priority"]
                .as_u64(),
            Some(10)
        );
        assert!(
            context_result["structuredContent"]["reading_plan"][0]["suggested_tool"]
                ["suggested_arguments"]["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("auth.py"))
        );
        assert!(
            context_result["structuredContent"]["reading_plan"][0]["ranges"][0]["start_line"]
                .as_u64()
                .is_some_and(|line| line > 0)
        );
        assert!(
            context_result["structuredContent"]["files"][0]["source"]
                .as_str()
                .is_some_and(is_known_context_source)
        );
        assert!(
            context_result["structuredContent"]["files"][0]["score"]
                .as_i64()
                .is_some_and(|score| score > 0)
        );
        assert!(
            context_result["structuredContent"]["files"][0]["ranges"][0]["source"]
                .as_str()
                .is_some_and(is_known_context_source)
        );
        assert!(
            context_result["structuredContent"]["files"][0]["ranges"][0]["score"]
                .as_i64()
                .is_some_and(|score| score > 0)
        );
        assert!(
            context_result["structuredContent"]["files"][0]["ranges"][0]["reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty())
        );
        let auto_context_result = handle_tool_call(json!({
            "name": "context_pack",
            "arguments": {
                "root": dir.path(),
                "task": "understand auth repository",
                "token_budget": 1200
            }
        }))
        .unwrap();
        assert_eq!(
            auto_context_result["structuredContent"]["seed_strategy"].as_str(),
            Some("auto_task_match")
        );
        assert_eq!(
            auto_context_result["structuredContent"]["selected_seeds"][0]["source"].as_str(),
            Some("task_match")
        );
        assert_eq!(
            auto_context_result["structuredContent"]["selected_seeds"][0]["matched_keywords"][0]
                .as_str(),
            Some("auth")
        );
        assert_eq!(
            auto_context_result["structuredContent"]["files"][0]["file"].as_str(),
            Some("auth.py")
        );
        assert!(
            auto_context_result["structuredContent"]["files"][0]["reason"]
                .as_str()
                .is_some_and(|reason| reason.contains("matched task keywords: auth"))
        );
        assert!(
            auto_context_result["structuredContent"]["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("auto-selected seed files"))
        );
        let file_context_result = handle_tool_call(json!({
            "name": "context_pack",
            "arguments": {
                "root": dir.path(),
                "task": "understand auth file",
                "files": ["auth.py:4"],
                "token_budget": 1200
            }
        }))
        .unwrap();
        assert_eq!(
            file_context_result["structuredContent"]["selected_seeds"][0]["value"].as_str(),
            Some("auth.py")
        );
        assert_eq!(
            file_context_result["structuredContent"]["selected_seeds"][0]["start_line"].as_u64(),
            Some(4)
        );
        assert_eq!(
            file_context_result["structuredContent"]["selected_seeds"][0]["end_line"].as_u64(),
            Some(4)
        );
        assert_eq!(
            file_context_result["structuredContent"]["selected_seeds"][0]["locations"],
            json!([{"start_line": 4, "end_line": 4}])
        );
        assert_eq!(
            file_context_result["structuredContent"]["reading_plan"][0]["requested_locations"],
            json!([{"start_line": 4, "end_line": 4}])
        );
        assert_eq!(
            file_context_result["structuredContent"]["files"][0]["file"].as_str(),
            Some("auth.py")
        );
        assert!(
            file_context_result["structuredContent"]["files"][0]["ranges"]
                .as_array()
                .unwrap()
                .iter()
                .any(|range| range["source"] == "task_location"
                    && range["start_line"].as_u64().unwrap() <= 4
                    && range["end_line"].as_u64().unwrap() >= 4)
        );
        assert!(
            file_context_result["structuredContent"]["summary"]
                .as_str()
                .unwrap()
                .contains("seed files")
        );

        let located_route_result = handle_tool_call(json!({
            "name": "agent_route",
            "arguments": {
                "root": dir.path(),
                "task": "inspect auth class",
                "files": ["auth.py:4", "auth.py#L7-L8"],
                "token_budget": 1200
            }
        }))
        .unwrap();
        assert_eq!(
            located_route_result["structuredContent"]["context_pack"]["selected_seeds"][0]
                ["start_line"]
                .as_u64(),
            Some(4)
        );
        assert_eq!(
            located_route_result["structuredContent"]["context_pack"]["selected_seeds"][0]["locations"],
            json!([
                {"start_line": 4, "end_line": 4},
                {"start_line": 7, "end_line": 8}
            ])
        );
        assert_eq!(
            located_route_result["structuredContent"]["current_reading_step"]["requested_locations"],
            json!([
                {"start_line": 4, "end_line": 4},
                {"start_line": 7, "end_line": 8}
            ])
        );
        assert_eq!(
            located_route_result["structuredContent"]["execution_plan"][0]["requested_locations_by_file"],
            json!({
                "auth.py": [
                    {"start_line": 4, "end_line": 4},
                    {"start_line": 7, "end_line": 8}
                ]
            })
        );
        assert_eq!(
            located_route_result["structuredContent"]["current_reading_step"]["suggested_tool"]["suggested_arguments"]
                ["locations"],
            json!([
                {"start_line": 4, "end_line": 4},
                {"start_line": 7, "end_line": 8}
            ])
        );
        assert_eq!(
            located_route_result["structuredContent"]["execution_plan"][1]["suggested_tool"]["suggested_arguments"]
                ["locations"],
            json!([
                {"start_line": 4, "end_line": 4},
                {"start_line": 7, "end_line": 8}
            ])
        );
        assert_eq!(
            located_route_result["structuredContent"]["impact_seed_files"][0].as_str(),
            Some("auth.py")
        );

        let callers_result = handle_tool_call(json!({
            "name": "callers",
            "arguments": {
                "root": dir.path(),
                "symbol": "helper"
            }
        }))
        .unwrap();
        assert_eq!(
            callers_result["structuredContent"][0]["caller"].as_str(),
            Some("AuthService.login")
        );

        let callees_result = handle_tool_call(json!({
            "name": "callees",
            "arguments": {
                "root": dir.path(),
                "symbol": "AuthService.login"
            }
        }))
        .unwrap();
        assert_eq!(
            callees_result["structuredContent"][0]["callee"].as_str(),
            Some("helper")
        );
    }

    #[test]
    fn dependency_graph_filters_tool_arguments() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/auth.c"),
            r#"
#include "auth.h"
#include "audit.h"
#include <stdio.h>

int login(void) {
    return AUTH_OK;
}
"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("src/auth.h"), "#define AUTH_OK 1\n").unwrap();
        std::fs::write(dir.path().join("src/audit.h"), "#define AUDIT_OK 1\n").unwrap();

        handle_tool_call(json!({
            "name": "index_project",
            "arguments": {
                "root": dir.path(),
                "force": true
            }
        }))
        .unwrap();

        let graph_result = handle_tool_call(json!({
            "name": "dependency_graph",
            "arguments": {
                "root": dir.path(),
                "files": ["src/auth.h"],
                "languages": ["c"],
                "kinds": ["include"],
                "limit": 10
            }
        }))
        .unwrap();
        assert_eq!(graph_result["structuredContent"]["edges"].as_u64(), Some(1));
        assert_eq!(
            graph_result["structuredContent"]["summary"]["edges"].as_u64(),
            Some(1)
        );
        assert_eq!(
            graph_result["structuredContent"]["top_sources"][0]["source_file"].as_str(),
            Some("src/auth.c")
        );
        assert_eq!(
            graph_result["structuredContent"]["top_targets"][0]["target"].as_str(),
            Some("auth.h")
        );
        assert_eq!(
            graph_result["structuredContent"]["dependencies"][0]["source_file"].as_str(),
            Some("src/auth.c")
        );
        assert_eq!(
            graph_result["structuredContent"]["dependencies"][0]["resolved_file"].as_str(),
            Some("src/auth.h")
        );

        let paged_graph = handle_tool_call(json!({
            "name": "dependency_graph",
            "arguments": {
                "root": dir.path(),
                "limit": 1,
                "offset": 1
            }
        }))
        .unwrap();
        assert_eq!(paged_graph["structuredContent"]["edges"].as_u64(), Some(3));
        assert_eq!(paged_graph["structuredContent"]["limit"].as_u64(), Some(1));
        assert_eq!(paged_graph["structuredContent"]["offset"].as_u64(), Some(1));
        assert_eq!(
            paged_graph["structuredContent"]["page_size"].as_u64(),
            Some(1)
        );
        assert_eq!(
            paged_graph["structuredContent"]["has_more"].as_bool(),
            Some(true)
        );
        assert_eq!(
            paged_graph["structuredContent"]["dependencies"][0]["target"].as_str(),
            Some("audit.h")
        );
    }

    #[test]
    fn config_status_reports_parse_errors_as_structured_content() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".codeinsight")).unwrap();
        std::fs::write(
            dir.path().join(".codeinsight/config.toml"),
            "[impact_analysis\n",
        )
        .unwrap();

        let config_status_result = handle_tool_call(json!({
            "name": "config_status",
            "arguments": {
                "root": dir.path()
            }
        }))
        .unwrap();

        assert_eq!(
            config_status_result["structuredContent"]["exists"].as_bool(),
            Some(true)
        );
        assert_eq!(
            config_status_result["structuredContent"]["loaded"].as_bool(),
            Some(false)
        );
        assert!(
            config_status_result["structuredContent"]["parse_error"]
                .as_str()
                .is_some_and(|error| error.contains(".codeinsight/config.toml"))
        );
    }

    #[test]
    fn agent_route_schema_exposes_backend_fallback() {
        let tools = tool_definitions();
        let agent_route = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "agent_route")
            .unwrap();
        let fallback = &agent_route["inputSchema"]["properties"]["backend_evidence"]["properties"]
            ["use_as_fallback"];

        assert_eq!(fallback["type"], "boolean");
        assert_eq!(fallback["default"], false);
        let preferred = &agent_route["inputSchema"]["properties"]["backend_evidence"]["properties"]
            ["prefer_for_context"];
        assert_eq!(preferred["type"], "boolean");
        assert_eq!(preferred["default"], false);
        let include_impact = &agent_route["inputSchema"]["properties"]["include_impact"];
        assert_eq!(include_impact["type"], "boolean");
        assert_eq!(include_impact["default"], true);
        assert!(
            include_impact["description"]
                .as_str()
                .unwrap()
                .contains("fast first read")
        );
        let response_mode = &agent_route["inputSchema"]["properties"]["response_mode"];
        assert_eq!(response_mode["type"], "string");
        assert_eq!(response_mode["default"], "full");
        assert_eq!(
            response_mode["enum"],
            serde_json::json!(["full", "compact"])
        );
        let response_token_budget =
            &agent_route["inputSchema"]["properties"]["response_token_budget"];
        assert_eq!(response_token_budget["type"], "integer");
        assert_eq!(response_token_budget["minimum"], 500);
        assert!(
            response_token_budget["description"]
                .as_str()
                .unwrap()
                .contains("structured route payload")
        );

        let candidates = &agent_route["inputSchema"]["properties"]["backend_evidence"]["properties"]
            ["candidates"];
        assert_eq!(candidates["type"], "array");
        assert_eq!(candidates["maxItems"], 16);
        assert_eq!(candidates["items"]["required"][0], "file");
        assert_eq!(candidates["items"]["properties"]["file"]["maxLength"], 512);
        assert_eq!(
            candidates["items"]["properties"]["symbol"]["type"],
            "string"
        );
        assert_eq!(
            agent_route["inputSchema"]["properties"]["backend_evidence"]["properties"]["candidate_files"]
                ["items"]["maxLength"],
            512
        );
        assert_eq!(candidates["items"]["properties"]["score"]["type"], "number");
        assert_eq!(candidates["items"]["properties"]["evidence"]["maxItems"], 6);
        assert_eq!(
            agent_route["inputSchema"]["properties"]["backend_evidence"]["properties"]["evidence_sources"]
                ["maxItems"],
            12
        );
        let tool_results = &agent_route["inputSchema"]["properties"]["backend_evidence"]["properties"]
            ["tool_results"];
        assert_eq!(tool_results["type"], "object");
        assert_eq!(tool_results["minProperties"], 1);
        assert!(
            tool_results["description"]
                .as_str()
                .unwrap()
                .contains("at most 64 items across all pages per tool")
        );
        let search_graph = &tool_results["properties"]["search_graph"];
        assert_eq!(search_graph["oneOf"][0]["type"], "object");
        assert_eq!(search_graph["oneOf"][1]["type"], "array");
        assert_eq!(search_graph["oneOf"][1]["maxItems"], 16);
        assert_eq!(search_graph["oneOf"][1]["items"]["type"], "object");
        assert_eq!(
            tool_results["properties"]["get_code_snippet"]["oneOf"][0]["type"],
            "object"
        );
        assert_eq!(
            tool_results["properties"]["query_graph"]["oneOf"][0]["type"],
            "object"
        );
        assert_eq!(
            tool_results["properties"]["trace_path"]["oneOf"][0]["type"],
            "object"
        );
    }

    #[test]
    fn agent_first_read_schema_is_small_and_bounded() {
        let tools = tool_definitions();
        let first_read = tools
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["name"] == "agent_first_read")
            .unwrap();
        let properties = first_read["inputSchema"]["properties"].as_object().unwrap();

        assert_eq!(properties.len(), 9);
        assert_eq!(properties["token_budget"]["default"], 6000);
        assert_eq!(properties["response_token_budget"]["default"], 8000);
        assert_eq!(properties["response_token_budget"]["minimum"], 500);
        assert!(!properties.contains_key("include_impact"));
        assert!(!properties.contains_key("response_mode"));
        assert!(!properties.contains_key("backend_evidence"));
        assert_eq!(
            properties["backend"]["properties"]["provider"]["enum"],
            serde_json::json!(["codebase-memory-mcp"])
        );
        assert_eq!(
            properties["backend"]["properties"]["refresh_index"]["default"],
            false
        );
        assert_eq!(
            properties["backend"]["properties"]["timeout_ms"]["minimum"],
            1000
        );
        assert_eq!(
            properties["backend"]["properties"]["timeout_ms"]["maximum"],
            300000
        );
        assert_eq!(
            properties["backend"]["properties"]["timeout_ms"]["default"],
            60000
        );
        assert_eq!(
            properties["backend"]["properties"]["on_failure"]["enum"],
            serde_json::json!(["fallback_local", "error"])
        );
        assert_eq!(
            properties["backend"]["properties"]["on_failure"]["default"],
            "fallback_local"
        );
        let backend_candidates = &properties["backend_candidates"];
        assert_eq!(backend_candidates["type"], "object");
        assert_eq!(
            backend_candidates["properties"]["candidate_files"]["maxItems"],
            8
        );
        assert_eq!(
            backend_candidates["properties"]["candidates"]["maxItems"],
            8
        );
        assert_eq!(
            backend_candidates["properties"]["candidates"]["items"]["anyOf"],
            serde_json::json!([
                {"required": ["file"]},
                {"required": ["file_path"]}
            ])
        );
        assert_eq!(
            backend_candidates["properties"]["search_graph"]["type"],
            "object"
        );
        assert!(backend_candidates.get("required").is_none());
        assert_eq!(backend_candidates["anyOf"].as_array().unwrap().len(), 7);
        assert_eq!(
            first_read["inputSchema"]["required"],
            serde_json::json!(["root", "task"])
        );
    }

    #[test]
    fn codebase_memory_search_query_prioritizes_domain_terms() {
        let query = codebase_memory_search_query("find backend failure policy implementation");
        let keywords = query.split_whitespace().collect::<Vec<_>>();
        assert!(keywords.contains(&"backend"));
        assert!(keywords.contains(&"failure"));
        assert!(keywords.contains(&"policy"));
        assert!(!keywords.contains(&"find"));
        assert!(!keywords.contains(&"implementation"));
        assert!(keywords.len() <= 8);
        assert_eq!(query, "backend failure policy");
        assert_eq!(
            codebase_memory_search_query("find AgentFirstReadBackendFailurePolicy"),
            "AgentFirstReadBackendFailurePolicy"
        );
        let chinese_query = codebase_memory_search_query("修复安全漏洞");
        assert!(chinese_query.is_ascii());
        assert!(
            chinese_query
                .split_whitespace()
                .any(|word| word == "security")
        );
        assert_eq!(codebase_memory_search_query("find inspect"), "find inspect");
    }

    #[test]
    fn agent_first_read_backend_candidates_enforce_compact_contract() {
        let unknown_field = serde_json::from_value::<AgentFirstReadBackendCandidates>(json!({
            "provider": "codebase-memory-mcp",
            "candidate_files": ["src/main.rs"],
            "tool_results": {"search_graph": {"results": []}}
        }))
        .unwrap_err();
        assert!(unknown_field.to_string().contains("unknown field"));

        let too_many_files = AgentFirstReadBackendCandidates {
            provider: "codebase-memory-mcp".to_string(),
            candidate_files: (0..=AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT)
                .map(|index| format!("src/{index}.rs"))
                .collect(),
            candidates: Vec::new(),
            search_graph: None,
            evidence_sources: Vec::new(),
            confidence: None,
            latency_ms: None,
        };
        assert!(
            too_many_files
                .into_evidence()
                .unwrap_err()
                .to_string()
                .contains("at most 8 candidates")
        );

        let structured = serde_json::from_value::<AgentFirstReadBackendCandidates>(json!({
            "provider": "codebase-memory-mcp",
            "candidates": [
                {"file": "src/main.rs", "symbol": "main"},
                {"file": "src/lib.rs"}
            ],
            "evidence_sources": ["search_graph"]
        }))
        .unwrap()
        .into_evidence()
        .unwrap();
        assert_eq!(
            structured.candidate_files,
            vec!["src/main.rs".to_string(), "src/lib.rs".to_string()]
        );
        assert_eq!(structured.candidates.len(), 2);
        assert_eq!(structured.candidates[0].symbol.as_deref(), Some("main"));

        let search_graph = serde_json::from_value::<AgentFirstReadBackendCandidates>(json!({
            "provider": "codebase-memory-mcp",
            "candidates": [{
                "qualified_name": "fixture.src.auth.AuthService",
                "label": "Class",
                "file_path": "src/auth.py",
                "start_line": 4,
                "end_line": 7,
                "in_degree": 4,
                "out_degree": 2
            }]
        }))
        .unwrap()
        .into_evidence()
        .unwrap();
        assert_eq!(search_graph.candidate_files, vec!["src/auth.py"]);
        assert_eq!(
            search_graph.candidates[0].symbol.as_deref(),
            Some("AuthService")
        );
        assert_eq!(
            serde_json::to_value(&search_graph.candidates[0].locations).unwrap(),
            json!([{"start_line": 4, "end_line": 7}])
        );

        let complete_search_graph =
            serde_json::from_value::<AgentFirstReadBackendCandidates>(json!({
                "provider": "codebase-memory-mcp",
                "search_graph": {
                    "structuredContent": {
                        "total": 12,
                        "results": [{"file_path": "src/fallback.rs", "name": "fallback"}],
                        "semantic_results": (0..=AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT)
                            .map(|index| json!({
                                "file_path": format!("src/semantic-{index}.rs"),
                                "name": format!("semantic_{index}"),
                                "label": "Function"
                            }))
                            .collect::<Vec<_>>(),
                        "elapsed_ms": 17
                    }
                }
            }))
            .unwrap()
            .into_evidence()
            .unwrap();
        assert_eq!(
            complete_search_graph.candidates.len(),
            AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT
        );
        assert_eq!(
            complete_search_graph.candidate_files[0],
            "src/semantic-0.rs"
        );
        assert_eq!(
            complete_search_graph.evidence_count,
            AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT
        );
        assert_eq!(complete_search_graph.latency_ms, Some(17));
        assert_eq!(complete_search_graph.evidence_sources, vec!["search_graph"]);

        let fallback_results = serde_json::from_value::<AgentFirstReadBackendCandidates>(json!({
            "provider": "codebase-memory-mcp",
            "search_graph": {
                "results": [{"file_path": "src/fallback.rs", "name": "fallback"}],
                "semantic_results": []
            }
        }))
        .unwrap()
        .into_evidence()
        .unwrap();
        assert_eq!(fallback_results.candidate_files, vec!["src/fallback.rs"]);
        assert_eq!(fallback_results.evidence_count, 1);

        let raw_search_graph = agent_first_read_backend_evidence(&json!({
            "result": {
                "structuredContent": {
                    "results": [{"file_path": "src/raw.rs", "name": "raw_target"}],
                    "elapsed_ms": 9
                }
            }
        }))
        .unwrap();
        assert_eq!(
            raw_search_graph.provider,
            AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER
        );
        assert_eq!(raw_search_graph.candidate_files, vec!["src/raw.rs"]);
        assert_eq!(
            raw_search_graph.candidates[0].symbol.as_deref(),
            Some("raw_target")
        );
        assert_eq!(raw_search_graph.latency_ms, Some(9));

        assert!(codebase_memory_prefers_architecture_entrypoints(
            "我应该先读哪个文件"
        ));
        assert!(codebase_memory_prefers_architecture_entrypoints(
            "which file should I read first"
        ));
        assert!(codebase_memory_prefers_architecture_entrypoints(
            "where should I begin"
        ));
        assert!(!codebase_memory_prefers_architecture_entrypoints(
            "which authentication file validates JWT"
        ));
        assert_eq!(
            codebase_memory_search_code_pattern(
                "find the refreshed_repository_changed status handling"
            )
            .as_deref(),
            Some("refreshed_repository_changed")
        );
        assert_eq!(
            codebase_memory_search_code_pattern("fix authentication token validation").as_deref(),
            Some("authentication")
        );
        assert_eq!(
            codebase_memory_search_code_pattern("fix JWT validation in repository").as_deref(),
            Some("JWT")
        );
        let search_code = agent_first_read_search_code_evidence(&json!({
            "result": {
                "structuredContent": {
                    "results": [{
                        "file": "src/mcp.rs",
                        "node": "codebase_memory_agent_first_read_result_with_binary",
                        "qualified_name": "fixture.src.mcp.codebase_memory_agent_first_read_result_with_binary"
                    }],
                    "elapsed_ms": 6
                }
            }
        }))
        .unwrap();
        assert_eq!(search_code.candidate_files, vec!["src/mcp.rs"]);
        assert_eq!(
            search_code.candidates[0].symbol.as_deref(),
            Some("codebase_memory_agent_first_read_result_with_binary")
        );
        assert_eq!(
            search_code.candidates[0].source.as_deref(),
            Some("search_code")
        );
        assert!(search_code.prefer_for_context);
        assert_eq!(search_code.evidence_count, 1);
        assert_eq!(search_code.latency_ms, Some(6));

        let architecture = agent_first_read_architecture_evidence(
            &json!({
                "result": {
                    "structuredContent": {
                        "entry_points": [{
                            "file": "src/main.rs",
                            "name": "main",
                            "qualified_name": "fixture.src.main.main"
                        }],
                        "elapsed_ms": 4
                    }
                }
            }),
            "understand this repository",
        )
        .unwrap();
        assert_eq!(architecture.candidate_files, vec!["src/main.rs"]);
        assert_eq!(architecture.candidates[0].symbol.as_deref(), Some("main"));
        assert_eq!(
            architecture.candidates[0].source.as_deref(),
            Some("get_architecture.entry_points")
        );
        assert!(architecture.prefer_for_context);
        assert!(!architecture.use_as_fallback);
        assert_eq!(architecture.evidence_count, 1);
        assert_eq!(architecture.latency_ms, Some(4));

        let specific_architecture = agent_first_read_architecture_evidence(
            &json!({"entry_points": [{"file": "src/main.rs", "name": "main"}]}),
            "fix authentication token validation",
        )
        .unwrap();
        assert!(!specific_architecture.prefer_for_context);
        assert!(specific_architecture.use_as_fallback);
        assert!(
            agent_first_read_architecture_evidence(
                &json!({"entry_points": []}),
                "understand this repository"
            )
            .is_none()
        );

        let unsupported_backend = codebase_memory_agent_first_read_result(
            Path::new("/tmp/repo"),
            "find target",
            AgentFirstReadBackendConfig {
                provider: "unknown".to_string(),
                project: None,
                refresh_index: false,
                timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
                on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
            },
        )
        .unwrap_err();
        assert!(unsupported_backend.to_string().contains("unsupported"));

        let invalid_timeout = codebase_memory_agent_first_read_result(
            Path::new("/tmp/repo"),
            "find target",
            AgentFirstReadBackendConfig {
                provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                project: None,
                refresh_index: false,
                timeout_ms: CODEBASE_MEMORY_MIN_TIMEOUT_MS - 1,
                on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
            },
        )
        .unwrap_err();
        assert!(invalid_timeout.to_string().contains("backend.timeout_ms"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let backend_dir = TempDir::new().unwrap();
            let backend = backend_dir.path().join("fake-codebase-memory-mcp");
            std::fs::write(
                backend.as_os_str(),
                r#"#!/bin/sh
case "$2" in
  index_repository)
    [ ! -f "$0.indexed" ] || exit 8
    touch "$0.indexed"
    printf '%s\n' '{"project":"fixture","status":"indexed"}'
    ;;
  search_graph)
    printf '%s\n' '{"results":[{"file_path":"src/target.rs","name":"target"}],"elapsed_ms":5}'
    ;;
  *)
    exit 2
    ;;
esac
"#,
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&backend).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&backend, permissions).unwrap();

            let root = backend_dir.path().join("fixture");
            std::fs::create_dir_all(root.join("src")).unwrap();
            for file in ["main.rs", "target.rs", "reused.rs", "recovered.rs"] {
                std::fs::write(root.join("src").join(file), "fn fixture() {}\n").unwrap();
            }
            let result = codebase_memory_agent_first_read_result_with_binary(
                &root,
                "find target",
                AgentFirstReadBackendConfig {
                    provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                    project: Some("fixture".to_string()),
                    refresh_index: true,
                    timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
                    on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
                },
                backend.as_os_str(),
            )
            .unwrap();
            assert_eq!(result.index_status, "refreshed_forced");
            let evidence = result.evidence.unwrap();
            assert_eq!(evidence.provider, AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER);
            assert_eq!(evidence.candidate_files, vec!["src/target.rs"]);
            assert_eq!(evidence.candidates[0].symbol.as_deref(), Some("target"));
            assert_eq!(evidence.latency_ms, Some(5));

            let reuse_backend = backend_dir.path().join("reuse-codebase-memory-mcp");
            std::fs::write(
                &reuse_backend,
                r#"#!/bin/sh
case "$2" in
  index_repository)
    exit 9
    ;;
  search_graph)
    printf '%s\n' '{"results":[{"file_path":"src/reused.rs","name":"reused"}]}'
    ;;
esac
"#,
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&reuse_backend).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&reuse_backend, permissions).unwrap();
            let reused = codebase_memory_agent_first_read_result_with_binary(
                &root,
                "find reused",
                AgentFirstReadBackendConfig {
                    provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                    project: Some("fixture".to_string()),
                    refresh_index: false,
                    timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
                    on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
                },
                reuse_backend.as_os_str(),
            )
            .unwrap()
            .evidence
            .unwrap();
            assert_eq!(reused.candidate_files, vec!["src/reused.rs"]);

            let stale_backend = backend_dir.path().join("stale-codebase-memory-mcp");
            std::fs::write(
                &stale_backend,
                r#"#!/bin/sh
case "$2" in
  index_repository)
    touch "$0.refreshed"
    printf '%s\n' '{"project":"fixture","status":"indexed"}'
    ;;
  search_graph)
    if [ -f "$0.refreshed" ]; then
      printf '%s\n' '{"results":[{"file_path":"src/recovered.rs","name":"recovered"}]}'
    else
      printf '%s\n' '{"results":[{"file_path":"src/removed.rs","name":"removed"}]}'
    fi
    ;;
  *)
    exit 2
    ;;
esac
"#,
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&stale_backend).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&stale_backend, permissions).unwrap();
            let recovered = codebase_memory_agent_first_read_result_with_binary(
                &root,
                "find recovered",
                AgentFirstReadBackendConfig {
                    provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                    project: Some("fixture".to_string()),
                    refresh_index: false,
                    timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
                    on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
                },
                stale_backend.as_os_str(),
            )
            .unwrap();
            assert_eq!(recovered.index_status, "refreshed_stale_candidates");
            assert_eq!(
                recovered.evidence.unwrap().candidate_files,
                vec!["src/recovered.rs"]
            );

            let empty_backend = backend_dir.path().join("empty-codebase-memory-mcp");
            std::fs::write(
                &empty_backend,
                "#!/bin/sh\nprintf '%s\\n' '{\"results\":[]}'\n",
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&empty_backend).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&empty_backend, permissions).unwrap();
            let empty = codebase_memory_agent_first_read_result_with_binary(
                &root,
                "find missing",
                AgentFirstReadBackendConfig {
                    provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                    project: Some("fixture".to_string()),
                    refresh_index: false,
                    timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
                    on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
                },
                empty_backend.as_os_str(),
            )
            .unwrap()
            .evidence;
            assert!(empty.is_none());

            let architecture_backend = backend_dir.path().join("architecture-codebase-memory-mcp");
            std::fs::write(
                &architecture_backend,
                r#"#!/bin/sh
case "$2" in
  search_graph)
    exit 9
    ;;
  get_architecture)
    printf '%s\n' '{"entry_points":[{"file":"src/main.rs","name":"main"}],"elapsed_ms":3}'
    ;;
  *)
    exit 2
    ;;
esac
"#,
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&architecture_backend)
                .unwrap()
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&architecture_backend, permissions).unwrap();
            let architecture = codebase_memory_agent_first_read_result_with_binary(
                &root,
                "understand this repository",
                AgentFirstReadBackendConfig {
                    provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                    project: Some("fixture".to_string()),
                    refresh_index: false,
                    timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
                    on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
                },
                architecture_backend.as_os_str(),
            )
            .unwrap()
            .evidence
            .unwrap();
            assert_eq!(architecture.candidate_files, vec!["src/main.rs"]);
            assert!(architecture.prefer_for_context);
            assert_eq!(
                architecture.evidence_sources,
                vec!["get_architecture.entry_points"]
            );

            let search_code_backend = backend_dir.path().join("search-codebase-memory-mcp");
            std::fs::write(
                &search_code_backend,
                r#"#!/bin/sh
case "$2" in
  search_graph)
    printf '%s\n' '{"results":[]}'
    ;;
  search_code)
    printf '%s\n' '{"results":[{"file":"src/target.rs","qualified_name":"fixture.src.target.target"}],"elapsed_ms":2}'
    ;;
  get_architecture)
    printf '%s\n' '{"entry_points":[{"file":"src/main.rs","name":"main"}]}'
    ;;
  *)
    exit 2
    ;;
esac
"#,
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&search_code_backend)
                .unwrap()
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&search_code_backend, permissions).unwrap();
            let search_code = codebase_memory_agent_first_read_result_with_binary(
                &root,
                "find refreshed_repository_changed status",
                AgentFirstReadBackendConfig {
                    provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                    project: Some("fixture".to_string()),
                    refresh_index: false,
                    timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
                    on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
                },
                search_code_backend.as_os_str(),
            )
            .unwrap()
            .evidence
            .unwrap();
            assert_eq!(search_code.candidate_files, vec!["src/target.rs"]);
            assert_eq!(
                search_code.candidates[0].source.as_deref(),
                Some("search_code")
            );
            assert_eq!(search_code.evidence_sources, vec!["search_code"]);

            let slow_backend = backend_dir.path().join("slow-codebase-memory-mcp");
            std::fs::write(&slow_backend, "#!/bin/sh\nexec sleep 1\n").unwrap();
            let mut permissions = std::fs::metadata(&slow_backend).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&slow_backend, permissions).unwrap();
            let timeout_error = codebase_memory_cli_value(
                slow_backend.as_os_str(),
                "search_graph",
                &[],
                Duration::from_millis(20),
            )
            .unwrap_err();
            assert_eq!(
                timeout_error.to_string(),
                "codebase-memory-mcp search_graph timed out after 20 ms"
            );

            let budget_backend = backend_dir.path().join("budget-codebase-memory-mcp");
            std::fs::write(
                &budget_backend,
                r#"#!/bin/sh
case "$2" in
  search_graph)
    sleep 0.2
    printf '%s\n' '{"results":[]}'
    ;;
  search_code)
    exec sleep 2
    ;;
  *)
    exit 2
    ;;
esac
"#,
            )
            .unwrap();
            let mut permissions = std::fs::metadata(&budget_backend).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&budget_backend, permissions).unwrap();
            let started = Instant::now();
            let timeout_error = codebase_memory_agent_first_read_result_with_binary(
                &root,
                "find refreshed_repository_changed status",
                AgentFirstReadBackendConfig {
                    provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                    project: Some("fixture".to_string()),
                    refresh_index: false,
                    timeout_ms: CODEBASE_MEMORY_MIN_TIMEOUT_MS,
                    on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
                },
                budget_backend.as_os_str(),
            )
            .unwrap_err();
            let elapsed = started.elapsed();
            assert!(
                elapsed < Duration::from_millis(1_500),
                "shared timeout budget took {elapsed:?}"
            );
            let timeout_message = timeout_error.to_string();
            assert!(
                ["search_graph", "search_code"]
                    .iter()
                    .any(|stage| timeout_message
                        == format!(
                            "codebase-memory-mcp backend timeout budget of 1000 ms exhausted before or during {stage}"
                        )),
                "unexpected shared timeout error: {timeout_message}"
            );
        }

        let mixed = serde_json::from_value::<AgentFirstReadBackendCandidates>(json!({
            "provider": "codebase-memory-mcp",
            "candidate_files": ["src/main.rs"],
            "search_graph": {"results": [{"file_path": "src/lib.rs"}]}
        }))
        .unwrap()
        .into_evidence()
        .unwrap_err();
        assert!(mixed.to_string().contains("cannot be combined"));

        let empty = serde_json::from_value::<AgentFirstReadBackendCandidates>(json!({
            "provider": "codebase-memory-mcp"
        }))
        .unwrap()
        .into_evidence()
        .unwrap_err();
        assert!(
            empty
                .to_string()
                .contains("candidate_files, candidates, or search_graph")
        );

        let missing_path = serde_json::from_value::<AgentFirstReadBackendCandidates>(json!({
            "provider": "codebase-memory-mcp",
            "candidates": [{"name": "AuthService"}]
        }))
        .unwrap()
        .into_evidence()
        .unwrap_err();
        assert!(missing_path.to_string().contains("file or file_path"));
    }

    #[test]
    #[cfg(unix)]
    fn codebase_memory_refreshes_only_when_repository_content_changes() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let root = dir.path().join("fixture");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("target.rs"), "fn target() {}\n").unwrap();

        let git = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init"]);
        git(&["config", "user.email", "codeinsight@example.invalid"]);
        git(&["config", "user.name", "CodeInsight Test"]);
        git(&["add", "target.rs"]);
        git(&["commit", "-m", "fixture"]);

        let backend = dir.path().join("fake-codebase-memory-mcp");
        std::fs::write(
            &backend,
            r#"#!/bin/sh
marker="$(dirname "$0")/index-count"
case "$2" in
  query_graph)
    printf '%s\n' '{"columns":["b.head_sha"],"rows":[["stale"]]}'
    ;;
  index_repository)
    printf x >> "$marker"
    printf '%s\n' '{"project":"fixture","status":"indexed"}'
    ;;
  search_graph)
    printf '%s\n' '{"results":[{"file_path":"target.rs","name":"target"}]}'
    ;;
  *)
    exit 2
    ;;
esac
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&backend).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&backend, permissions).unwrap();
        let marker = dir.path().join("index-count");
        let config = || AgentFirstReadBackendConfig {
            provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
            project: Some("fixture".to_string()),
            refresh_index: false,
            timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
            on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
        };
        let route = || {
            codebase_memory_agent_first_read_result_with_binary(
                &root,
                "find target",
                config(),
                backend.as_os_str(),
            )
            .unwrap()
        };

        let result = route();
        assert_eq!(result.index_status, "refreshed_stale_graph");
        assert_eq!(result.evidence.unwrap().candidate_files, vec!["target.rs"]);
        assert_eq!(std::fs::read(&marker).unwrap().len(), 1);
        let cache_key = codebase_memory_index_cache_key(&root, "fixture", backend.as_os_str());
        CODEBASE_MEMORY_INDEX_FINGERPRINTS
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .remove(&cache_key);
        let result = route();
        assert_eq!(result.index_status, "reused_cached_fingerprint");
        assert_eq!(result.evidence.unwrap().candidate_files, vec!["target.rs"]);
        assert_eq!(std::fs::read(&marker).unwrap().len(), 1);

        std::fs::write(root.join("target.rs"), "fn target() { changed(); }\n").unwrap();
        let result = route();
        assert_eq!(result.index_status, "refreshed_repository_changed");
        assert_eq!(result.evidence.unwrap().candidate_files, vec!["target.rs"]);
        assert_eq!(std::fs::read(&marker).unwrap().len(), 2);
        let result = route();
        assert_eq!(result.index_status, "reused_cached_fingerprint");
        assert_eq!(result.evidence.unwrap().candidate_files, vec!["target.rs"]);
        assert_eq!(std::fs::read(&marker).unwrap().len(), 2);

        std::fs::write(root.join("target.rs"), "fn target() {}\n").unwrap();
        let result = route();
        assert_eq!(result.index_status, "refreshed_repository_changed");
        assert_eq!(result.evidence.unwrap().candidate_files, vec!["target.rs"]);
        assert_eq!(std::fs::read(&marker).unwrap().len(), 3);
        let result = route();
        assert_eq!(result.index_status, "reused_cached_fingerprint");
        assert_eq!(result.evidence.unwrap().candidate_files, vec!["target.rs"]);
        assert_eq!(std::fs::read(&marker).unwrap().len(), 3);
    }

    #[test]
    #[cfg(unix)]
    fn codebase_memory_reuses_graph_at_current_head() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let root = dir.path().join("fixture-current");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("target.rs"), "fn target() {}\n").unwrap();
        for args in [
            vec!["init"],
            vec!["config", "user.email", "codeinsight@example.invalid"],
            vec!["config", "user.name", "CodeInsight Test"],
            vec!["add", "target.rs"],
            vec!["commit", "-m", "fixture"],
        ] {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(&root)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        let head = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(&root)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap();

        let backend = dir.path().join("current-codebase-memory-mcp");
        std::fs::write(
            &backend,
            format!(
                r#"#!/bin/sh
case "$2" in
  query_graph)
    printf '%s\n' '{{"columns":["b.head_sha"],"rows":[["{}"]]}}'
    ;;
  index_repository)
    exit 9
    ;;
  search_graph)
    printf '%s\n' '{{"results":[{{"file_path":"target.rs","name":"target"}}]}}'
    ;;
esac
"#,
                head.trim()
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&backend).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&backend, permissions).unwrap();

        let result = codebase_memory_agent_first_read_result_with_binary(
            &root,
            "find target",
            AgentFirstReadBackendConfig {
                provider: AGENT_FIRST_READ_SEARCH_GRAPH_PROVIDER.to_string(),
                project: Some("fixture-current".to_string()),
                refresh_index: false,
                timeout_ms: CODEBASE_MEMORY_TIMEOUT_MS,
                on_failure: AgentFirstReadBackendFailurePolicy::FallbackLocal,
            },
            backend.as_os_str(),
        )
        .unwrap();
        assert_eq!(result.index_status, "reused_graph_head");
        assert_eq!(result.evidence.unwrap().candidate_files, vec!["target.rs"]);
    }

    #[test]
    fn rejects_missing_required_arguments() {
        let error = handle_tool_call(json!({
            "name": "symbol_search",
            "arguments": {
                "query": "AuthService"
            }
        }))
        .unwrap_err();
        assert!(error.to_string().contains("root"));
    }

    #[test]
    fn compact_tool_text_exposes_stale_symbol_recovery() {
        let result = json!({
            "response_mode": "compact",
            "routing_decision": {
                "first_file": "src/main.ts",
                "route_quality": {
                    "recommended_action": "inspect_file_outline_for_current_symbol"
                },
                "backend_route_agreement": {
                    "status": "backend_fallback",
                    "provider": "codebase-memory-mcp",
                    "candidate_dispositions": [{
                        "context_suggested_tool": {
                            "tool": "agent_first_read"
                        },
                        "symbol_status": "stale",
                        "symbol_suggested_tool": {
                            "tool": "file_outline"
                        }
                    }]
                },
                "backend_selected_candidate": {
                    "symbol": "removedGraphSymbol"
                }
            },
            "context_pack": {
                "budget": {
                    "selected_files": 1,
                    "selected_ranges": 2
                }
            },
            "impact_status": "deferred_by_request"
        });

        let text = tool_text_content("agent_first_read", &result).unwrap();

        assert!(text.contains("backend_symbol=removedGraphSymbol"));
        assert!(text.contains("candidate_recovery_tool=agent_first_read"));
        assert!(text.contains("backend_symbol_status=stale"));
        assert!(text.contains("symbol_recovery_tool=file_outline"));
        assert!(text.contains("next_action=inspect_file_outline_for_current_symbol"));

        let location_result = json!({
            "response_mode": "compact",
            "routing_decision": {
                "first_file": "src/server.ts",
                "route_quality": {
                    "recommended_action": "inspect_file_outline_for_current_location"
                },
                "backend_route_agreement": {
                    "status": "backend_preferred",
                    "provider": "codebase-memory-mcp",
                    "candidate_dispositions": [{
                        "location_status": "stale",
                        "location_suggested_tool": {
                            "tool": "file_outline"
                        }
                    }]
                },
                "backend_selected_candidate": {
                    "symbol": "startServer"
                }
            },
            "context_pack": {
                "budget": {
                    "selected_files": 1,
                    "selected_ranges": 1
                }
            },
            "impact_status": "deferred_by_request"
        });

        let location_text = tool_text_content("agent_route", &location_result).unwrap();

        assert!(location_text.contains("backend_location_status=stale"));
        assert!(location_text.contains("location_recovery_tool=file_outline"));
        assert!(location_text.contains("next_action=inspect_file_outline_for_current_location"));
    }

    #[test]
    fn agent_route_rejects_response_budget_in_full_mode() {
        let error = handle_tool_call(json!({
            "name": "agent_route",
            "arguments": {
                "root": ".",
                "task": "inspect routing",
                "response_token_budget": 2500
            }
        }))
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "response_token_budget requires compact response_mode"
        );
    }

    #[test]
    fn agent_first_read_rejects_automatic_and_manual_backend_evidence() {
        let error = handle_tool_call(json!({
            "name": "agent_first_read",
            "arguments": {
                "root": ".",
                "task": "inspect routing",
                "backend": {
                    "provider": "codebase-memory-mcp"
                },
                "backend_candidates": {
                    "results": [{"file_path": "src/lib.rs"}]
                }
            }
        }))
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "agent_first_read backend cannot be combined with backend_candidates"
        );
    }

    #[test]
    fn rejects_invalid_argument_shapes() {
        let error = handle_tool_call(json!({
            "name": "symbol_search",
            "arguments": []
        }))
        .unwrap_err();
        assert!(error.to_string().contains("arguments"));

        let error = handle_tool_call(json!({
            "name": "symbol_search",
            "arguments": {
                "root": ".",
                "query": "AuthService",
                "limit": 0
            }
        }))
        .unwrap_err();
        assert!(error.to_string().contains("limit"));

        let error = handle_tool_call(json!({
            "name": "context_pack",
            "arguments": {
                "root": ".",
                "task": "x",
                "token_budget": 0
            }
        }))
        .unwrap_err();
        assert!(error.to_string().contains("token_budget"));

        let error = handle_tool_call(json!({
            "name": "file_outline",
            "arguments": {
                "path": ".",
                "locations": [{"start_line": 9, "end_line": 4}]
            }
        }))
        .unwrap_err();
        assert!(error.to_string().contains("end_line"));
    }

    fn is_known_context_source(source: &str) -> bool {
        matches!(
            source,
            "seed_file"
                | "symbol_definition"
                | "reference"
                | "call_graph"
                | "semantic"
                | "dependency"
        )
    }
}
