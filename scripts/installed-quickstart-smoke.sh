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


def assert_actionable_reading_plan(payload, label):
    if not payload.get("files") or not payload.get("reading_plan"):
        raise AssertionError({label: payload})

    first_file = payload["files"][0]["file"]
    first_step = payload["reading_plan"][0]
    if first_step.get("order") != 1:
        raise AssertionError({label: first_step})
    if first_step.get("file") != first_file:
        raise AssertionError({label: first_step, "first_file": first_file})
    if not isinstance(first_step.get("selection_rank"), int) or first_step["selection_rank"] < 1:
        raise AssertionError({label: first_step})
    if not first_step.get("next_action"):
        raise AssertionError({label: first_step})
    if not first_step.get("focus"):
        raise AssertionError({label: first_step})
    if not first_step.get("question"):
        raise AssertionError({label: first_step})

    reason = first_step.get("reason", "")
    if "Read this step to answer:" not in reason:
        raise AssertionError({label: reason})
    if "If deeper evidence is needed, call" not in reason:
        raise AssertionError({label: reason})
    if "Selection reason:" not in reason:
        raise AssertionError({label: reason})
    if not first_step.get("selection_reason"):
        raise AssertionError({label: first_step})

    suggested_tool = first_step.get("suggested_tool", {})
    if not suggested_tool.get("tool"):
        raise AssertionError({label: first_step})
    if suggested_tool.get("priority", 0) < 1:
        raise AssertionError({label: first_step})
    if not suggested_tool.get("suggested_arguments"):
        raise AssertionError({label: first_step})
    if not first_step.get("ranges"):
        raise AssertionError({label: first_step})
    if first_step["ranges"][0].get("start_line", 0) < 1:
        raise AssertionError({label: first_step})
    return first_step


def assert_read_less(payload, label):
    read_less = payload.get("read_less", {})
    if not isinstance(read_less.get("baseline_source_lines"), int):
        raise AssertionError({label: read_less})
    if not isinstance(read_less.get("selected_source_lines"), int):
        raise AssertionError({label: read_less})
    if not isinstance(read_less.get("source_lines_avoided"), int):
        raise AssertionError({label: read_less})
    if read_less["baseline_source_lines"] < read_less["selected_source_lines"]:
        raise AssertionError({label: read_less})
    if read_less["source_lines_avoided"] < 0:
        raise AssertionError({label: read_less})
    if not read_less.get("line_reduction"):
        raise AssertionError({label: read_less})
    if not read_less.get("read_less_ratio"):
        raise AssertionError({label: read_less})
    return read_less


def first_omitted_candidate(payload):
    omitted_candidates = payload.get("omitted_candidates", [])
    if not omitted_candidates:
        return {}
    candidate = omitted_candidates[0]
    if not candidate.get("file"):
        raise AssertionError(candidate)
    if not isinstance(candidate.get("selection_rank"), int) or candidate["selection_rank"] < 1:
        raise AssertionError(candidate)
    if not candidate.get("omission_reason"):
        raise AssertionError(candidate)
    if not candidate.get("next_action"):
        raise AssertionError(candidate)
    return candidate


def assert_continuation_summary(payload, label):
    continuation = payload.get("continuation_summary", {})
    if not continuation.get("status"):
        raise AssertionError({label: continuation})
    if not continuation.get("next_action"):
        raise AssertionError({label: continuation})
    return continuation


def assert_agent_route_execution_evidence(route, reading_step, label):
    if route.get("current_reading_step") != reading_step:
        raise AssertionError({label: route.get("current_reading_step"), "reading_step": reading_step})
    first_execution = route["execution_plan"][0]
    if f"candidate rank {reading_step['selection_rank']}" not in first_execution.get("instruction", ""):
        raise AssertionError({label: first_execution})
    if reading_step["focus"] not in first_execution.get("instruction", ""):
        raise AssertionError({label: first_execution})
    continuation = assert_continuation_summary(route["context_pack"], label)
    continuation_instruction = route["execution_plan"][2].get("instruction", "")
    if continuation["next_action"] not in continuation_instruction:
        raise AssertionError({label: route["execution_plan"][2]})
    omitted = first_omitted_candidate(route["context_pack"])
    if omitted:
        if omitted["file"] not in continuation_instruction:
            raise AssertionError({label: route["execution_plan"][2]})
        if f"candidate rank {omitted['selection_rank']}" not in continuation_instruction:
            raise AssertionError({label: route["execution_plan"][2]})
        if omitted["omission_reason"] not in continuation_instruction:
            raise AssertionError({label: route["execution_plan"][2]})
    return continuation, omitted


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
context_reading_step = assert_actionable_reading_plan(context, "cli_context_pack")
context_read_less = assert_read_less(context, "cli_context_pack")
context_continuation = assert_continuation_summary(context, "cli_context_pack")
context_omitted = first_omitted_candidate(context)

