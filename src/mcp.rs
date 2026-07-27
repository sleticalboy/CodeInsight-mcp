use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{
    cli::Transport,
    model::{AgentRouteBackendCandidate, AgentRouteBackendEvidence},
    tools,
};

const AGENT_FIRST_READ_RESPONSE_TOKEN_BUDGET: usize = 8_000;
const AGENT_FIRST_READ_BACKEND_CANDIDATE_LIMIT: usize = 8;
const AGENT_FIRST_READ_BACKEND_EVIDENCE_SOURCE_LIMIT: usize = 6;

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

        Ok(AgentRouteBackendCandidate {
            file,
            symbol,
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
    let mut payload = search_graph;
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
            serde_json::to_value(tools::file_outline_value(path)?)?
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
            let backend_evidence = optional_json_object::<AgentFirstReadBackendCandidates>(
                &arguments,
                "backend_candidates",
            )?
            .map(AgentFirstReadBackendCandidates::into_evidence)
            .transpose()?;
            let report = tools::agent_route_value(
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
            )?;
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
        .unwrap_or("-");
    let impact_status = result
        .get("impact_status")
        .and_then(Value::as_str)
        .unwrap_or("unknown");

    Ok(format!(
        "Compact agent route: first_file={first_file}; selected_files={selected_files}; selected_ranges={selected_ranges}; backend_status={backend_status}; backend_provider={backend_provider}; backend_symbol={backend_symbol}; next_action={next_action}; impact_status={impact_status}. Read structuredContent for selected excerpts and execution_plan."
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
        "description": "Optional compact candidate ranking from an external code graph. Pass candidate_files for file-only routing, candidates to preserve exact symbols, or a complete search_graph response. When no explicit seed is provided, CodeInsight prefers this ranking as bounded context seeds.",
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
                        "name": {"type": "string", "maxLength": 512}
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
            "evidence_sources": {
                "type": "array",
                "maxItems": 6,
                "items": {"type": "string", "maxLength": 160}
            },
            "confidence": {"type": "number"},
            "latency_ms": {"type": "integer", "minimum": 0}
        },
        "required": ["provider"],
        "anyOf": [
            {"required": ["candidate_files"]},
            {"required": ["candidates"]},
            {"required": ["search_graph"]}
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
                    "path": {"type": "string"}
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
                        "items": {"type": "string"}
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
                        "items": {"type": "string"}
                    },
                    "token_budget": {"type": "integer", "minimum": 500, "default": 6000},
                    "response_token_budget": {
                        "type": "integer",
                        "minimum": 500,
                        "default": 8000,
                        "description": "Hard cap for the compact structured route payload. The MCP envelope and concise text summary are excluded."
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
                        "items": {"type": "string"}
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
                "files": ["auth.py"],
                "token_budget": 1200
            }
        }))
        .unwrap();
        assert_eq!(
            file_context_result["structuredContent"]["files"][0]["file"].as_str(),
            Some("auth.py")
        );
        assert!(
            file_context_result["structuredContent"]["summary"]
                .as_str()
                .unwrap()
                .contains("seed files")
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

        assert_eq!(properties.len(), 8);
        assert_eq!(properties["token_budget"]["default"], 6000);
        assert_eq!(properties["response_token_budget"]["default"], 8000);
        assert_eq!(properties["response_token_budget"]["minimum"], 500);
        assert!(!properties.contains_key("include_impact"));
        assert!(!properties.contains_key("response_mode"));
        assert!(!properties.contains_key("backend_evidence"));
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
        assert_eq!(
            backend_candidates["required"],
            serde_json::json!(["provider"])
        );
        assert_eq!(backend_candidates["anyOf"].as_array().unwrap().len(), 3);
        assert_eq!(
            first_read["inputSchema"]["required"],
            serde_json::json!(["root", "task"])
        );
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
