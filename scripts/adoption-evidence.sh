#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${CODEINSIGHT_ADOPTION_ROOT:-}"
TASK="${CODEINSIGHT_ADOPTION_TASK:-understand the main application entrypoint}"
TOKEN_BUDGET="${CODEINSIGHT_ADOPTION_TOKEN_BUDGET:-6000}"
OUTPUT_DIR="${CODEINSIGHT_ADOPTION_OUTPUT_DIR:-}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
FORCE_INDEX="${CODEINSIGHT_ADOPTION_FORCE_INDEX:-1}"
LOCAL_REPO_EVIDENCE_SCRIPT="${CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT:-$ROOT_DIR/scripts/local-repo-evidence.sh}"
MCP_FIRST_CALL_SMOKE_SCRIPT="${CODEINSIGHT_MCP_FIRST_CALL_SMOKE_SCRIPT:-$ROOT_DIR/scripts/mcp-first-call-smoke.sh}"

usage() {
  cat <<'EOF'
usage: scripts/adoption-evidence.sh [REPO_ROOT] [options]

Builds a user-facing adoption evidence bundle for a local repository:
local first-read evidence, raw agent_route JSON, compact local evidence JSON,
MCP first-call JSON, and an aggregate Markdown/JSON summary.

Options:
  --root PATH           Repository root. Also accepted as the first argument.
  --task TEXT           Task for local evidence and MCP first-call checks.
  --token-budget N      Token budget for context routing. Default: 6000.
  --output-dir PATH     Evidence output directory. Default: /tmp/codeinsight-adoption-evidence.
  --bin PATH            Use a specific codeinsight binary.
  --no-force-index      Reuse the existing index when available.
  -h, --help            Show this help text.

Environment:
  CODEINSIGHT_ADOPTION_ROOT
  CODEINSIGHT_ADOPTION_TASK
  CODEINSIGHT_ADOPTION_TOKEN_BUDGET
  CODEINSIGHT_ADOPTION_OUTPUT_DIR
  CODEINSIGHT_ADOPTION_FORCE_INDEX
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "adoption evidence failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --root)
        [ "$#" -ge 2 ] || fail "--root requires a path"
        REPO_ROOT="$2"
        shift 2
        ;;
      --task)
        [ "$#" -ge 2 ] || fail "--task requires text"
        TASK="$2"
        shift 2
        ;;
      --token-budget)
        [ "$#" -ge 2 ] || fail "--token-budget requires a number"
        TOKEN_BUDGET="$2"
        shift 2
        ;;
      --output-dir)
        [ "$#" -ge 2 ] || fail "--output-dir requires a path"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --bin)
        [ "$#" -ge 2 ] || fail "--bin requires a path"
        CODEINSIGHT_BIN="$2"
        shift 2
        ;;
      --no-force-index)
        FORCE_INDEX="0"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        fail "unknown argument: $1"
        ;;
      *)
        if [ -n "$REPO_ROOT" ]; then
          fail "unexpected positional argument: $1"
        fi
        REPO_ROOT="$1"
        shift
        ;;
    esac
  done
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

write_markdown_summary() {
  local target="$1"
  local local_summary="$2"
  local mcp_summary="$3"

  {
    echo "# CodeInsight Adoption Evidence"
    echo
    echo "- Repository: \`$REPO_ROOT\`"
    echo "- Task: \`$TASK\`"
    echo "- Token budget: \`$TOKEN_BUDGET\`"
    echo "- Status: \`pass\`"
    echo
    echo "## Key Results"
    echo
    echo "- Route: \`$(json_value "$local_summary" '.route_tools | join(" -> ")')\`"
    echo "- Selected context: \`$(json_value "$local_summary" '.metrics.selected_lines')/$(json_value "$local_summary" '.metrics.total_lines')\` source lines, \`$(json_value "$local_summary" '.metrics.line_reduction')\` reduction"
    echo "- First selected file: \`$(json_value "$local_summary" '.metrics.first_file')\`"
    echo "- First reading question: $(json_value "$local_summary" '.metrics.first_reading_question')"
    echo "- First suggested tool: \`$(json_value "$local_summary" '.metrics.first_suggested_tool')\`"
    echo "- Impact risk: \`$(json_value "$local_summary" '.metrics.risk_level')\`"
    echo "- Impacted files: \`$(json_value "$local_summary" '.metrics.impacted_files')\`"
    echo "- MCP server: \`$(json_value "$mcp_summary" '.server')\`"
    echo "- MCP suggested tool executed: \`$(json_value "$mcp_summary" '.suggested_tool_executed')\`"
    echo "- MCP impact status: \`$(json_value "$mcp_summary" '.impact_status')\`"
    echo
    echo "## Artifacts"
    echo
    echo "- Local evidence Markdown: \`$OUTPUT_DIR/local-repo-evidence.md\`"
    echo "- Local evidence summary JSON: \`$OUTPUT_DIR/local-repo-evidence.json\`"
    echo "- Raw agent_route JSON: \`$OUTPUT_DIR/agent-route.json\`"
    echo "- MCP first-call summary JSON: \`$OUTPUT_DIR/mcp-first-call.json\`"
    echo "- Aggregate summary JSON: \`$OUTPUT_DIR/summary.json\`"
    echo
    echo "## Adoption Policy"
    echo
    echo "1. Start broad repository tasks with \`agent_route\`."
    echo "2. Read \`context_pack.files[]\` in \`reading_plan[]\` order."
    echo "3. Use the suggested tool only after selected context is read."
    echo "4. Review \`impact_analysis\` before editing."
  } >"$target"
}