agent_route = run_json([
    "agent-route",
    smoke_root,
    "--task",
    "understand the main application entrypoint",
    "--token-budget",
    "1200",
    "--force-index",
    "--impact-limit",
    "10",
    "--impact-depth",
    "2",
    "--impact-evidence-limit",
    "3",
])
if [step["tool"] for step in agent_route["route"]] != [
    "index_project",
    "project_overview",
    "context_pack",
    "impact_analysis",
]:
    raise AssertionError(agent_route["route"])
if [step["action"] for step in agent_route["execution_plan"]] != [
    "read_selected_context",
    "use_current_reading_step_suggested_tool",
    "use_continuation_if_needed",
    "review_impact_before_edits",
]:
    raise AssertionError(agent_route["execution_plan"])
if agent_route["execution_plan"][0]["status"] != "ready":
    raise AssertionError(agent_route["execution_plan"])
if not agent_route["execution_plan"][0]["files"]:
    raise AssertionError(agent_route["execution_plan"])
if "reading_plan[] order" not in agent_route["execution_plan"][0]["instruction"]:
    raise AssertionError(agent_route["execution_plan"])
if agent_route["context_pack"]["reading_plan"][0]["focus"] not in agent_route["execution_plan"][0]["instruction"]:
    raise AssertionError(agent_route["execution_plan"])
if not agent_route["execution_plan"][1]["suggested_tool"]["tool"]:
    raise AssertionError(agent_route["execution_plan"])
if not agent_route["context_pack"]["files"] or not agent_route["context_pack"]["reading_plan"]:
    raise AssertionError(agent_route["context_pack"])
if agent_route["context_pack"]["budget"]["applied_token_budget"] != 1200:
    raise AssertionError(agent_route["context_pack"]["budget"])
agent_route_reading_step = assert_actionable_reading_plan(
    agent_route["context_pack"],
    "cli_agent_route_context_pack",
)
agent_route_read_less = assert_read_less(
    agent_route["context_pack"],
    "cli_agent_route_context_pack",
)
agent_route_continuation, agent_route_omitted = assert_agent_route_execution_evidence(
    agent_route,
    agent_route_reading_step,
    "cli_agent_route_execution_plan",
)
if agent_route["impact_status"] != "complete":
    raise AssertionError(agent_route)
if agent_route["impact_analysis"]["format"] != "summary":
    raise AssertionError(agent_route["impact_analysis"])
if agent_route["impact_analysis"]["depth"] != 2:
    raise AssertionError(agent_route["impact_analysis"])
