#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
FIRST_CALL_ROOT="${CODEINSIGHT_FIRST_CALL_ROOT:-}"
FIRST_CALL_TASK="${CODEINSIGHT_FIRST_CALL_TASK:-understand app entrypoint flow}"
FIRST_CALL_TOKEN_BUDGET="${CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET:-1600}"
SUMMARY_JSON=""
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
usage: scripts/mcp-first-call-smoke.sh [--summary-json PATH] [--help]

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

Options:
  --summary-json PATH  Also write the JSON summary to PATH.
  -h, --help           Show this help.
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --summary-json)
        if [ "$#" -lt 2 ]; then
          fail_with usage "--summary-json requires a path"
        fi
        SUMMARY_JSON="$2"
        shift 2
        ;;
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
    SUMMARY_JSON="$SUMMARY_JSON" \
    python3 <<'PY'
import json
import os
import subprocess
import sys
import tempfile

codeinsight_bin = os.environ["CODEINSIGHT_BIN"]
root = os.environ["FIRST_CALL_ROOT"]
task = os.environ["FIRST_CALL_TASK"]
token_budget = int(os.environ["FIRST_CALL_TOKEN_BUDGET"])
summary_json_path = os.environ.get("SUMMARY_JSON", "")


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
    expect(reading_plan[0].get("file"), "agent_route_contract", "reading_plan[0].file is missing")
    expect(
        isinstance(reading_plan[0].get("selection_rank"), int)
        and reading_plan[0]["selection_rank"] > 0,
        "agent_route_contract",
        "reading_plan[0].selection_rank is missing",
    )
    expect(reading_plan[0].get("next_action"), "agent_route_contract", "reading_plan[0].next_action is missing")
    expect(reading_plan[0].get("focus"), "agent_route_contract", "reading_plan[0].focus is missing")
    expect(reading_plan[0].get("question"), "agent_route_contract", "reading_plan[0].question is missing")
    expect(reading_plan[0].get("reason"), "agent_route_contract", "reading_plan[0].reason is missing")
    expect(
        reading_plan[0].get("selection_reason"),
        "agent_route_contract",
        "reading_plan[0].selection_reason is missing",
    )
    expect(
        reading_plan[0].get("suggested_tool", {}).get("tool"),
        "agent_route_contract",
        "reading_plan[0].suggested_tool.tool is missing",
    )
    first_context_file = context_pack["files"][0]["file"]
    first_reading_file = reading_plan[0]["file"]
    expect(
        first_reading_file == first_context_file,
        "agent_route_contract",
        f"reading_plan[0].file should match context_pack.files[0].file: {first_reading_file!r} != {first_context_file!r}",
    )
    first_reason = reading_plan[0]["reason"]
    first_focus = reading_plan[0]["focus"]
    first_question = reading_plan[0]["question"]
    first_reading_tool = reading_plan[0]["suggested_tool"]["tool"]
    current_reading_step = route.get("current_reading_step", {})
    current_reading_step_matches_reading_plan = current_reading_step == reading_plan[0]
    expect(
        current_reading_step_matches_reading_plan,
        "agent_route_contract",
        "agent_route.current_reading_step should mirror context_pack.reading_plan[0]",
    )
    expect(
        first_question in first_reason,
        "agent_route_contract",
        "reading_plan[0].reason should include reading_plan[0].question",
    )
    expect(
        "If deeper evidence is needed, call " in first_reason
        and first_reading_tool in first_reason,
        "agent_route_contract",
        "reading_plan[0].reason should name the suggested tool",
    )
    expect(
        "Selection reason:" in first_reason,
        "agent_route_contract",
        "reading_plan[0].reason should include selection provenance",
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
    reading_files = [step["file"] for step in reading_plan]
    expect(
        first_execution.get("files") == reading_files,
        "agent_route_contract",
        f"execution_plan[0].files should match reading_plan file order: {first_execution.get('files')!r} != {reading_files!r}",
    )
    expect(
        f"candidate rank {reading_plan[0]['selection_rank']}" in first_execution.get("instruction", ""),
        "agent_route_contract",
        "execution_plan[0].instruction should expose reading_plan[0].selection_rank",
    )
    first_execution_instruction = first_execution.get("instruction", "")
    first_execution_instruction_has_focus = first_focus in first_execution_instruction
    expect(
        first_execution_instruction_has_focus,
        "agent_route_contract",
        "execution_plan[0].instruction should include reading_plan[0].focus",
    )
    first_execution_instruction_has_question = first_question in first_execution_instruction
    expect(
        first_execution_instruction_has_question,
        "agent_route_contract",
        "execution_plan[0].instruction should include reading_plan[0].question",
    )
    read_less = context_pack.get("read_less", {})
    expect(isinstance(read_less, dict), "agent_route_contract", "context_pack.read_less is missing")
    expect(
        isinstance(read_less.get("baseline_source_lines"), int)
        and read_less["baseline_source_lines"] >= 0,
        "agent_route_contract",
        "context_pack.read_less.baseline_source_lines is missing",
    )
    expect(
        isinstance(read_less.get("selected_source_lines"), int)
        and read_less["selected_source_lines"] >= 0,
        "agent_route_contract",
        "context_pack.read_less.selected_source_lines is missing",
    )
    expect(
        isinstance(read_less.get("source_lines_avoided"), int)
        and read_less["source_lines_avoided"] >= 0,
        "agent_route_contract",
        "context_pack.read_less.source_lines_avoided is missing",
    )
    expect(
        isinstance(read_less.get("line_reduction"), str)
        and len(read_less["line_reduction"]) > 0,
        "agent_route_contract",
        "context_pack.read_less.line_reduction is missing",
    )
    expect(
        isinstance(read_less.get("read_less_ratio"), str)
        and len(read_less["read_less_ratio"]) > 0,
        "agent_route_contract",
        "context_pack.read_less.read_less_ratio is missing",
    )
    first_execution_instruction_has_read_less = (
        "Read-less evidence: selected" in first_execution_instruction
        and f"selected {read_less['selected_source_lines']} of {read_less['baseline_source_lines']} source lines"
        in first_execution_instruction
        and f"avoided {read_less['source_lines_avoided']}" in first_execution_instruction
        and read_less["read_less_ratio"] in first_execution_instruction
    )
    expect(
        first_execution_instruction_has_read_less,
        "agent_route_contract",
        "execution_plan[0].instruction should include context_pack.read_less evidence",
    )

    suggested_tool = execution_plan[1].get("suggested_tool", {})
    expect(suggested_tool.get("tool"), "suggested_tool", "execution_plan suggested_tool.tool is missing")
    expect(
        suggested_tool.get("suggested_arguments"),
        "suggested_tool",
        "execution_plan suggested_tool.suggested_arguments is missing",
    )
    expect(
        execution_plan[1].get("files") == [first_reading_file],
        "suggested_tool",
        f"execution_plan[1].files should point to reading_plan[0]: {execution_plan[1].get('files')!r}",
    )
    expect(
        suggested_tool == reading_plan[0]["suggested_tool"],
        "suggested_tool",
        "execution_plan[1].suggested_tool should match reading_plan[0].suggested_tool",
    )
    current_step_instruction = execution_plan[1].get("instruction", "")
    current_step_instruction_has_focus = first_focus in current_step_instruction
    expect(
        current_step_instruction_has_focus,
        "suggested_tool",
        "execution_plan[1].instruction should include reading_plan[0].focus",
    )
    current_step_instruction_has_question = first_question in current_step_instruction
    expect(
        current_step_instruction_has_question,
        "suggested_tool",
        "execution_plan[1].instruction should include reading_plan[0].question",
    )
    current_step_instruction_has_action = reading_plan[0]["next_action"] in current_step_instruction
    expect(
        current_step_instruction_has_action,
        "suggested_tool",
        "execution_plan[1].instruction should include reading_plan[0].next_action",
    )
    continuation_summary = context_pack.get("continuation_summary", {})
    expect(
        continuation_summary.get("next_action"),
        "agent_route_contract",
        "continuation_summary.next_action is missing",
    )
    expect(
        continuation_summary["next_action"] in execution_plan[2].get("instruction", ""),
        "agent_route_contract",
        "execution_plan[2].instruction should name continuation_summary.next_action",
    )
    if continuation_summary.get("suggested_tool") is not None:
        expect(
            execution_plan[2].get("suggested_tool") == continuation_summary["suggested_tool"],
            "agent_route_contract",
            "execution_plan[2].suggested_tool should match continuation_summary.suggested_tool",
        )
    omitted_candidates = context_pack.get("omitted_candidates", [])
    first_omitted = omitted_candidates[0] if omitted_candidates else {}
    if first_omitted:
        expect(
            isinstance(first_omitted.get("selection_rank"), int)
            and first_omitted["selection_rank"] > 0,
            "agent_route_contract",
            "omitted_candidates[0].selection_rank is missing",
        )
        expect(
            first_omitted.get("omission_reason"),
            "agent_route_contract",
            "omitted_candidates[0].omission_reason is missing",
        )
        continuation_instruction = execution_plan[2].get("instruction", "")
        expect(
            first_omitted["file"] in continuation_instruction
            and f"candidate rank {first_omitted['selection_rank']}" in continuation_instruction
            and first_omitted["omission_reason"] in continuation_instruction,
            "agent_route_contract",
            "execution_plan[2].instruction should expose omitted candidate evidence",
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

    with tempfile.TemporaryDirectory(prefix="codeinsight-empty-first-call-") as empty_root:
        blocked_route = call_tool(
            5,
            "agent_route",
            {
                "root": empty_root,
                "task": "understand this empty repository",
                "token_budget": token_budget,
                "impact_limit": 10,
                "impact_depth": 2,
                "impact_evidence_limit": 3,
            },
            "agent_route_blocked_contract",
        )

    blocked_context_pack = blocked_route.get("context_pack", {})
    blocked_continuation = blocked_context_pack.get("continuation_summary", {})
    blocked_execution_plan = blocked_route.get("execution_plan", [])
    blocked_route_steps = blocked_route.get("route", [])
    expect(
        [step.get("tool") for step in blocked_route_steps] == expected_route_tools,
        "agent_route_blocked_contract",
        "blocked route should preserve the default route tool order",
    )
    expect(
        blocked_route_steps[2].get("status") == "blocked_no_seed",
        "agent_route_blocked_contract",
        f"blocked context route step should be blocked_no_seed: {blocked_route_steps}",
    )
    expect(
        blocked_route.get("impact_status") == "skipped_no_seed",
        "agent_route_blocked_contract",
        f"blocked impact_status should be skipped_no_seed: {blocked_route.get('impact_status')!r}",
    )
    expect(
        blocked_context_pack.get("seed_strategy") == "auto_no_seed",
        "agent_route_blocked_contract",
        f"blocked seed_strategy should be auto_no_seed: {blocked_context_pack.get('seed_strategy')!r}",
    )
    expect(
        blocked_context_pack.get("files") == [],
        "agent_route_blocked_contract",
        "blocked context_pack.files should be empty",
    )
    expect(
        blocked_context_pack.get("reading_plan") == [],
        "agent_route_blocked_contract",
        "blocked context_pack.reading_plan should be empty",
    )
    expect(
        "current_reading_step" not in blocked_route,
        "agent_route_blocked_contract",
        "blocked agent_route should omit current_reading_step",
    )
    expect(
        blocked_continuation.get("status") == "blocked_no_seed",
        "agent_route_blocked_contract",
        f"blocked continuation status should be blocked_no_seed: {blocked_continuation}",
    )
    expect(
        blocked_continuation.get("next_action") == "provide_seed_file_or_symbol",
        "agent_route_blocked_contract",
        f"blocked continuation next_action should ask for a seed: {blocked_continuation}",
    )
    expect(
        [step.get("action") for step in blocked_execution_plan] == expected_execution_plan_actions,
        "agent_route_blocked_contract",
        f"blocked execution_plan actions should preserve client order: {blocked_execution_plan}",
    )
    expected_blocked_statuses = [
        "blocked_no_reading_plan",
        "blocked_no_current_reading_step",
        "manual_after_selected_context",
        "skipped_no_seed",
    ]
    blocked_execution_statuses = [step.get("status") for step in blocked_execution_plan]
    expect(
        blocked_execution_statuses == expected_blocked_statuses,
        "agent_route_blocked_contract",
        f"unexpected blocked execution statuses: {blocked_execution_statuses}",
    )

    summary = {
        "status": "pass",
        "server": server_name,
        "root": root,
        "task": task,
        "token_budget": token_budget,
        "route_tools": route_tools,
        "selected_files": [item["file"] for item in context_pack["files"]],
        "first_context_file": first_context_file,
        "first_reading_file": first_reading_file,
        "first_reading_selection_rank": reading_plan[0]["selection_rank"],
        "current_reading_step_matches_reading_plan": current_reading_step_matches_reading_plan,
        "context_pack_read_less": read_less,
        "baseline_source_lines": read_less["baseline_source_lines"],
        "selected_source_lines": read_less["selected_source_lines"],
        "source_lines_avoided": read_less["source_lines_avoided"],
        "line_reduction": read_less["line_reduction"],
        "read_less_ratio": read_less["read_less_ratio"],
        "reading_plan": [
            {
                "file": step["file"],
                "selection_rank": step["selection_rank"],
                "next_action": step["next_action"],
                "focus": step["focus"],
                "question": step["question"],
                "reason": step["reason"],
                "selection_reason": step["selection_reason"],
                "suggested_tool": step["suggested_tool"]["tool"],
            }
            for step in reading_plan
        ],
        "execution_plan_actions": execution_plan_actions,
        "execution_plan_reads_in_reading_plan_order": True,
        "first_execution_action": first_execution["action"],
        "first_execution_instruction_has_focus": first_execution_instruction_has_focus,
        "first_execution_instruction_has_question": first_execution_instruction_has_question,
        "first_execution_instruction_has_read_less": first_execution_instruction_has_read_less,
        "current_step_suggested_tool_matches_reading_plan": True,
        "current_step_instruction_has_focus": current_step_instruction_has_focus,
        "current_step_instruction_has_question": current_step_instruction_has_question,
        "current_step_instruction_has_action": current_step_instruction_has_action,
        "continuation_after_selected_context": True,
        "continuation_status": continuation_summary.get("status", ""),
        "continuation_next_action": continuation_summary.get("next_action", ""),
        "first_omitted_file": first_omitted.get("file", ""),
        "first_omitted_selection_rank": first_omitted.get("selection_rank"),
        "first_omitted_omission_reason": first_omitted.get("omission_reason", ""),
        "first_omitted_next_action": first_omitted.get("next_action", ""),
        "suggested_tool": {
            "tool": suggested_tool["tool"],
            "arguments": suggested_tool["suggested_arguments"],
        },
        "suggested_tool_executed": suggested_tool_executed,
        "impact_status": route["impact_status"],
        "impact_counts": impact_counts,
        "blocked_no_seed": {
            "route_step_status": blocked_route_steps[2]["status"],
            "seed_strategy": blocked_context_pack["seed_strategy"],
            "continuation_status": blocked_continuation["status"],
            "continuation_next_action": blocked_continuation["next_action"],
            "context_files": len(blocked_context_pack["files"]),
            "reading_plan_steps": len(blocked_context_pack["reading_plan"]),
            "has_current_reading_step": "current_reading_step" in blocked_route,
            "impact_status": blocked_route["impact_status"],
            "execution_plan_actions": [step["action"] for step in blocked_execution_plan],
            "execution_plan_statuses": blocked_execution_statuses,
        },
    }
    summary_json = json.dumps(summary, indent=2, sort_keys=True)
    print(summary_json)
    if summary_json_path:
        parent = os.path.dirname(summary_json_path)
        if parent:
            os.makedirs(parent, exist_ok=True)
        with open(summary_json_path, "w", encoding="utf-8") as summary_file:
            summary_file.write(summary_json)
            summary_file.write("\n")
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
