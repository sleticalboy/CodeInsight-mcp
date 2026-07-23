#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
FIRST_CALL_ROOT="${CODEINSIGHT_FIRST_CALL_ROOT:-}"
FIRST_CALL_TASK="${CODEINSIGHT_FIRST_CALL_TASK:-inspect src/auth.ts before editing login behavior}"
FIRST_CALL_TOKEN_BUDGET="${CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET:-1600}"
FIRST_CALL_FILES="${CODEINSIGHT_FIRST_CALL_FILES:-}"
FIRST_CALL_SYMBOLS="${CODEINSIGHT_FIRST_CALL_SYMBOLS:-}"
SUMMARY_JSON=""
TEMP_DIR=""
DEFAULT_FIXTURE="0"

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
                                        Defaults to "inspect src/auth.ts before editing login behavior".
  CODEINSIGHT_FIRST_CALL_FILES          Newline-separated explicit seed files.
  CODEINSIGHT_FIRST_CALL_SYMBOLS        Newline-separated explicit seed symbols.
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
  DEFAULT_FIXTURE="1"
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
    FIRST_CALL_FILES="$FIRST_CALL_FILES" \
    FIRST_CALL_SYMBOLS="$FIRST_CALL_SYMBOLS" \
    SUMMARY_JSON="$SUMMARY_JSON" \
    DEFAULT_FIXTURE="$DEFAULT_FIXTURE" \
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
seed_files = [item for item in os.environ.get("FIRST_CALL_FILES", "").splitlines() if item]
seed_symbols = [item for item in os.environ.get("FIRST_CALL_SYMBOLS", "").splitlines() if item]
summary_json_path = os.environ.get("SUMMARY_JSON", "")
default_fixture = os.environ.get("DEFAULT_FIXTURE") == "1"


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

    route_arguments = {
        "root": root,
        "task": task,
        "token_budget": token_budget,
        "impact_limit": 10,
        "impact_depth": 2,
        "impact_evidence_limit": 3,
    }
    if seed_files:
        route_arguments["files"] = seed_files
    if seed_symbols:
        route_arguments["symbols"] = seed_symbols

    route = call_tool(
        3,
        "agent_route",
        route_arguments,
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
    selected_seeds = context_pack.get("selected_seeds", [])
    seed_strategy = context_pack.get("seed_strategy", "")
    first_seed = selected_seeds[0] if selected_seeds else {}
    if default_fixture and not seed_files and not seed_symbols:
        expect(
            seed_strategy == "auto_task_path",
            "agent_route_contract",
            f"default first-call seed_strategy should be auto_task_path: {seed_strategy!r}",
        )
        expect(
            first_seed.get("source") == "task_path",
            "agent_route_contract",
            f"default first-call first seed should come from task_path: {first_seed!r}",
        )
        expect(
            first_seed.get("value") == "src/auth.ts",
            "agent_route_contract",
            f"default first-call first seed should target src/auth.ts: {first_seed!r}",
        )
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
    routing_decision = route.get("routing_decision", {})
    expect(
        isinstance(routing_decision, dict),
        "agent_route_contract",
        "agent_route.routing_decision is missing",
    )
    expect(
        routing_decision.get("seed_strategy") == seed_strategy,
        "agent_route_contract",
        f"routing_decision.seed_strategy should mirror context_pack.seed_strategy: {routing_decision!r}",
    )
    expect(
        routing_decision.get("first_seed_source", "") == first_seed.get("source", ""),
        "agent_route_contract",
        "routing_decision.first_seed_source should mirror selected_seeds[0].source",
    )
    expect(
        routing_decision.get("first_seed_value", "") == first_seed.get("value", ""),
        "agent_route_contract",
        "routing_decision.first_seed_value should mirror selected_seeds[0].value",
    )
    expect(
        routing_decision.get("first_file") == first_reading_file,
        "agent_route_contract",
        "routing_decision.first_file should mirror reading_plan[0].file",
    )
    expect(
        routing_decision.get("first_selection_rank") == reading_plan[0]["selection_rank"],
        "agent_route_contract",
        "routing_decision.first_selection_rank should mirror reading_plan[0].selection_rank",
    )
    expect(
        routing_decision.get("first_suggested_tool", {}).get("tool") == first_reading_tool,
        "agent_route_contract",
        "routing_decision.first_suggested_tool should mirror reading_plan[0].suggested_tool",
    )
    expect(
        routing_decision.get("line_reduction") == read_less["line_reduction"]
        and routing_decision.get("read_less_ratio") == read_less["read_less_ratio"]
        and routing_decision.get("source_lines_avoided") == read_less["source_lines_avoided"],
        "agent_route_contract",
        "routing_decision read-less metrics should mirror context_pack.read_less",
    )
    route_quality = routing_decision.get("route_quality", {})
    expect(
        isinstance(route_quality, dict),
        "agent_route_contract",
        "routing_decision.route_quality is missing",
    )
    expect(
        isinstance(route_quality.get("level"), str)
        and len(route_quality["level"]) > 0
        and isinstance(route_quality.get("score"), int)
        and isinstance(route_quality.get("evidence_count"), int)
        and isinstance(route_quality.get("evidence_sources"), list)
        and isinstance(route_quality.get("warnings"), list)
        and isinstance(route_quality.get("recommended_action"), str)
        and len(route_quality["recommended_action"]) > 0,
        "agent_route_contract",
        f"routing_decision.route_quality should expose level, score, evidence, warnings, and action: {route_quality!r}",
    )
    if default_fixture and not seed_files and not seed_symbols:
        expect(
            route_quality["level"] == "high"
            and route_quality["score"] >= 80
            and route_quality["evidence_count"] >= 1
            and route_quality["recommended_action"]
            in ("read_selected_context", "read_selected_context_then_use_continuation_if_needed"),
            "agent_route_contract",
            f"default route_quality should be high-confidence first-read evidence: {route_quality!r}",
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
    suggested_tool_result_names = []
    if suggested_tool["tool"] == "file_outline":
        expect(
            isinstance(suggested_result, list),
            "suggested_tool",
            f"file_outline should return a list, got {type(suggested_result).__name__}",
        )
        suggested_tool_result_names = [
            symbol.get("name")
            for symbol in suggested_result
            if isinstance(symbol, dict) and symbol.get("name")
        ]
        expect(
            suggested_tool_result_names,
            "suggested_tool",
            "file_outline suggested tool returned no symbols",
        )
        if default_fixture:
            expect(
                ("main" in suggested_tool_result_names)
                or ("AuthService" in suggested_tool_result_names and "login" in suggested_tool_result_names),
                "suggested_tool",
                f"file_outline suggested tool did not return expected default fixture symbols; names={suggested_tool_result_names}",
            )

    expect(
        route.get("impact_status") == "complete",
        "agent_route_contract",
        f"impact_status should be complete, got {route.get('impact_status')!r}",
    )
    impact_analysis = route.get("impact_analysis", {})
    impact_counts = impact_analysis.get("impact_counts")
    expect(impact_counts is not None, "agent_route_contract", "impact_analysis.impact_counts is missing")
    impact_suggested_checks = impact_analysis.get("suggested_checks", [])
    expect(
        impact_suggested_checks,
        "agent_route_contract",
        "impact_analysis.suggested_checks is missing",
    )
    impact_execution = execution_plan[3]
    expect(
        impact_execution.get("suggested_checks") == impact_suggested_checks,
        "agent_route_contract",
        "execution_plan[3].suggested_checks should mirror impact_analysis.suggested_checks",
    )
    impact_suggested_tool = impact_execution.get("suggested_tool", {})
    expect(
        impact_suggested_tool.get("tool") == "impact_analysis",
        "agent_route_contract",
        f"execution_plan[3].suggested_tool should reopen impact_analysis: {impact_suggested_tool!r}",
    )
    impact_instruction = impact_execution.get("instruction", "")
    expect(
        "First suggested check:" in impact_instruction,
        "agent_route_contract",
        "execution_plan[3].instruction should name the first suggested check",
    )

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
    blocked_routing_decision = blocked_route.get("routing_decision", {})
    blocked_route_quality = blocked_routing_decision.get("route_quality", {})
    expect(
        blocked_routing_decision.get("seed_strategy") == "auto_no_seed"
        and blocked_routing_decision.get("selected_file_count") == 0
        and blocked_routing_decision.get("continuation_status") == "blocked_no_seed"
        and blocked_routing_decision.get("impact_status") == "skipped_no_seed",
        "agent_route_blocked_contract",
        f"blocked routing_decision should expose no-seed status: {blocked_routing_decision!r}",
    )
    expect(
        blocked_route_quality.get("level") == "blocked"
        and blocked_route_quality.get("score") == 0
        and blocked_route_quality.get("evidence_count") == 0
        and blocked_route_quality.get("recommended_action") == "provide_seed_file_or_symbol",
        "agent_route_blocked_contract",
        f"blocked routing_decision.route_quality should expose no-seed recovery action: {blocked_route_quality!r}",
    )

    unmatched_route = call_tool(
        6,
        "agent_route",
        {
            "root": root,
            "task": "understand unmatched explicit symbol",
            "symbols": ["ThisSymbolDoesNotExist"],
            "token_budget": token_budget,
            "impact_limit": 10,
            "impact_depth": 2,
            "impact_evidence_limit": 3,
        },
        "agent_route_blocked_contract",
    )
    unmatched_context_pack = unmatched_route.get("context_pack", {})
    unmatched_continuation = unmatched_context_pack.get("continuation_summary", {})
    unmatched_execution_plan = unmatched_route.get("execution_plan", [])
    unmatched_route_steps = unmatched_route.get("route", [])
    expect(
        [step.get("tool") for step in unmatched_route_steps] == expected_route_tools,
        "agent_route_blocked_contract",
        "unmatched explicit seed route should preserve the default route tool order",
    )
    expect(
        unmatched_route_steps[2].get("status") == "blocked_no_context",
        "agent_route_blocked_contract",
        f"unmatched explicit seed route step should be blocked_no_context: {unmatched_route_steps}",
    )
    expect(
        unmatched_route.get("impact_status") == "skipped_no_context",
        "agent_route_blocked_contract",
        f"unmatched explicit seed impact_status should be skipped_no_context: {unmatched_route.get('impact_status')!r}",
    )
    expect(
        unmatched_context_pack.get("files") == [],
        "agent_route_blocked_contract",
        "unmatched explicit seed context_pack.files should be empty",
    )
    expect(
        unmatched_context_pack.get("reading_plan") == [],
        "agent_route_blocked_contract",
        "unmatched explicit seed context_pack.reading_plan should be empty",
    )
    expect(
        unmatched_context_pack.get("budget", {}).get("truncation_reason") == "no_context_for_explicit_seed",
        "agent_route_blocked_contract",
        f"unmatched explicit seed truncation_reason should be no_context_for_explicit_seed: {unmatched_context_pack.get('budget')!r}",
    )
    expect(
        "current_reading_step" not in unmatched_route,
        "agent_route_blocked_contract",
        "unmatched explicit seed route should omit current_reading_step",
    )
    expect(
        unmatched_continuation.get("status") == "blocked_no_context",
        "agent_route_blocked_contract",
        f"unmatched explicit seed continuation status should be blocked_no_context: {unmatched_continuation}",
    )
    expect(
        unmatched_continuation.get("next_action") == "provide_matching_seed_file_or_symbol",
        "agent_route_blocked_contract",
        f"unmatched explicit seed continuation next_action should ask for a matching seed: {unmatched_continuation}",
    )
    expect(
        [step.get("action") for step in unmatched_execution_plan] == expected_execution_plan_actions,
        "agent_route_blocked_contract",
        f"unmatched explicit seed execution_plan actions should preserve client order: {unmatched_execution_plan}",
    )
    expected_unmatched_statuses = [
        "blocked_no_reading_plan",
        "blocked_no_current_reading_step",
        "manual_after_selected_context",
        "skipped_no_context",
    ]
    unmatched_execution_statuses = [step.get("status") for step in unmatched_execution_plan]
    expect(
        unmatched_execution_statuses == expected_unmatched_statuses,
        "agent_route_blocked_contract",
        f"unexpected unmatched explicit seed execution statuses: {unmatched_execution_statuses}",
    )
    unmatched_routing_decision = unmatched_route.get("routing_decision", {})
    unmatched_route_quality = unmatched_routing_decision.get("route_quality", {})
    expect(
        unmatched_routing_decision.get("selected_file_count") == 0
        and unmatched_routing_decision.get("continuation_status") == "blocked_no_context"
        and unmatched_routing_decision.get("impact_status") == "skipped_no_context",
        "agent_route_blocked_contract",
        f"unmatched routing_decision should expose no-context status: {unmatched_routing_decision!r}",
    )
    expect(
        unmatched_route_quality.get("level") == "blocked"
        and unmatched_route_quality.get("score") == 0
        and unmatched_route_quality.get("evidence_count") == 0
        and unmatched_route_quality.get("recommended_action") == "provide_matching_seed_file_or_symbol",
        "agent_route_blocked_contract",
        f"unmatched routing_decision.route_quality should expose no-context recovery action: {unmatched_route_quality!r}",
    )

    with tempfile.TemporaryDirectory(prefix="codeinsight-unindexed-first-call-") as scoped_root:
        os.makedirs(os.path.join(scoped_root, ".codeinsight"), exist_ok=True)
        os.makedirs(os.path.join(scoped_root, "src"), exist_ok=True)
        with open(os.path.join(scoped_root, ".codeinsight", "config.toml"), "w", encoding="utf-8") as config_file:
            config_file.write('[index]\ninclude = ["src/auth.ts"]\n')
        with open(os.path.join(scoped_root, "src", "main.ts"), "w", encoding="utf-8") as main_file:
            main_file.write('export function main() {\n  return "skip";\n}\n')
        with open(os.path.join(scoped_root, "src", "auth.ts"), "w", encoding="utf-8") as auth_file:
            auth_file.write('export function authOnly() {\n  return "keep";\n}\n')
        unindexed_route = call_tool(
            7,
            "agent_route",
            {
                "root": scoped_root,
                "task": "inspect src/main.ts before editing startup",
                "token_budget": token_budget,
                "impact_limit": 10,
                "impact_depth": 2,
                "impact_evidence_limit": 3,
            },
            "agent_route_blocked_contract",
        )

    unindexed_context_pack = unindexed_route.get("context_pack", {})
    unindexed_continuation = unindexed_context_pack.get("continuation_summary", {})
    unindexed_execution_plan = unindexed_route.get("execution_plan", [])
    unindexed_route_steps = unindexed_route.get("route", [])
    unindexed_selected_seeds = unindexed_context_pack.get("selected_seeds", [])
    unindexed_first_seed = unindexed_selected_seeds[0] if unindexed_selected_seeds else {}
    expect(
        [step.get("tool") for step in unindexed_route_steps] == expected_route_tools,
        "agent_route_blocked_contract",
        "unindexed task-path route should preserve the default route tool order",
    )
    expect(
        unindexed_route_steps[2].get("status") == "blocked_unindexed_task_path",
        "agent_route_blocked_contract",
        f"unindexed task-path route step should be blocked_unindexed_task_path: {unindexed_route_steps}",
    )
    expect(
        unindexed_route.get("impact_status") == "skipped_unindexed_task_path",
        "agent_route_blocked_contract",
        f"unindexed task-path impact_status should be skipped_unindexed_task_path: {unindexed_route.get('impact_status')!r}",
    )
    expect(
        unindexed_context_pack.get("seed_strategy") == "auto_task_path_unindexed",
        "agent_route_blocked_contract",
        f"unindexed task-path seed_strategy should be auto_task_path_unindexed: {unindexed_context_pack.get('seed_strategy')!r}",
    )
    expect(
        unindexed_first_seed.get("source") == "task_path_unindexed"
        and unindexed_first_seed.get("value") == "src/main.ts",
        "agent_route_blocked_contract",
        f"unindexed task-path first seed should name src/main.ts: {unindexed_first_seed!r}",
    )
    expect(
        unindexed_context_pack.get("files") == [],
        "agent_route_blocked_contract",
        "unindexed task-path context_pack.files should be empty",
    )
    expect(
        unindexed_context_pack.get("reading_plan") == [],
        "agent_route_blocked_contract",
        "unindexed task-path context_pack.reading_plan should be empty",
    )
    expect(
        unindexed_context_pack.get("budget", {}).get("truncation_reason") == "unindexed_task_path",
        "agent_route_blocked_contract",
        f"unindexed task-path truncation_reason should be unindexed_task_path: {unindexed_context_pack.get('budget')!r}",
    )
    expect(
        "current_reading_step" not in unindexed_route,
        "agent_route_blocked_contract",
        "unindexed task-path route should omit current_reading_step",
    )
    expect(
        unindexed_continuation.get("status") == "blocked_unindexed_task_path",
        "agent_route_blocked_contract",
        f"unindexed task-path continuation status should be blocked_unindexed_task_path: {unindexed_continuation}",
    )
    expect(
        unindexed_continuation.get("next_action") == "index_or_update_scope_for_task_path",
        "agent_route_blocked_contract",
        f"unindexed task-path continuation next_action should ask for scope update: {unindexed_continuation}",
    )
    unindexed_message = unindexed_continuation.get("message", "")
    expect(
        "src/main.ts" in unindexed_message and "Index scope is enabled" in unindexed_message,
        "agent_route_blocked_contract",
        f"unindexed task-path continuation message should mention path and index scope: {unindexed_message!r}",
    )
    expect(
        [step.get("action") for step in unindexed_execution_plan] == expected_execution_plan_actions,
        "agent_route_blocked_contract",
        f"unindexed task-path execution_plan actions should preserve client order: {unindexed_execution_plan}",
    )
    expected_unindexed_statuses = [
        "blocked_no_reading_plan",
        "blocked_no_current_reading_step",
        "manual_after_selected_context",
        "skipped_unindexed_task_path",
    ]
    unindexed_execution_statuses = [step.get("status") for step in unindexed_execution_plan]
    expect(
        unindexed_execution_statuses == expected_unindexed_statuses,
        "agent_route_blocked_contract",
        f"unexpected unindexed task-path execution statuses: {unindexed_execution_statuses}",
    )
    unindexed_routing_decision = unindexed_route.get("routing_decision", {})
    unindexed_route_quality = unindexed_routing_decision.get("route_quality", {})
    expect(
        unindexed_routing_decision.get("seed_strategy") == "auto_task_path_unindexed"
        and unindexed_routing_decision.get("first_seed_source") == "task_path_unindexed"
        and unindexed_routing_decision.get("first_seed_value") == "src/main.ts"
        and unindexed_routing_decision.get("selected_file_count") == 0
        and unindexed_routing_decision.get("continuation_status") == "blocked_unindexed_task_path"
        and unindexed_routing_decision.get("impact_status") == "skipped_unindexed_task_path",
        "agent_route_blocked_contract",
        f"unindexed routing_decision should expose task-path block status: {unindexed_routing_decision!r}",
    )
    expect(
        unindexed_route_quality.get("level") == "blocked"
        and unindexed_route_quality.get("score") == 0
        and unindexed_route_quality.get("evidence_count") == 0
        and unindexed_route_quality.get("recommended_action") == "index_or_update_scope_for_task_path",
        "agent_route_blocked_contract",
        f"unindexed routing_decision.route_quality should expose scope recovery action: {unindexed_route_quality!r}",
    )
    expect(
        "task path seed is not indexed" in unindexed_execution_plan[3].get("instruction", ""),
        "agent_route_blocked_contract",
        "unindexed task-path impact instruction should name the skipped reason",
    )

    summary = {
        "status": "pass",
        "server": server_name,
        "root": root,
        "task": task,
        "token_budget": token_budget,
        "route_tools": route_tools,
        "selected_files": [item["file"] for item in context_pack["files"]],
        "seed_strategy": seed_strategy,
        "selected_seeds": selected_seeds,
        "first_seed_source": first_seed.get("source", ""),
        "first_seed_value": first_seed.get("value", ""),
        "first_context_file": first_context_file,
        "first_reading_file": first_reading_file,
        "first_reading_selection_rank": reading_plan[0]["selection_rank"],
        "route_quality": route_quality,
        "route_quality_level": route_quality["level"],
        "route_quality_score": route_quality["score"],
        "route_quality_evidence_count": route_quality["evidence_count"],
        "route_quality_recommended_action": route_quality["recommended_action"],
        "routing_decision": {
            "seed_strategy": routing_decision["seed_strategy"],
            "route_quality": route_quality,
            "first_seed_source": routing_decision.get("first_seed_source", ""),
            "first_seed_value": routing_decision.get("first_seed_value", ""),
            "first_file": routing_decision.get("first_file", ""),
            "first_selection_rank": routing_decision.get("first_selection_rank"),
            "first_suggested_tool": routing_decision.get("first_suggested_tool", {}).get("tool", ""),
            "line_reduction": routing_decision["line_reduction"],
            "read_less_ratio": routing_decision["read_less_ratio"],
            "continuation_status": routing_decision["continuation_status"],
            "continuation_next_action": routing_decision["continuation_next_action"],
            "impact_status": routing_decision["impact_status"],
        },
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
        "suggested_tool_result_names": suggested_tool_result_names,
        "suggested_tool_executed": suggested_tool_executed,
        "impact_status": route["impact_status"],
        "impact_counts": impact_counts,
        "impact_execution_suggested_tool": impact_suggested_tool.get("tool", ""),
        "impact_execution_suggested_checks": len(impact_execution.get("suggested_checks", [])),
        "impact_suggested_checks": len(impact_suggested_checks),
        "impact_first_suggested_check": impact_suggested_checks[0],
        "impact_execution_instruction_has_first_check": True,
        "blocked_no_seed": {
            "route_step_status": blocked_route_steps[2]["status"],
            "seed_strategy": blocked_context_pack["seed_strategy"],
            "continuation_status": blocked_continuation["status"],
            "continuation_next_action": blocked_continuation["next_action"],
            "context_files": len(blocked_context_pack["files"]),
            "reading_plan_steps": len(blocked_context_pack["reading_plan"]),
            "has_current_reading_step": "current_reading_step" in blocked_route,
            "route_quality": blocked_route_quality,
            "impact_status": blocked_route["impact_status"],
            "execution_plan_actions": [step["action"] for step in blocked_execution_plan],
            "execution_plan_statuses": blocked_execution_statuses,
        },
        "blocked_no_context": {
            "route_step_status": unmatched_route_steps[2]["status"],
            "continuation_status": unmatched_continuation["status"],
            "continuation_next_action": unmatched_continuation["next_action"],
            "truncation_reason": unmatched_context_pack["budget"]["truncation_reason"],
            "context_files": len(unmatched_context_pack["files"]),
            "reading_plan_steps": len(unmatched_context_pack["reading_plan"]),
            "has_current_reading_step": "current_reading_step" in unmatched_route,
            "route_quality": unmatched_route_quality,
            "impact_status": unmatched_route["impact_status"],
            "execution_plan_actions": [step["action"] for step in unmatched_execution_plan],
            "execution_plan_statuses": unmatched_execution_statuses,
        },
        "blocked_unindexed_task_path": {
            "route_step_status": unindexed_route_steps[2]["status"],
            "seed_strategy": unindexed_context_pack["seed_strategy"],
            "first_seed_source": unindexed_first_seed.get("source", ""),
            "first_seed_value": unindexed_first_seed.get("value", ""),
            "continuation_status": unindexed_continuation["status"],
            "continuation_next_action": unindexed_continuation["next_action"],
            "truncation_reason": unindexed_context_pack["budget"]["truncation_reason"],
            "context_files": len(unindexed_context_pack["files"]),
            "reading_plan_steps": len(unindexed_context_pack["reading_plan"]),
            "has_current_reading_step": "current_reading_step" in unindexed_route,
            "route_quality": unindexed_route_quality,
            "impact_status": unindexed_route["impact_status"],
            "execution_plan_actions": [step["action"] for step in unindexed_execution_plan],
            "execution_plan_statuses": unindexed_execution_statuses,
            "continuation_message_has_scope_hint": "Index scope is enabled" in unindexed_message,
            "impact_instruction_has_skipped_reason": "task path seed is not indexed"
            in unindexed_execution_plan[3].get("instruction", ""),
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
