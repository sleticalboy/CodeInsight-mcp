use anyhow::{Result, bail};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use crate::cli::Transport;

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
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
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
                            "name": "symbol_search",
                            "description": "Search symbols in an indexed repository.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "root": {"type": "string"},
                                    "query": {"type": "string"},
                                    "limit": {"type": "integer"}
                                },
                                "required": ["root", "query"]
                            }
                        }
                    ]
                }
            }),
            "notifications/initialized" => continue,
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("method not implemented in MVP scaffold: {method}")
                }
            }),
        };

        stdout.write_all(response.to_string().as_bytes()).await?;
        stdout.write_all(b"\n").await?;
        stdout.flush().await?;
    }

    Ok(())
}

#[allow(dead_code)]
fn unsupported(message: &str) -> Result<()> {
    bail!("{message}")
}
