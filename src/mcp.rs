use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::{cli::Transport, tools};

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
            _ => json_error(
                id,
                -32601,
                format!("method not implemented in MVP scaffold: {method}"),
            ),
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
            let limit = optional_positive_usize(&arguments, "limit", 500)?;
            serde_json::to_value(tools::dependency_graph_value(root, limit)?)?
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

    Ok(json!({
        "content": [
            {
                "type": "text",
                "text": serde_json::to_string_pretty(&result)?
            }
        ],
        "structuredContent": result
    }))
}

fn tool_definitions() -> Value {
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
            "name": "project_overview",
            "description": "Return indexed project language, file, symbol, and directory stats.",
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
                    "limit": {"type": "integer", "minimum": 1}
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
            "description": "Build an agent-ready context pack from seed symbols, seed files, and a token budget.",
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
            !impact_result["structuredContent"]["top_reasons"]
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
            context_result["structuredContent"]["files"][0]["file"].as_str(),
            Some("auth.py")
        );
        assert!(
            context_result["structuredContent"]["files"][0]["ranges"][0]["reason"]
                .as_str()
                .is_some_and(|reason| !reason.is_empty())
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
                "symbols": []
            }
        }))
        .unwrap_err();
        assert!(error.to_string().contains("symbol or file"));
    }
}
