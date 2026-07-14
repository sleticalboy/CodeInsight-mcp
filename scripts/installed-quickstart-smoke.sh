#!/usr/bin/env bash
set -euo pipefail

CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-$(command -v codeinsight || true)}"
TEMP_DIR=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

main() {
  if [ -z "$CODEINSIGHT_BIN" ] || [ ! -x "$CODEINSIGHT_BIN" ]; then
    echo "CODEINSIGHT_BIN is not executable; install codeinsight or set CODEINSIGHT_BIN=/path/to/codeinsight" >&2
    exit 1
  fi
  if ! command -v python3 >/dev/null 2>&1; then
    echo "missing required command: python3" >&2
    exit 1
  fi

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  smoke_root="$TEMP_DIR/sample-app"
  mkdir -p "$smoke_root/src" "$smoke_root/tests"

  cat >"$smoke_root/package.json" <<'EOF'
{"name":"sample-app","version":"0.1.0","type":"module","scripts":{"start":"node src/index.js"}}
EOF
  cat >"$smoke_root/src/index.js" <<'EOF'
import { createGreeting } from './service.js';

export function main(name = 'world') {
  return createGreeting(name);
}

if (process.argv[1] && process.argv[1].endsWith('index.js')) {
  console.log(main(process.argv[2]));
}
EOF
  cat >"$smoke_root/src/service.js" <<'EOF'
import { titleCase } from './util.js';

export function createGreeting(name) {
  return `Hello, ${titleCase(name)}!`;
}
EOF
  cat >"$smoke_root/src/util.js" <<'EOF'
export function titleCase(value) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}
EOF
  cat >"$smoke_root/tests/service.test.js" <<'EOF'
import { createGreeting } from '../src/service.js';

console.log(createGreeting('codex'));
EOF

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" SMOKE_ROOT="$smoke_root" python3 <<'PY'
import json
import os
import subprocess

codeinsight_bin = os.environ["CODEINSIGHT_BIN"]
smoke_root = os.environ["SMOKE_ROOT"]


def run_json(args):
    output = subprocess.check_output([codeinsight_bin, *args], text=True)
    return json.loads(output)


version = run_json(["version"])
if version["name"] != "codeinsight":
    raise AssertionError(version)

indexed = run_json(["index", smoke_root, "--force"])
if indexed["indexed_files"] < 4 or indexed["symbols"] < 3:
    raise AssertionError(indexed)

overview = run_json(["overview", smoke_root])
if len(overview["entrypoints"]) < 1 or len(overview["recommended_next_tools"]) < 1:
    raise AssertionError(overview)

context = run_json([
    "context-pack",
    smoke_root,
    "--task",
    "understand the main application entrypoint",
    "--token-budget",
    "1200",
])
if len(context["files"]) < 1 or len(context["reading_plan"]) < 1:
    raise AssertionError(context)
if context["budget"]["applied_token_budget"] != 1200:
    raise AssertionError(context["budget"])

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
    if initialize["result"]["serverInfo"]["name"] != "codeinsight":
        raise AssertionError(initialize)

    tools = request({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
    tool_names = {tool["name"] for tool in tools["result"]["tools"]}
    for expected in ("index_project", "project_overview", "context_pack", "impact_analysis", "version"):
        if expected not in tool_names:
            raise AssertionError(expected)

    mcp_index = request(
        {
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "index_project",
                "arguments": {"root": smoke_root, "force": True},
            },
        }
    )["result"]["structuredContent"]
    if mcp_index["indexed_files"] < 4:
        raise AssertionError(mcp_index)

    mcp_overview = request(
        {
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "project_overview",
                "arguments": {"root": smoke_root},
            },
        }
    )["result"]["structuredContent"]
    if not mcp_overview["entrypoints"]:
        raise AssertionError(mcp_overview)

    mcp_context = request(
        {
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "context_pack",
                "arguments": {
                    "root": smoke_root,
                    "task": "understand the main application entrypoint",
                    "token_budget": 1200,
                },
            },
        }
    )["result"]["structuredContent"]
    if not mcp_context["files"] or not mcp_context["reading_plan"]:
        raise AssertionError(mcp_context)
finally:
    proc.terminate()
    try:
        proc.wait(timeout=2)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=2)

print(json.dumps({
    "binary": codeinsight_bin,
    "version": version["version"],
    "indexed_files": indexed["indexed_files"],
    "symbols": indexed["symbols"],
    "overview_entrypoints": len(overview["entrypoints"]),
    "overview_recommendations": len(overview["recommended_next_tools"]),
    "context_files": len(context["files"]),
    "context_reading_plan": len(context["reading_plan"]),
    "mcp_tools": len(tool_names),
}, indent=2))
PY

  echo "installed quickstart smoke passed"
}

main "$@"
