#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${CODEINSIGHT_DEMO_ROOT:-$ROOT_DIR}"
DEMO_TASK="${CODEINSIGHT_DEMO_TASK:-understand agent context routing}"
TOKEN_BUDGET="${CODEINSIGHT_DEMO_TOKEN_BUDGET:-6000}"
FORCE_INDEX="${CODEINSIGHT_DEMO_FORCE_INDEX:-1}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "two-minute demo failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

require_json_number_gt_zero() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query > 0" "$file" >/dev/null; then
    fail "expected ${description} to be greater than zero"
  fi
}

require_json_string() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query != null and $query != \"\" and $query != \"-\"" "$file" >/dev/null; then
    fail "expected ${description} to be present"
  fi
}

selected_context_lines() {
  local route_json="$1"
  jq -r '[.context_pack.files[].ranges[] | (.end_line - .start_line + 1)] | add // 0' "$route_json"
}

line_reduction() {
  local total_lines="$1"
  local selected_lines="$2"
  awk -v total="$total_lines" -v selected="$selected_lines" 'BEGIN {
    if (total <= 0) {
      printf "n/a"
    } else {
      reduction = (1 - (selected / total)) * 100
      if (reduction < 0) reduction = 0
      printf "%.1f%%", reduction
    }
  }'
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

build_binary_if_needed() {
  if [ -z "$CODEINSIGHT_BIN" ]; then
    require_command cargo
    echo "building release binary..."
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml"
    CODEINSIGHT_BIN="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r '.target_directory')/release/codeinsight"
  fi

  if [ ! -x "$CODEINSIGHT_BIN" ]; then
    fail "CODEINSIGHT_BIN is not executable: $CODEINSIGHT_BIN"
  fi
}