if agent_route["impact_analysis"]["evidence_limit"] != 3:
    raise AssertionError(agent_route["impact_analysis"])

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
    for expected in ("index_project", "project_overview", "context_pack", "agent_route", "impact_analysis", "version"):
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
    mcp_context_reading_step = assert_actionable_reading_plan(
        mcp_context,
        "mcp_context_pack",
    )
    mcp_context_read_less = assert_read_less(mcp_context, "mcp_context_pack")
    mcp_context_continuation = assert_continuation_summary(mcp_context, "mcp_context_pack")
    mcp_context_omitted = first_omitted_candidate(mcp_context)

    mcp_agent_route = request(
        {
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "agent_route",
                "arguments": {
                    "root": smoke_root,
                    "task": "understand the main application entrypoint",
                    "token_budget": 1200,
                    "force_index": True,
                    "impact_limit": 10,
                    "impact_depth": 2,
                    "impact_evidence_limit": 3,
                },
            },
        }
    )["result"]["structuredContent"]
    if [step["tool"] for step in mcp_agent_route["route"]] != [
        "index_project",
        "project_overview",
        "context_pack",
        "impact_analysis",
    ]:
        raise AssertionError(mcp_agent_route["route"])
    if [step["action"] for step in mcp_agent_route["execution_plan"]] != [
        "read_selected_context",
        "use_current_reading_step_suggested_tool",
        "use_continuation_if_needed",
        "review_impact_before_edits",
    ]:
        raise AssertionError(mcp_agent_route["execution_plan"])
    if mcp_agent_route["execution_plan"][0]["status"] != "ready":
        raise AssertionError(mcp_agent_route["execution_plan"])
    if not mcp_agent_route["execution_plan"][1]["suggested_tool"]["tool"]:
        raise AssertionError(mcp_agent_route["execution_plan"])
    if not mcp_agent_route["context_pack"]["reading_plan"]:
        raise AssertionError(mcp_agent_route["context_pack"])
    if (
        mcp_agent_route["context_pack"]["reading_plan"][0]["focus"]
        not in mcp_agent_route["execution_plan"][0]["instruction"]
    ):
        raise AssertionError(mcp_agent_route["execution_plan"])
    mcp_agent_route_reading_step = assert_actionable_reading_plan(
        mcp_agent_route["context_pack"],
        "mcp_agent_route_context_pack",
    )
    mcp_agent_route_read_less = assert_read_less(
        mcp_agent_route["context_pack"],
        "mcp_agent_route_context_pack",
    )
    mcp_agent_route_continuation, mcp_agent_route_omitted = assert_agent_route_execution_evidence(
        mcp_agent_route,
        mcp_agent_route_reading_step,
        "mcp_agent_route_execution_plan",
    )
    if mcp_agent_route["impact_status"] != "complete":
        raise AssertionError(mcp_agent_route)
    if mcp_agent_route["impact_analysis"]["depth"] != 2:
        raise AssertionError(mcp_agent_route["impact_analysis"])
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
    "context_reading_question": context_reading_step["question"],
    "context_reading_reason": context_reading_step["reason"],
    "context_selection_rank": context_reading_step["selection_rank"],
    "context_selection_reason": context_reading_step["selection_reason"],
    "context_source_lines_avoided": context_read_less["source_lines_avoided"],
    "context_read_less_ratio": context_read_less["read_less_ratio"],
    "context_continuation_status": context_continuation["status"],
    "context_continuation_next_action": context_continuation["next_action"],
    "context_first_omitted_file": context_omitted.get("file", ""),
    "context_first_omitted_selection_rank": context_omitted.get("selection_rank"),
    "context_first_omitted_omission_reason": context_omitted.get("omission_reason", ""),
    "agent_route_tools": [step["tool"] for step in agent_route["route"]],
    "agent_route_execution_plan": [step["action"] for step in agent_route["execution_plan"]],
    "agent_route_context_files": len(agent_route["context_pack"]["files"]),
    "agent_route_current_reading_step_matches_reading_plan": (
        agent_route["current_reading_step"] == agent_route_reading_step
    ),
    "agent_route_reading_question": agent_route_reading_step["question"],
    "agent_route_reading_reason": agent_route_reading_step["reason"],
    "agent_route_selection_rank": agent_route_reading_step["selection_rank"],
    "agent_route_selection_reason": agent_route_reading_step["selection_reason"],
    "agent_route_source_lines_avoided": agent_route_read_less["source_lines_avoided"],
    "agent_route_read_less_ratio": agent_route_read_less["read_less_ratio"],
    "agent_route_continuation_status": agent_route_continuation["status"],
    "agent_route_continuation_next_action": agent_route_continuation["next_action"],
    "agent_route_first_omitted_file": agent_route_omitted.get("file", ""),
    "agent_route_first_omitted_selection_rank": agent_route_omitted.get("selection_rank"),
    "agent_route_first_omitted_omission_reason": agent_route_omitted.get("omission_reason", ""),
    "agent_route_impact_status": agent_route["impact_status"],
    "mcp_agent_route_impact_status": mcp_agent_route["impact_status"],
    "mcp_agent_route_execution_plan": [step["action"] for step in mcp_agent_route["execution_plan"]],
    "mcp_agent_route_current_reading_step_matches_reading_plan": (
        mcp_agent_route["current_reading_step"] == mcp_agent_route_reading_step
    ),
    "mcp_context_reading_question": mcp_context_reading_step["question"],
    "mcp_context_reading_reason": mcp_context_reading_step["reason"],
    "mcp_context_selection_rank": mcp_context_reading_step["selection_rank"],
    "mcp_context_selection_reason": mcp_context_reading_step["selection_reason"],
    "mcp_context_source_lines_avoided": mcp_context_read_less["source_lines_avoided"],
    "mcp_context_read_less_ratio": mcp_context_read_less["read_less_ratio"],
    "mcp_context_continuation_status": mcp_context_continuation["status"],
    "mcp_context_continuation_next_action": mcp_context_continuation["next_action"],
    "mcp_context_first_omitted_file": mcp_context_omitted.get("file", ""),
    "mcp_context_first_omitted_selection_rank": mcp_context_omitted.get("selection_rank"),
    "mcp_context_first_omitted_omission_reason": mcp_context_omitted.get("omission_reason", ""),
    "mcp_agent_route_reading_question": mcp_agent_route_reading_step["question"],
    "mcp_agent_route_reading_reason": mcp_agent_route_reading_step["reason"],
    "mcp_agent_route_selection_rank": mcp_agent_route_reading_step["selection_rank"],
    "mcp_agent_route_selection_reason": mcp_agent_route_reading_step["selection_reason"],
    "mcp_agent_route_source_lines_avoided": mcp_agent_route_read_less["source_lines_avoided"],
    "mcp_agent_route_read_less_ratio": mcp_agent_route_read_less["read_less_ratio"],
    "mcp_agent_route_continuation_status": mcp_agent_route_continuation["status"],
    "mcp_agent_route_continuation_next_action": mcp_agent_route_continuation["next_action"],
    "mcp_agent_route_first_omitted_file": mcp_agent_route_omitted.get("file", ""),
    "mcp_agent_route_first_omitted_selection_rank": mcp_agent_route_omitted.get("selection_rank"),
    "mcp_agent_route_first_omitted_omission_reason": mcp_agent_route_omitted.get("omission_reason", ""),
    "mcp_tools": len(tool_names),
}, indent=2))
PY

  echo "installed quickstart smoke passed"
}

main "$@"
