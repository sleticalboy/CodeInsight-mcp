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
