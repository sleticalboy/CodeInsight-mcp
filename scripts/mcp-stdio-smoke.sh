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

  for index in $(seq 1 30); do
    cat >"$SMOKE_ROOT/src/consumer_${index}.py" <<'EOF'
from auth import AuthService

def build_service():
    details = "this intentionally verbose service reference line includes many descriptive words so that budget pressure can omit lower ranked context candidates during smoke testing and continue with focused follow up recommendations"
    return AuthService().login(), details
EOF
  done

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


def same_root(path):
    return os.path.samefile(path, smoke_root)


def call_suggested_tool(suggested_tool, request_id):
    return request(
        {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {
                "name": suggested_tool["tool"],
                "arguments": suggested_tool["suggested_arguments"],
            },
        }
    )["result"]["structuredContent"]


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
    recommended_tools = overview_result["recommended_next_tools"]
    overview_context_tool = next(
        (
            tool
            for tool in recommended_tools
            if tool["tool"] == "context_pack"
            and tool["priority"] == 10
            and same_root(tool["suggested_arguments"]["root"])
            and tool["suggested_arguments"]["token_budget"] == 6000
        ),
        None,
    )
    overview_config_tool = next(
        (
            tool
            for tool in recommended_tools
            if tool["tool"] == "config_status"
            and tool["priority"] == 80
            and same_root(tool["suggested_arguments"]["root"])
        ),
        None,
    )
    assert any(
        tool["tool"] == "context_pack"
        and tool["priority"] == 10
        and same_root(tool["suggested_arguments"]["root"])
        and tool["suggested_arguments"]["token_budget"] == 6000
        for tool in recommended_tools
    ), "context_pack recommendation not found"
    assert any(
        tool["tool"] == "impact_analysis"
        and tool["priority"] == 40
        and tool["suggested_arguments"]["files"] == ["src/main.ts"]
        and tool["suggested_arguments"]["symbols"] == ["main"]
        for tool in recommended_tools
    ), "impact_analysis recommendation not found"
    assert any(
        tool["tool"] == "config_status"
        and tool["priority"] == 80
        and same_root(tool["suggested_arguments"]["root"])
        for tool in recommended_tools
    ), "config_status recommendation not found"
    overview_context_result = call_suggested_tool(overview_context_tool, 14)
    assert overview_context_result["seed_strategy"] == "auto_entrypoint"
    assert overview_context_result["reading_plan"], "overview context_pack recommendation produced no reading_plan"
    overview_config_result = call_suggested_tool(overview_config_tool, 15)
    assert overview_config_result["exists"] is False
    assert "detected_test_commands" in overview_config_result

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
                    "token_budget": 500,
                },
            },
        }
    )
    assert any(
        symbol["name"] == smoke_symbol
        for symbol in context["result"]["structuredContent"]["symbols"]
    ), f"context symbol not found: {smoke_symbol}"
    context_result = context["result"]["structuredContent"]
    assert context_result["seed_strategy"] == "explicit"
    assert any(
        seed["kind"] == "symbol"
        and seed["value"] == smoke_symbol
        and seed["source"] == "explicit"
        for seed in context_result["selected_seeds"]
    ), "explicit context seed not found"
    assert context_result["budget"]["requested_token_budget"] == 500
    assert context_result["budget"]["applied_token_budget"] == 500
    assert context_result["budget"]["estimated_tokens"] == context_result["estimated_tokens"]
    assert context_result["budget"]["truncated"] == context_result["truncated"]
    assert context_result["budget"]["candidate_files"] >= context_result["budget"]["selected_files"]
    assert context_result["budget"]["candidate_ranges"] >= context_result["budget"]["selected_ranges"]
    assert context_result["budget"]["omitted_files"] == (
        context_result["budget"]["candidate_files"] - context_result["budget"]["selected_files"]
    )
    assert context_result["budget"]["omitted_ranges"] == (
        context_result["budget"]["candidate_ranges"] - context_result["budget"]["selected_ranges"]
    )
    assert context_result["budget"]["truncation_reason"]
    assert "omitted_candidates" in context_result
    explicit_reading_plan = context_result["reading_plan"]
    assert explicit_reading_plan, "explicit context reading_plan missing"
    assert explicit_reading_plan[0]["order"] == 1
    assert explicit_reading_plan[0]["file"] == context_result["files"][0]["file"]
    assert explicit_reading_plan[0]["next_action"]
    assert explicit_reading_plan[0]["question"]
    assert explicit_reading_plan[0]["suggested_tool"]["tool"]
    assert explicit_reading_plan[0]["suggested_tool"]["priority"] >= 1
    assert explicit_reading_plan[0]["suggested_tool"]["suggested_arguments"]
    assert explicit_reading_plan[0]["ranges"], "explicit context reading_plan ranges missing"
    assert explicit_reading_plan[0]["ranges"][0]["start_line"] >= 1
    explicit_suggested_result = call_suggested_tool(
        explicit_reading_plan[0]["suggested_tool"],
        12,
    )
    if explicit_reading_plan[0]["suggested_tool"]["tool"] == "file_outline":
        assert any(
            symbol["name"] == smoke_symbol
            for symbol in explicit_suggested_result
        ), "explicit suggested file_outline did not return seed symbol"
    if context_result["budget"]["omitted_files"] > 0:
        assert context_result["omitted_candidates"], "omitted context candidates missing"
        omitted_candidate = context_result["omitted_candidates"][0]
        assert omitted_candidate["file"]
        assert omitted_candidate["source"]
        assert omitted_candidate["score"] > 0
        assert omitted_candidate["reason"]
        assert omitted_candidate["ranges"], "omitted candidate ranges missing"
        assert "excerpt" not in omitted_candidate["ranges"][0]
        assert omitted_candidate["suggested_tool"]["tool"] == "context_pack"
        assert omitted_candidate["suggested_tool"]["priority"] >= 50
        assert omitted_candidate["suggested_tool"]["suggested_arguments"]["files"][0] == omitted_candidate["file"]
        omitted_follow_up_result = call_suggested_tool(
            omitted_candidate["suggested_tool"],
            16,
        )
        assert any(
            item["file"] == omitted_candidate["file"]
            for item in omitted_follow_up_result["files"]
        ), "omitted candidate follow-up did not select requested file"

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
    assert auto_context_result["reading_plan"], "auto context reading_plan missing"
    assert auto_context_result["reading_plan"][0]["order"] == 1
    assert auto_context_result["reading_plan"][0]["file"] == auto_context_result["files"][0]["file"]
    assert auto_context_result["reading_plan"][0]["focus"]
    assert auto_context_result["reading_plan"][0]["next_action"]
    assert auto_context_result["reading_plan"][0]["question"]
    assert auto_context_result["reading_plan"][0]["suggested_tool"]["tool"]
    assert auto_context_result["reading_plan"][0]["suggested_tool"]["priority"] >= 1
    assert auto_context_result["reading_plan"][0]["suggested_tool"]["suggested_arguments"]
    assert auto_context_result["reading_plan"][0]["ranges"], "auto context reading_plan ranges missing"
    assert auto_context_result["reading_plan"][0]["ranges"][0]["start_line"] >= 1
    auto_suggested_result = call_suggested_tool(
        auto_context_result["reading_plan"][0]["suggested_tool"],
        13,
    )
    if auto_context_result["reading_plan"][0]["suggested_tool"]["tool"] == "file_outline":
        assert any(
            symbol["name"] == "main"
            for symbol in auto_suggested_result
        ), "auto suggested file_outline did not return entrypoint symbol"

    print("MCP stdio smoke passed")
    print(f"root: {smoke_root}")
    print(f"symbol: {smoke_symbol}")
    print(f"tools: {len(tool_names)}")
    print(f"indexed_files: {index_result['indexed_files']}")
    print(f"overview_entrypoints: {len(overview_result['entrypoints'])}")
    print(f"overview_recommendations: {len(recommended_tools)}")
    print(f"overview_context_seed_strategy: {overview_context_result['seed_strategy']}")
    print(f"auto_seed_strategy: {auto_context_result['seed_strategy']}")
    print(f"auto_reading_plan_steps: {len(auto_context_result['reading_plan'])}")
    print(f"explicit_suggested_tool: {explicit_reading_plan[0]['suggested_tool']['tool']}")
    print(f"auto_suggested_tool: {auto_context_result['reading_plan'][0]['suggested_tool']['tool']}")
    print(f"explicit_omitted_candidates: {len(context_result['omitted_candidates'])}")
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
