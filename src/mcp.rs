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
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));

    let result = match name {
        "index_project" => {
            let root = required_path(&arguments, "root")?;
            let force = arguments
                .get("force")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            serde_json::to_value(tools::index_project_value(root, force)?)?
        }
        "project_overview" => {
            let root = required_path(&arguments, "root")?;
            serde_json::to_value(tools::project_overview_value(root)?)?
        }
        "symbol_search" => {
            let root = required_path(&arguments, "root")?;
            let query = required_str(&arguments, "query")?;
            let limit = arguments.get("limit").and_then(Value::as_u64).unwrap_or(20) as usize;
            serde_json::to_value(tools::symbol_search_value(root, query, limit)?)?
        }
        "file_outline" => {
            let path = required_path(&arguments, "path")?;
            serde_json::to_value(tools::file_outline_value(path)?)?
        }
        "dependency_graph" => {
            let root = required_path(&arguments, "root")?;
            let limit = arguments
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(500) as usize;
            serde_json::to_value(tools::dependency_graph_value(root, limit)?)?
        }
        "find_references" => {
            let root = required_path(&arguments, "root")?;
            let symbol = required_str(&arguments, "symbol")?;
            let limit = arguments
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(100) as usize;
            let include_definitions = arguments
                .get("include_definitions")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            serde_json::to_value(tools::find_references_value(
                root,
                symbol,
                limit,
                include_definitions,
            )?)?
        }
        "context_pack" => {
            let root = required_path(&arguments, "root")?;
            let task = required_str(&arguments, "task")?.to_string();
            let symbols = required_string_array(&arguments, "symbols")?;
            let token_budget = arguments
                .get("token_budget")
                .and_then(Value::as_u64)
                .unwrap_or(6000) as usize;
            serde_json::to_value(tools::context_pack_value(
                root,
                task,
                symbols,
                token_budget,
            )?)?
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
            "name": "context_pack",
            "description": "Build an agent-ready context pack from seed symbols and a token budget.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "root": {"type": "string"},
                    "task": {"type": "string"},
                    "symbols": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "token_budget": {"type": "integer", "minimum": 500}
                },
                "required": ["root", "task", "symbols"]
            }
        }
    ])
}

fn required_path(arguments: &Value, key: &str) -> Result<PathBuf> {
    Ok(PathBuf::from(required_str(arguments, key)?))
}

fn required_str<'a>(arguments: &'a Value, key: &str) -> Result<&'a str> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .with_context(|| format!("missing or invalid string argument: {key}"))
}

fn required_string_array(arguments: &Value, key: &str) -> Result<Vec<String>> {
    let values = arguments
        .get(key)
        .and_then(Value::as_array)
        .with_context(|| format!("missing or invalid string array argument: {key}"))?;
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .with_context(|| format!("invalid non-string value in argument: {key}"))
        })
        .collect()
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
        pass
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
}