write_summary_json() {
  local target="$1"
  local local_summary="$2"
  local mcp_summary="$3"

  jq -n \
    --arg repository "$REPO_ROOT" \
    --arg task "$TASK" \
    --arg output_dir "$OUTPUT_DIR" \
    --slurpfile local "$local_summary" \
    --slurpfile mcp "$mcp_summary" \
    '{
      status: "pass",
      repository: $repository,
      task: $task,
      output_dir: $output_dir,
      local_evidence: $local[0],
      mcp_first_call: $mcp[0],
      artifacts: {
        markdown: ($output_dir + "/adoption-evidence.md"),
        local_markdown: ($output_dir + "/local-repo-evidence.md"),
        local_summary_json: ($output_dir + "/local-repo-evidence.json"),
        raw_agent_route_json: ($output_dir + "/agent-route.json"),
        mcp_first_call_json: ($output_dir + "/mcp-first-call.json")
      }
    }' >"$target"

  jq -e \
    '.status == "pass"
      and .local_evidence.status == "pass"
      and .mcp_first_call.status == "pass"
      and .local_evidence.route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .mcp_first_call.route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .mcp_first_call.suggested_tool_executed == true' \
    "$target" >/dev/null ||
    fail "aggregate summary JSON does not match the adoption evidence contract"
}

main() {
  parse_args "$@"
  require_command jq

  if [ -z "$REPO_ROOT" ]; then
    fail "missing repository root"
  fi
  if [ ! -d "$REPO_ROOT" ]; then
    fail "repository root does not exist: $REPO_ROOT"
  fi
  case "$TOKEN_BUDGET" in
    ''|*[!0-9]*)
      fail "--token-budget must be a positive integer"
      ;;
  esac
  if [ "$TOKEN_BUDGET" -le 0 ]; then
    fail "--token-budget must be greater than zero"
  fi
  if [ ! -x "$LOCAL_REPO_EVIDENCE_SCRIPT" ]; then
    fail "local repo evidence script is not executable: $LOCAL_REPO_EVIDENCE_SCRIPT"
  fi
  if [ ! -x "$MCP_FIRST_CALL_SMOKE_SCRIPT" ]; then
    fail "MCP first-call smoke script is not executable: $MCP_FIRST_CALL_SMOKE_SCRIPT"
  fi

  REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
  OUTPUT_DIR="${OUTPUT_DIR:-/tmp/codeinsight-adoption-evidence}"
  mkdir -p "$OUTPUT_DIR"

  local local_args mcp_env
  local_args=(
    "$REPO_ROOT"
    "--task"
    "$TASK"
    "--token-budget"
    "$TOKEN_BUDGET"
    "--output"
    "$OUTPUT_DIR/local-repo-evidence.md"
    "--json"
    "$OUTPUT_DIR/agent-route.json"
    "--summary-json"
    "$OUTPUT_DIR/local-repo-evidence.json"
  )
  if [ -n "$CODEINSIGHT_BIN" ]; then
    local_args+=("--bin" "$CODEINSIGHT_BIN")
  fi
  if [ "$FORCE_INDEX" != "1" ]; then
    local_args+=("--no-force-index")
  fi

  "$LOCAL_REPO_EVIDENCE_SCRIPT" "${local_args[@]}" >/dev/null

  mcp_env=(
    "CODEINSIGHT_FIRST_CALL_ROOT=$REPO_ROOT"
    "CODEINSIGHT_FIRST_CALL_TASK=$TASK"
    "CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET=$TOKEN_BUDGET"
  )
  if [ -n "$CODEINSIGHT_BIN" ]; then
    mcp_env+=("CODEINSIGHT_BIN=$CODEINSIGHT_BIN")
  fi

  env "${mcp_env[@]}" \
    "$MCP_FIRST_CALL_SMOKE_SCRIPT" \
    --summary-json "$OUTPUT_DIR/mcp-first-call.json" >/dev/null

  write_summary_json "$OUTPUT_DIR/summary.json" \
    "$OUTPUT_DIR/local-repo-evidence.json" \
    "$OUTPUT_DIR/mcp-first-call.json"
  write_markdown_summary "$OUTPUT_DIR/adoption-evidence.md" \
    "$OUTPUT_DIR/local-repo-evidence.json" \
    "$OUTPUT_DIR/mcp-first-call.json"

  echo "adoption evidence written to $OUTPUT_DIR"
  echo "markdown: $OUTPUT_DIR/adoption-evidence.md"
  echo "summary_json: $OUTPUT_DIR/summary.json"
}

main "$@"