main() {
  require_command jq

  echo "CodeInsight two-minute demo"
  echo
  echo "Problem: AI agents waste the first read by scanning broad files and guessing entrypoints."
  echo "Promise: route the agent through agent_route before edits."
  echo
  echo "[Live run]"

  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local route_json
  route_json="$TEMP_DIR/agent-route.json"

  echo "CodeInsight agent_route demo"
  echo "root: $DEMO_ROOT"
  echo "task: $DEMO_TASK"
  echo "token_budget: $TOKEN_BUDGET"
  echo

  local args
  args=(
    "agent-route"
    "$DEMO_ROOT"
    "--task"
    "$DEMO_TASK"
    "--token-budget"
    "$TOKEN_BUDGET"
    "--impact-depth"
    "2"
    "--impact-evidence-limit"
    "20"
  )
  if [ "$FORCE_INDEX" = "1" ]; then
    args+=("--force-index")
  fi

  "$CODEINSIGHT_BIN" "${args[@]}" >"$route_json"

  require_json_number_gt_zero "$route_json" '.index_report.indexed_files' "indexed files"
  require_json_number_gt_zero "$route_json" '.context_pack.files | length' "context_pack selected files"
  require_json_number_gt_zero "$route_json" '.context_pack.reading_plan | length' "context_pack reading plan steps"
  require_json_number_gt_zero "$route_json" '.execution_plan | length' "agent_route execution plan steps"
  require_json_string "$route_json" '.context_pack.reading_plan[0].next_action' "first reading-plan next action"
  require_json_string "$route_json" '.context_pack.reading_plan[0].reason' "first reading-plan reason"
  require_json_string "$route_json" '.context_pack.reading_plan[0].selection_reason' "first reading-plan selection reason"
  require_json_string "$route_json" '.execution_plan[0].action' "first execution-plan action"
  require_json_string "$route_json" '.execution_plan[1].suggested_tool.tool' "first execution-plan suggested tool"

  local total_lines selected_lines reduction first_entrypoint first_context_file
  local entrypoints recommended_tools selected_files selected_ranges reading_plan_steps
  local execution_plan_steps first_execution_action second_execution_action
  local first_execution_suggested_tool first_next_action first_reading_reason first_selection_reason
  local continuation risk_level impacted_files suggested_checks impact_seed_file

  total_lines="$(json_value "$route_json" '.overview.total_lines // 0')"
  selected_lines="$(selected_context_lines "$route_json")"
  reduction="$(line_reduction "$total_lines" "$selected_lines")"
  first_entrypoint="$(json_value "$route_json" '.overview.entrypoints[0].file // "-"')"
  first_context_file="$(json_value "$route_json" '.context_pack.files[0].file // "-"')"
  entrypoints="$(json_value "$route_json" '.overview.entrypoints | length')"
  recommended_tools="$(json_value "$route_json" '.overview.recommended_next_tools | length')"
  selected_files="$(json_value "$route_json" '.context_pack.files | length')"
  selected_ranges="$(json_value "$route_json" '.context_pack.budget.selected_ranges')"
  reading_plan_steps="$(json_value "$route_json" '.context_pack.reading_plan | length')"
  execution_plan_steps="$(json_value "$route_json" '.execution_plan | length')"
  first_execution_action="$(json_value "$route_json" '.execution_plan[0].action // "-"')"
  second_execution_action="$(json_value "$route_json" '.execution_plan[1].action // "-"')"
  first_execution_suggested_tool="$(json_value "$route_json" '.execution_plan[1].suggested_tool.tool // "-"')"
  first_next_action="$(json_value "$route_json" '.context_pack.reading_plan[0].next_action // "-"')"
  first_reading_reason="$(json_value "$route_json" '.context_pack.reading_plan[0].reason // "-"')"
  first_selection_reason="$(json_value "$route_json" '.context_pack.reading_plan[0].selection_reason // "-"')"
  context_route_reason="$(json_value "$route_json" '.route[] | select(.tool == "context_pack") | .reason')"
  continuation="$(json_value "$route_json" '.context_pack.continuation_summary.status // "-"')"
  impact_route_reason="$(json_value "$route_json" '.route[] | select(.tool == "impact_analysis") | .reason')"
  risk_level="$(json_value "$route_json" '.impact_analysis.risk_level // empty')"
  impacted_files="$(json_value "$route_json" '.impact_analysis.impact_counts.impacted_files // 0')"
  suggested_checks="$(json_value "$route_json" '.impact_analysis.suggested_checks | length')"
  impact_seed_file="$(json_value "$route_json" '.impact_seed_files[0] // "-"')"

  echo "1. index_project"
  echo "   indexed_files: $(json_value "$route_json" '.index_report.indexed_files')"
  echo "   symbols: $(json_value "$route_json" '.index_report.symbols')"
  echo "   duration_ms: $(json_value "$route_json" '.index_report.duration_ms')"
  echo "   errors: $(json_value "$route_json" '.index_report.errors | length')"
  echo

  echo "2. project_overview"
  echo "   total_lines: $total_lines"
  echo "   entrypoints: $entrypoints"
  echo "   first_entrypoint: $first_entrypoint"
  echo "   recommended_next_tools: $recommended_tools"
  echo

  echo "3. context_pack"
  echo "   selected_files: $selected_files"
  echo "   selected_ranges: $selected_ranges"
  echo "   reading_plan_steps: $reading_plan_steps"
  echo "   execution_plan_steps: $execution_plan_steps"
  echo "   first_execution_action: $first_execution_action"
  echo "   second_execution_action: $second_execution_action"
  echo "   first_execution_suggested_tool: $first_execution_suggested_tool"
  echo "   first_next_action: $first_next_action"
  echo "   selected_lines: $selected_lines"
  echo "   line_reduction: $reduction"
  echo "   estimated_tokens: $(json_value "$route_json" '.context_pack.estimated_tokens')"
  echo "   continuation: $continuation"
  echo "   first_context_file: $first_context_file"
  echo "   reading_plan_reason: $first_reading_reason"
  echo "   selection_reason: $first_selection_reason"
  echo "   route_reason: $context_route_reason"
  echo

  echo "4. impact_analysis"
  if [ -n "$risk_level" ]; then
    echo "   seed_file: $impact_seed_file"
    echo "   risk_level: $risk_level"
    echo "   impacted_files: $impacted_files"
    echo "   paths: $(json_value "$route_json" '.impact_analysis.impact_counts.paths // 0')"
    echo "   suggested_checks: $suggested_checks"
    echo "   route_reason: $impact_route_reason"
  else
    echo "   skipped: agent_route did not produce an impact seed"
  fi

  echo
  echo "Run against another repository:"
  echo "  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh"
  echo
  echo "[Evidence summary]"
  echo "agent_route selected ${selected_lines}/${total_lines} source lines (${reduction} reduction) across ${selected_files} files."
  echo "The first selected file is ${first_context_file}; read it before offering ${first_execution_suggested_tool}."
  if [ -n "$risk_level" ]; then
    echo "Before edits, impact_analysis reports ${risk_level} risk across ${impacted_files} impacted files."
  else
    echo "Before edits, run impact_analysis when context_pack selects a file seed."
  fi
  echo
  echo "[Talk track]"
  echo "1. agent_route ran index_project, project_overview, context_pack, and impact_analysis in one call."
  echo "2. project_overview found ${entrypoints} entrypoints and ${recommended_tools} recommended next tools."
  echo "3. context_pack selected ${selected_files} files and ${selected_ranges} ranges, then produced ${reading_plan_steps} reading-plan steps."
  echo "4. execution_plan starts with ${first_execution_action}, then ${second_execution_action}; this keeps suggested tools behind selected-context reading."
  echo "5. The first execution-plan suggested tool is ${first_execution_suggested_tool}; offer it only after the selected file has been read."
  echo "6. The first reading-plan action is ${first_next_action}; ${first_reading_reason}"
  echo "7. The selected context reduced source reading by ${reduction}; ${context_route_reason}"
  echo "8. Selection evidence: ${first_selection_reason}"
  echo "9. Continuation status is ${continuation}, so the agent knows whether to ask for a focused follow-up."
  if [ -n "$risk_level" ]; then
    echo "10. impact_analysis reports ${risk_level} risk across ${impacted_files} impacted files with ${suggested_checks} suggested checks; ${impact_route_reason}"
  else
    echo "10. impact_analysis is the pre-edit step when context_pack selects a file seed."
  fi
  echo
  echo "[Agent policy]"
  echo "Call agent_route with root, task, and token_budget for the default first read."
  echo "Read context_pack.files in reading_plan order, use continuation_summary only after that, and run focused follow-up tools only when needed."
  echo
  echo "Run this walkthrough against another repository:"
  echo "  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh"
}

main "$@"
