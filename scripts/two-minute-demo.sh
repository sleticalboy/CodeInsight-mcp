#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
AGENT_ROUTER_DEMO_SCRIPT="${CODEINSIGHT_AGENT_ROUTER_DEMO_SCRIPT:-$ROOT_DIR/scripts/agent-router-demo.sh}"
OUTPUT_FILE=""

fail() {
  echo "two-minute demo failed: $*" >&2
  exit 1
}

extract_metric() {
  local output_file="$1"
  local key="$2"

  awk -F': ' -v key="$key" '$1 ~ "^[[:space:]]*" key "$" { print $2; exit }' "$output_file"
}

require_metric() {
  local output_file="$1"
  local key="$2"
  local value

  value="$(extract_metric "$output_file" "$key")"
  if [ -z "$value" ] || [ "$value" = "-" ]; then
    fail "missing metric: $key"
  fi
  printf "%s" "$value"
}

cleanup() {
  if [ -n "$OUTPUT_FILE" ]; then
    rm -f "$OUTPUT_FILE"
  fi
}

main() {
  local entrypoints recommended_tools selected_files selected_ranges reading_plan_steps
  local first_next_action line_reduction continuation risk_level impacted_files suggested_checks

  if [ ! -x "$AGENT_ROUTER_DEMO_SCRIPT" ]; then
    fail "agent router demo script is not executable: $AGENT_ROUTER_DEMO_SCRIPT"
  fi

  OUTPUT_FILE="$(mktemp)"
  trap cleanup EXIT INT TERM

  echo "CodeInsight two-minute demo"
  echo
  echo "Problem: AI agents waste the first read by scanning broad files and guessing entrypoints."
  echo "Promise: route the agent through project_overview, context_pack, and impact_analysis before edits."
  echo
  echo "[Live run]"

  "$AGENT_ROUTER_DEMO_SCRIPT" | tee "$OUTPUT_FILE"

  entrypoints="$(require_metric "$OUTPUT_FILE" "entrypoints")"
  recommended_tools="$(require_metric "$OUTPUT_FILE" "recommended_next_tools")"
  selected_files="$(require_metric "$OUTPUT_FILE" "selected_files")"
  selected_ranges="$(require_metric "$OUTPUT_FILE" "selected_ranges")"
  reading_plan_steps="$(require_metric "$OUTPUT_FILE" "reading_plan_steps")"
  first_next_action="$(require_metric "$OUTPUT_FILE" "first_next_action")"
  line_reduction="$(require_metric "$OUTPUT_FILE" "line_reduction")"
  continuation="$(require_metric "$OUTPUT_FILE" "continuation")"
  risk_level="$(extract_metric "$OUTPUT_FILE" "risk_level")"
  impacted_files="$(extract_metric "$OUTPUT_FILE" "impacted_files")"
  suggested_checks="$(extract_metric "$OUTPUT_FILE" "suggested_checks")"

  echo
  echo "[Talk track]"
  echo "1. project_overview found ${entrypoints} entrypoints and ${recommended_tools} recommended next tools."
  echo "2. context_pack selected ${selected_files} files and ${selected_ranges} ranges, then produced ${reading_plan_steps} reading-plan steps."
  echo "3. The first action is ${first_next_action}; the selected context reduced source reading by ${line_reduction}."
  echo "4. Continuation status is ${continuation}, so the agent knows whether to ask for a focused follow-up."
  if [ -n "$risk_level" ]; then
    echo "5. impact_analysis reports ${risk_level} risk across ${impacted_files:-0} impacted files with ${suggested_checks:-0} suggested checks."
  else
    echo "5. impact_analysis is the pre-edit step when context_pack selects a file seed."
  fi
  echo
  echo "[Agent policy]"
  echo "Call index_project, then project_overview, then context_pack with a token budget."
  echo "Read context_pack.files in reading_plan order, use continuation_summary only after that, and run impact_analysis before edits."
  echo
  echo "Run this walkthrough against another repository:"
  echo "  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/two-minute-demo.sh"
}

main "$@"
