#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [ -z "${CODEINSIGHT_BIN:-}" ]; then
  CODEINSIGHT_BIN="$ROOT_DIR/target/release/codeinsight"
  BUILD_LOCAL_BINARY=true
else
  BUILD_LOCAL_BINARY=false
fi
SMOKE_ROOT="${CODEINSIGHT_SMOKE_ROOT:-}"
SMOKE_SYMBOL="${CODEINSIGHT_SMOKE_SYMBOL:-AuthService}"
TEMP_DIR=""

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

build_binary_if_needed() {
  if [ "$BUILD_LOCAL_BINARY" = true ] || [ ! -x "$CODEINSIGHT_BIN" ]; then
    cargo build --locked --release --manifest-path "$ROOT_DIR/Cargo.toml"
  fi
}

create_fixture() {
  TEMP_DIR="$(mktemp -d)"
  SMOKE_ROOT="$TEMP_DIR/repo"
  mkdir -p "$SMOKE_ROOT/src"

  cat >"$SMOKE_ROOT/src/auth.py" <<'EOF'
class AuthService:
    def login(self):
        return helper()

def helper():
    return "ok"
EOF

  cat >"$SMOKE_ROOT/src/main.ts" <<'EOF'
import { render } from "./ui";

export function main() {
  return render();
}
EOF

  cat >"$SMOKE_ROOT/src/ui.ts" <<'EOF'
export function render() {
  return "ok";
}
EOF
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

main() {
  require_command cargo
  require_command python3
  build_binary_if_needed

  if [ -z "$SMOKE_ROOT" ]; then
    create_fixture
  fi
  SMOKE_ROOT="$(cd "$SMOKE_ROOT" && pwd)"

  trap cleanup EXIT INT TERM

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" SMOKE_ROOT="$SMOKE_ROOT" SMOKE_SYMBOL="$SMOKE_SYMBOL" python3 <<'PY'
import json
import os
import subprocess

codeinsight_bin = os.environ["CODEINSIGHT_BIN"]
smoke_root = os.environ["SMOKE_ROOT"]
smoke_symbol = os.environ["SMOKE_SYMBOL"]

proc = subprocess.Popen(
    [codeinsight_bin, "serve", "--transport", "stdio"],
    stdin=subprocess.PIPE,
    stdout=subprocess.PIPE,
    stderr=subprocess.PIPE,
    text=True,
)


def request(payload):
    assert proc.stdin is not None
    assert proc.stdout is not None
    proc.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        stderr = proc.stderr.read() if proc.stderr is not None else ""
        raise RuntimeError(f"server exited before response: {stderr}")
    response = json.loads(line)
    if "error" in response:
        raise RuntimeError(json.dumps(response["error"], indent=2))
    return response


try:
    initialize = request({"jsonrpc": "2.0", "id": 1, "method": "initialize"})
    assert initialize["result"]["serverInfo"]["name"] == "codeinsight"
    assert "tools" in initialize["result"]["capabilities"]

    tools = request({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
    tool_names = {tool["name"] for tool in tools["result"]["tools"]}
    for expected in ("index_project", "project_overview", "config_status", "symbol_search", "impact_analysis", "embedding_status", "context_pack", "version"):
        assert expected in tool_names, expected

    config_status = request(
        {
            "jsonrpc": "2.0",
            "id": 9,
            "method": "tools/call",
            "params": {
                "name": "config_status",
                "arguments": {"root": smoke_root},
            },
        }
    )
    assert config_status["result"]["structuredContent"]["exists"] is False
    assert "detected_test_commands" in config_status["result"]["structuredContent"]

    indexed = request(
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "index_project",
                "arguments": {"root": smoke_root, "force": True},
            },
        }
    )
    index_result = indexed["result"]["structuredContent"]
    assert index_result["indexed_files"] >= 1
    assert len(index_result["errors"]) == 0

    overview = request(
        {
            "jsonrpc": "2.0",
            "id": 10,
            "method": "tools/call",
            "params": {
                "name": "project_overview",
                "arguments": {"root": smoke_root},
            },
        }
    )
    overview_result = overview["result"]["structuredContent"]
    assert overview_result["indexed_files"] == index_result["indexed_files"]
    assert overview_result["entrypoints"], "expected entrypoint candidates"
    assert any(
        entrypoint["file"] == "src/main.ts"
        and entrypoint["role"] == "source"
        for entrypoint in overview_result["entrypoints"]
    ), "source entrypoint not found"

    symbols = request(
        {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "symbol_search",
                "arguments": {"root": smoke_root, "query": smoke_symbol, "limit": 5},
            },
        }
    )
    assert any(
        symbol["name"] == smoke_symbol
        for symbol in symbols["result"]["structuredContent"]
    ), f"symbol not found: {smoke_symbol}"

    impact = request(
        {
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "impact_analysis",
                "arguments": {
                    "root": smoke_root,
                    "symbols": [smoke_symbol],
                    "files": ["src/auth.py"],
                    "limit": 10,
                    "depth": 2,
                    "format": "summary",
                    "evidence_limit": 2,
                },
            },
        }
    )
    assert impact["result"]["structuredContent"]["depth"] == 2
    assert impact["result"]["structuredContent"]["format"] == "summary"
    assert impact["result"]["structuredContent"]["risk_level"] in ("low", "medium", "high")
    assert impact["result"]["structuredContent"]["impact_counts"]["impacted_files"] >= 1
    assert len(impact["result"]["structuredContent"]["suggested_checks"]) >= 1
    assert any(
        item["file"] == "src/auth.py"
        for item in impact["result"]["structuredContent"]["impacted_files"]
    ), "impact file not found"

    embedding_status = request(
        {
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "embedding_status",
                "arguments": {"root": smoke_root},
            },
        }
    )
    embedding_result = embedding_status["result"]["structuredContent"]
    assert embedding_result["provider"] == "disabled"
    assert embedding_result["index"]["vector_status"] == "semantic_chunks_missing"

    version = request(
        {
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "version",
                "arguments": {},
            },
        }
    )
    version_result = version["result"]["structuredContent"]
    assert version_result["name"] == "codeinsight"
    assert version_result["version"]

    context = request(
        {
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "context_pack",
                "arguments": {
                    "root": smoke_root,
                    "task": f"understand {smoke_symbol}",
                    "symbols": [smoke_symbol],
                    "token_budget": 1200,
                },
            },
        }
    )
    assert any(
        symbol["name"] == smoke_symbol
        for symbol in context["result"]["structuredContent"]["symbols"]
    ), f"context symbol not found: {smoke_symbol}"
    assert context["result"]["structuredContent"]["seed_strategy"] == "explicit"
    assert any(
        seed["kind"] == "symbol"
        and seed["value"] == smoke_symbol
        and seed["source"] == "explicit"
        for seed in context["result"]["structuredContent"]["selected_seeds"]
    ), "explicit context seed not found"

    auto_context = request(
        {
            "jsonrpc": "2.0",
            "id": 11,
            "method": "tools/call",
            "params": {
                "name": "context_pack",
                "arguments": {
                    "root": smoke_root,
                    "task": "understand application entrypoint",
                    "token_budget": 1200,
                },
            },
        }
    )
    auto_context_result = auto_context["result"]["structuredContent"]
    assert auto_context_result["seed_strategy"] == "auto_entrypoint"
    assert any(
        seed["kind"] == "file"
        and seed["value"] == "src/main.ts"
        and seed["source"] == "overview_entrypoint"
        and seed["role"] == "source"
        for seed in auto_context_result["selected_seeds"]
    ), "auto entrypoint seed not found"
    assert any(
        item["file"] == "src/main.ts"
        for item in auto_context_result["files"]
    ), "auto context entrypoint file not selected"

    print("MCP stdio smoke passed")
    print(f"root: {smoke_root}")
    print(f"symbol: {smoke_symbol}")
    print(f"tools: {len(tool_names)}")
    print(f"indexed_files: {index_result['indexed_files']}")
    print(f"overview_entrypoints: {len(overview_result['entrypoints'])}")
    print(f"auto_seed_strategy: {auto_context_result['seed_strategy']}")
finally:
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=5)
PY
}

main "$@"
