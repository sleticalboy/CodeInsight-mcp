#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEMO_ROOT="${CODEINSIGHT_DEMO_ROOT:-$ROOT_DIR}"
DEMO_TASK="${CODEINSIGHT_DEMO_TASK:-understand agent context routing}"
TOKEN_BUDGET="${CODEINSIGHT_DEMO_TOKEN_BUDGET:-6000}"
IMPACT_FILE="${CODEINSIGHT_DEMO_IMPACT_FILE:-}"
FORCE_INDEX="${CODEINSIGHT_DEMO_FORCE_INDEX:-1}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

selected_context_lines() {
  local context_json="$1"
  jq -r '[.files[].ranges[] | (.end_line - .start_line + 1)] | add // 0' "$context_json"
}

require_json_number_gt_zero() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query > 0" "$file" >/dev/null; then
    echo "demo assertion failed: expected ${description} to be greater than zero" >&2
    echo "query: $query" >&2
    exit 1
  fi
}

require_json_string() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query != null and $query != \"\" and $query != \"-\"" "$file" >/dev/null; then
    echo "demo assertion failed: expected ${description} to be present" >&2
    echo "query: $query" >&2
    exit 1
  fi
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

build_binary_if_needed() {
  if [ -z "$CODEINSIGHT_BIN" ]; then
    require_command cargo
    echo "building release binary..."
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml"
    CODEINSIGHT_BIN="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r '.target_directory')/release/codeinsight"
  fi

  if [ -x "$CODEINSIGHT_BIN" ]; then
    return
  fi

  echo "CODEINSIGHT_BIN is not executable: $CODEINSIGHT_BIN" >&2
  exit 1
}

main() {
  require_command jq
  build_binary_if_needed

  local tmp_dir index_json overview_json context_json impact_json
  tmp_dir="$(mktemp -d)"
  trap "rm -rf '$tmp_dir'" EXIT

  index_json="$tmp_dir/index.json"
  overview_json="$tmp_dir/overview.json"
  context_json="$tmp_dir/context.json"
  impact_json="$tmp_dir/impact.json"

  echo "CodeInsight agent context router demo"
  echo "root: $DEMO_ROOT"
  echo "task: $DEMO_TASK"
  echo "token_budget: $TOKEN_BUDGET"
  echo

  if [ "$FORCE_INDEX" = "1" ]; then
    "$CODEINSIGHT_BIN" index "$DEMO_ROOT" --force >"$index_json"
  else
    "$CODEINSIGHT_BIN" index "$DEMO_ROOT" >"$index_json"
  fi

  "$CODEINSIGHT_BIN" overview "$DEMO_ROOT" >"$overview_json"
  "$CODEINSIGHT_BIN" context-pack "$DEMO_ROOT" \
    --task "$DEMO_TASK" \
    --token-budget "$TOKEN_BUDGET" \
    >"$context_json"

  require_json_number_gt_zero "$context_json" '.files | length' "context_pack selected files"
  require_json_number_gt_zero "$context_json" '.reading_plan | length' "context_pack reading plan steps"
  require_json_string "$context_json" '.reading_plan[0].next_action' "first reading-plan next action"
  require_json_string "$context_json" '.reading_plan[0].reason' "first reading-plan reason"
  require_json_string "$context_json" '.reading_plan[0].selection_reason' "first reading-plan selection reason"

  if [ -z "$IMPACT_FILE" ]; then
    IMPACT_FILE="$(json_value "$context_json" '.files[0].file // empty')"
  fi

  if [ -n "$IMPACT_FILE" ]; then
    "$CODEINSIGHT_BIN" impact-analysis "$DEMO_ROOT" \
      --file "$IMPACT_FILE" \
      --depth 2 \
      --format summary \
      >"$impact_json"
  fi

  local total_lines selected_lines reduction first_entrypoint first_context_file
  local first_next_action first_reading_plan_reason first_selection_reason
  total_lines="$(json_value "$overview_json" '.total_lines // 0')"
  selected_lines="$(selected_context_lines "$context_json")"
  reduction="$(line_reduction "$total_lines" "$selected_lines")"
  first_entrypoint="$(json_value "$overview_json" '.entrypoints[0].file // "-"')"
  first_context_file="$(json_value "$context_json" '.files[0].file // "-"')"
  first_next_action="$(json_value "$context_json" '.reading_plan[0].next_action // "-"')"
  first_reading_plan_reason="$(json_value "$context_json" '.reading_plan[0].reason // "-"')"
  first_selection_reason="$(json_value "$context_json" '.reading_plan[0].selection_reason // "-"')"

  echo "1. index_project"
  echo "   indexed_files: $(json_value "$index_json" '.indexed_files')"
  echo "   symbols: $(json_value "$index_json" '.symbols')"
  echo "   duration_ms: $(json_value "$index_json" '.duration_ms')"
  echo "   errors: $(json_value "$index_json" '.errors | length')"
  echo

  echo "2. project_overview"
  echo "   total_lines: $total_lines"
  echo "   entrypoints: $(json_value "$overview_json" '.entrypoints | length')"
  echo "   first_entrypoint: $first_entrypoint"
  echo "   recommended_next_tools: $(json_value "$overview_json" '.recommended_next_tools | length')"
  echo

  echo "3. context_pack"
  echo "   selected_files: $(json_value "$context_json" '.files | length')"
  echo "   selected_ranges: $(json_value "$context_json" '[.files[].ranges | length] | add // 0')"
  echo "   reading_plan_steps: $(json_value "$context_json" '.reading_plan | length')"
  echo "   first_next_action: $first_next_action"
  echo "   selected_lines: $selected_lines"
  echo "   line_reduction: $reduction"
  echo "   estimated_tokens: $(json_value "$context_json" '.estimated_tokens')"
  echo "   continuation: $(json_value "$context_json" '.continuation_summary.status // "-"')"
  echo "   first_context_file: $first_context_file"
  echo "   reading_plan_reason: $first_reading_plan_reason"
  echo "   selection_reason: $first_selection_reason"
  echo

  if [ -s "$impact_json" ]; then
    echo "4. impact_analysis"
    echo "   seed_file: $IMPACT_FILE"
    echo "   risk_level: $(json_value "$impact_json" '.risk_level // "-"')"
    echo "   impacted_files: $(json_value "$impact_json" '.impact_counts.impacted_files // 0')"
    echo "   paths: $(json_value "$impact_json" '.impact_counts.paths // 0')"
    echo "   suggested_checks: $(json_value "$impact_json" '.suggested_checks | length')"
    echo "   summary: $(json_value "$impact_json" '.summary // "-"')"
    echo "   call_related_files: $(json_value "$impact_json" '.impact_breakdown.call_related_files // 0')"
    echo "   dependency_related_files: $(json_value "$impact_json" '.impact_breakdown.dependency_related_files // 0')"
  else
    echo "4. impact_analysis"
    echo "   skipped: context_pack did not select a file seed"
  fi

  echo
  echo "Run against another repository:"
  echo "  CODEINSIGHT_DEMO_ROOT=/path/to/repo scripts/agent-router-demo.sh"
}

main "$@"
