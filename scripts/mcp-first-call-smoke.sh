#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
FIRST_CALL_ROOT="${CODEINSIGHT_FIRST_CALL_ROOT:-}"
FIRST_CALL_TASK="${CODEINSIGHT_FIRST_CALL_TASK:-understand app entrypoint flow}"
FIRST_CALL_TOKEN_BUDGET="${CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET:-1600}"
TEMP_DIR=""

fail_with() {
  local category="$1"
  shift
  echo "mcp first-call smoke failed [$category]: $*" >&2
  exit 1
}

fail() {
  fail_with binary "$@"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

usage() {
  cat <<'EOF'
usage: scripts/mcp-first-call-smoke.sh [--help]

Runs a compact MCP stdio first-call check and prints a JSON summary.

Environment:
  CODEINSIGHT_BIN                       Existing codeinsight binary to test.
                                        Defaults to a local release build.
  CODEINSIGHT_FIRST_CALL_ROOT           Repository to analyze.
                                        Defaults to a temporary TypeScript fixture.
  CODEINSIGHT_FIRST_CALL_TASK           Task passed to agent_route.
                                        Defaults to "understand app entrypoint flow".
  CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET   Token budget passed to agent_route.
                                        Defaults to 1600.

Output:
  stdout  JSON summary when the first MCP agent_route call succeeds.
  stderr  Categorized failures such as [binary], [mcp_server],
          [agent_route_contract], [suggested_tool], or [unexpected].
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fail_with usage "unknown argument: $1"
        ;;
    esac
  done
}

build_binary_if_needed() {
  if [ -z "$CODEINSIGHT_BIN" ]; then
    require_command cargo
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml" >/dev/null
    CODEINSIGHT_BIN="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')/release/codeinsight"
  fi

  if [ ! -x "$CODEINSIGHT_BIN" ]; then
    fail "CODEINSIGHT_BIN is not executable: $CODEINSIGHT_BIN"
  fi
}

create_fixture() {
  TEMP_DIR="$(mktemp -d)"
  FIRST_CALL_ROOT="$TEMP_DIR/repo"
  mkdir -p "$FIRST_CALL_ROOT/src"

  cat >"$FIRST_CALL_ROOT/package.json" <<'EOF'
{
  "type": "module",
  "scripts": {
    "start": "tsx src/main.ts"
  }
}
EOF

  cat >"$FIRST_CALL_ROOT/src/main.ts" <<'EOF'
import { AuthService } from "./auth";

export function main() {
  const service = new AuthService();
  return service.login("demo-user");
}

main();
EOF

  cat >"$FIRST_CALL_ROOT/src/auth.ts" <<'EOF'
import { auditLogin } from "./audit";

export class AuthService {
  login(user: string) {
    return auditLogin(user);
  }
}
EOF

  cat >"$FIRST_CALL_ROOT/src/audit.ts" <<'EOF'
export function auditLogin(user: string) {
  return { user, status: "accepted" };
}
EOF
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

main() {
  parse_args "$@"
  require_command python3
  build_binary_if_needed

  if [ -z "$FIRST_CALL_ROOT" ]; then
    create_fixture
  fi
  FIRST_CALL_ROOT="$(cd "$FIRST_CALL_ROOT" && pwd)"

  trap cleanup EXIT INT TERM

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" \
    FIRST_CALL_ROOT="$FIRST_CALL_ROOT" \
    FIRST_CALL_TASK="$FIRST_CALL_TASK" \
    FIRST_CALL_TOKEN_BUDGET="$FIRST_CALL_TOKEN_BUDGET" \
    python3 <<'PY'
import json
import os
import subprocess
import sys

codeinsight_bin = os.environ["CODEINSIGHT_BIN"]
root = os.environ["FIRST_CALL_ROOT"]
task = os.environ["FIRST_CALL_TASK"]
token_budget = int(os.environ["FIRST_CALL_TOKEN_BUDGET"])


class SmokeFailure(Exception):
    def __init__(self, category, message):
        super().__init__(message)
        self.category = category
        self.message = message


def fail(category, message):
    raise SmokeFailure(category, message)


def expect(condition, category, message):
    if not condition:
        fail(category, message)


def request(payload, category):
    assert proc.stdin is not None
    assert proc.stdout is not None
    try:
        proc.stdin.write(json.dumps(payload, separators=(",", ":")) + "\n")
        proc.stdin.flush()
    except BrokenPipeError as exc:
        stderr = proc.stderr.read() if proc.stderr is not None else ""
        fail(category, f"server closed stdin before response: {stderr or exc}")
    line = proc.stdout.readline()
    if not line:
        stderr = proc.stderr.read() if proc.stderr is not None else ""
        fail(category, f"server exited before response: {stderr or 'empty stderr'}")
    try:
        response = json.loads(line)
    except json.JSONDecodeError as exc:
        fail(category, f"server returned invalid JSON: {exc}: {line.strip()}")
    if "error" in response:
        fail(category, f"JSON-RPC error: {json.dumps(response['error'], sort_keys=True)}")
    return response


def call_tool(request_id, name, arguments, category):
    response = request(
        {
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments,
            },
        },
        category,
    )
    structured = response.get("result", {}).get("structuredContent")
    if structured is None:
        fail(category, f"{name} returned no structuredContent")
    return structured


proc = None
try:
    proc = subprocess.Popen(
        [codeinsight_bin, "serve", "--transport", "stdio"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    initialize = request(
        {"jsonrpc": "2.0", "id": 1, "method": "initialize"},
        "mcp_server",
    )
    server_name = initialize.get("result", {}).get("serverInfo", {}).get("name")
    expect(
        server_name == "codeinsight",
        "mcp_server",
        f"initialize returned unexpected server name: {server_name!r}",
    )

    tools = request(
        {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
        "mcp_server",
    )
    tool_names = {tool.get("name") for tool in tools.get("result", {}).get("tools", [])}
    for expected in ("agent_route", "context_pack", "impact_analysis", "version"):
        expect(expected in tool_names, "mcp_server", f"tools/list is missing {expected}")

    route = call_tool(
        3,
        "agent_route",
        {
            "root": root,
            "task": task,
            "token_budget": token_budget,
            "impact_limit": 10,
            "impact_depth": 2,
            "impact_evidence_limit": 3,
        },
        "agent_route_contract",
    )

    route_steps = route.get("route", [])
    route_tools = [step.get("tool") for step in route_steps]
    expected_route_tools = [
        "index_project",
        "project_overview",
        "context_pack",
        "impact_analysis",
    ]
    expect(
        route_tools == expected_route_tools,
        "agent_route_contract",
        f"unexpected route_tools: expected {expected_route_tools}, got {route_tools}",
    )

    execution_plan = route.get("execution_plan", [])
    execution_plan_actions = [step.get("action") for step in execution_plan]
    expected_execution_plan_actions = [
        "read_selected_context",
        "use_current_reading_step_suggested_tool",
        "use_continuation_if_needed",
        "review_impact_before_edits",
    ]
    expect(
        execution_plan_actions == expected_execution_plan_actions,
        "agent_route_contract",
        "unexpected execution_plan_actions: "
        f"expected {expected_execution_plan_actions}, got {execution_plan_actions}",
    )

    context_pack = route.get("context_pack", {})
    reading_plan = context_pack.get("reading_plan", [])
    expect(context_pack.get("files"), "agent_route_contract", "agent_route selected no context files")
    expect(reading_plan, "agent_route_contract", "agent_route returned no reading plan")
    expect(reading_plan[0].get("reason"), "agent_route_contract", "reading_plan[0].reason is missing")
    expect(
        reading_plan[0].get("selection_reason"),
        "agent_route_contract",
        "reading_plan[0].selection_reason is missing",
    )

    first_execution = execution_plan[0]
    expect(
        first_execution.get("action") == "read_selected_context",
        "agent_route_contract",
        "execution_plan[0].action should be read_selected_context",
    )
    expect(
        first_execution.get("status") == "ready",
        "agent_route_contract",
        f"execution_plan[0].status should be ready, got {first_execution.get('status')!r}",
    )
    expect(first_execution.get("files"), "agent_route_contract", "execution_plan[0].files is missing")

    suggested_tool = execution_plan[1].get("suggested_tool", {})
    expect(suggested_tool.get("tool"), "suggested_tool", "execution_plan suggested_tool.tool is missing")
    expect(
        suggested_tool.get("suggested_arguments"),
        "suggested_tool",
        "execution_plan suggested_tool.suggested_arguments is missing",
    )

    suggested_result = call_tool(
        4,
        suggested_tool["tool"],
        suggested_tool["suggested_arguments"],
        "suggested_tool",
    )
    suggested_tool_executed = True
    if suggested_tool["tool"] == "file_outline":
        names = [symbol.get("name") for symbol in suggested_result]
        expect(
            "main" in names,
            "suggested_tool",
            f"file_outline suggested tool did not return main; names={names}",
        )

    expect(
        route.get("impact_status") == "complete",
        "agent_route_contract",
        f"impact_status should be complete, got {route.get('impact_status')!r}",
    )
    impact_counts = route.get("impact_analysis", {}).get("impact_counts")
    expect(impact_counts is not None, "agent_route_contract", "impact_analysis.impact_counts is missing")

    summary = {
        "status": "pass",
        "server": server_name,
        "root": root,
        "task": task,
        "token_budget": token_budget,
        "route_tools": route_tools,
        "selected_files": [item["file"] for item in context_pack["files"]],
        "reading_plan": [
            {
                "file": step["file"],
                "reason": step["reason"],
                "selection_reason": step["selection_reason"],
                "suggested_tool": step["suggested_tool"]["tool"],
            }
            for step in reading_plan
        ],
        "execution_plan_actions": execution_plan_actions,
        "first_execution_action": first_execution["action"],
        "suggested_tool": {
            "tool": suggested_tool["tool"],
            "arguments": suggested_tool["suggested_arguments"],
        },
        "suggested_tool_executed": suggested_tool_executed,
        "impact_status": route["impact_status"],
        "impact_counts": impact_counts,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
except SmokeFailure as exc:
    print(f"mcp first-call smoke failed [{exc.category}]: {exc.message}", file=sys.stderr)
    sys.exit(1)
except Exception as exc:
    print(f"mcp first-call smoke failed [unexpected]: {type(exc).__name__}: {exc}", file=sys.stderr)
    sys.exit(1)
finally:
    if proc is not None:
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            proc.kill()
            proc.wait(timeout=5)
PY
}

main "$@"
