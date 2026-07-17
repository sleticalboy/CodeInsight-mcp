#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${CODEINSIGHT_EVIDENCE_ROOT:-}"
TASK="${CODEINSIGHT_EVIDENCE_TASK:-understand the main application entrypoint}"
TOKEN_BUDGET="${CODEINSIGHT_EVIDENCE_TOKEN_BUDGET:-6000}"
OUTPUT_FILE="${CODEINSIGHT_EVIDENCE_OUTPUT:-}"
JSON_FILE="${CODEINSIGHT_EVIDENCE_JSON:-}"
SUMMARY_JSON="${CODEINSIGHT_EVIDENCE_SUMMARY_JSON:-}"
FORCE_INDEX="${CODEINSIGHT_EVIDENCE_FORCE_INDEX:-1}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

usage() {
  cat <<'EOF'
usage: scripts/local-repo-evidence.sh [REPO_ROOT] [options]

Generates a copyable Markdown evidence summary for CodeInsight's first-read
route on a local repository.

Options:
  --root PATH           Repository root. Also accepted as the first argument.
  --task TEXT           Task for agent_route.
  --token-budget N      Token budget for context_pack. Default: 6000.
  --output PATH         Write Markdown evidence to PATH instead of stdout.
  --json PATH           Save the raw agent_route JSON to PATH.
  --summary-json PATH   Write a compact machine-readable evidence summary.
  --bin PATH            Use a specific codeinsight binary.
  --no-force-index      Reuse the existing index when available.
  -h, --help            Show this help text.

Environment:
  CODEINSIGHT_EVIDENCE_ROOT
  CODEINSIGHT_EVIDENCE_TASK
  CODEINSIGHT_EVIDENCE_TOKEN_BUDGET
  CODEINSIGHT_EVIDENCE_OUTPUT
  CODEINSIGHT_EVIDENCE_JSON
  CODEINSIGHT_EVIDENCE_SUMMARY_JSON
  CODEINSIGHT_EVIDENCE_FORCE_INDEX
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "local repo evidence failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
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
      --output)
        [ "$#" -ge 2 ] || fail "--output requires a path"
        OUTPUT_FILE="$2"
        shift 2
        ;;
      --json)
        [ "$#" -ge 2 ] || fail "--json requires a path"
        JSON_FILE="$2"
        shift 2
        ;;
      --summary-json)
        [ "$#" -ge 2 ] || fail "--summary-json requires a path"
        SUMMARY_JSON="$2"
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

build_binary_if_needed() {
  if [ -z "$CODEINSIGHT_BIN" ]; then
    require_command cargo
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml" >/dev/null
    CODEINSIGHT_BIN="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r '.target_directory')/release/codeinsight"
  fi

  if [ ! -x "$CODEINSIGHT_BIN" ]; then
    fail "CODEINSIGHT_BIN is not executable: $CODEINSIGHT_BIN"
  fi
}

write_markdown() {
  local route_json="$1"
  local target="$2"
  local total_lines selected_lines reduction risk_level

  total_lines="$(json_value "$route_json" '.overview.total_lines // 0')"
  selected_lines="$(selected_context_lines "$route_json")"
  reduction="$(line_reduction "$total_lines" "$selected_lines")"
  risk_level="$(json_value "$route_json" '.impact_analysis.risk_level // "not_available"')"

  {
    echo "# CodeInsight Local Repository Evidence"
    echo
    echo "- Repository: \`${REPO_ROOT}\`"
    echo "- Task: \`${TASK}\`"
    echo "- Token budget: \`${TOKEN_BUDGET}\`"
    echo "- Route: \`$(json_value "$route_json" '.route | map(.tool) | join(" -> ")')\`"
    echo
    echo "## Key Results"
    echo
    echo "- Indexed files: \`$(json_value "$route_json" '.index_report.indexed_files')\`"
    echo "- Symbols: \`$(json_value "$route_json" '.index_report.symbols')\`"
    echo "- Entrypoints: \`$(json_value "$route_json" '.overview.entrypoints | length')\`"
    echo "- Recommended next tools: \`$(json_value "$route_json" '.overview.recommended_next_tools | length')\`"
    echo "- Selected context: \`${selected_lines}/${total_lines}\` source lines, \`${reduction}\` reduction"
    echo "- Selected files: \`$(json_value "$route_json" '.context_pack.files | length')\`"
    echo "- Selected ranges: \`$(json_value "$route_json" '.context_pack.budget.selected_ranges')\`"
    echo "- Estimated tokens: \`$(json_value "$route_json" '.context_pack.estimated_tokens')\`"
    echo "- Reading-plan steps: \`$(json_value "$route_json" '.context_pack.reading_plan | length')\`"
    echo "- Seed strategy: \`$(json_value "$route_json" '.context_pack.seed_strategy // "-"')\`"
    echo "- Selected seeds: \`$(json_value "$route_json" '.context_pack.selected_seeds | length')\`"
    echo "- First seed source: \`$(json_value "$route_json" '.context_pack.selected_seeds[0].source // "-"')\`"
    echo "- Companion entrypoint: \`$(json_value "$route_json" '([.context_pack.selected_seeds[1:][]? | select(.source == "overview_entrypoint") | .value] | first) // "-"')\`"
    echo "- First selected file: \`$(json_value "$route_json" '.context_pack.files[0].file // "-"')\`"
    echo "- First reading question: $(json_value "$route_json" '.context_pack.reading_plan[0].question // "-"')"
    echo "- First next action: \`$(json_value "$route_json" '.context_pack.reading_plan[0].next_action // "-"')\`"
    echo "- First suggested tool: \`$(json_value "$route_json" '.execution_plan[1].suggested_tool.tool // "-"')\`"
    echo "- Continuation status: \`$(json_value "$route_json" '.context_pack.continuation_summary.status // "-"')\`"
    echo "- Impact risk: \`${risk_level}\`"
    echo "- Impacted files: \`$(json_value "$route_json" '.impact_analysis.impact_counts.impacted_files // 0')\`"
    echo "- Suggested checks: \`$(json_value "$route_json" '.impact_analysis.suggested_checks | length')\`"
    echo
    echo "## Agent Policy"
    echo
    echo "1. Read \`context_pack.files[]\` in \`reading_plan[]\` order."
    echo "2. Use \`reading_plan[].question\` as the local checklist for each selected file."
    echo "3. Offer \`reading_plan[].suggested_tool\` only after the current selected file has been read."
    echo "4. Review \`impact_analysis\` before editing."
    echo
    echo "## Route Reasons"
    echo
    echo "- Context route: $(json_value "$route_json" '.route[] | select(.tool == "context_pack") | .reason')"
    echo "- Impact route: $(json_value "$route_json" '.route[] | select(.tool == "impact_analysis") | .reason')"
    if [ -n "$JSON_FILE" ]; then
      echo
      echo "Raw agent_route JSON: \`${JSON_FILE}\`"
    fi
  } >"$target"
}

write_summary_json() {
  local route_json="$1"
  local target="$2"
  local total_lines selected_lines reduction

  total_lines="$(json_value "$route_json" '.overview.total_lines // 0')"
  selected_lines="$(selected_context_lines "$route_json")"
  reduction="$(line_reduction "$total_lines" "$selected_lines")"

  mkdir -p "$(dirname "$target")"
  jq \
    --arg repository "$REPO_ROOT" \
    --arg markdown "$OUTPUT_FILE" \
    --arg raw_agent_route_json "$JSON_FILE" \
    --argjson selected_lines "$selected_lines" \
    --arg line_reduction "$reduction" \
    '{
      status: "pass",
      repository: $repository,
      task,
      token_budget,
      route_tools: [.route[].tool],
      execution_plan_actions: [.execution_plan[].action],
      metrics: {
        indexed_files: .index_report.indexed_files,
        symbols: .index_report.symbols,
        index_errors: (.index_report.errors | length),
        entrypoints: (.overview.entrypoints | length),
        recommended_next_tools: (.overview.recommended_next_tools | length),
        total_lines: (.overview.total_lines // 0),
        selected_lines: $selected_lines,
        line_reduction: $line_reduction,
        selected_files: (.context_pack.files | length),
        selected_ranges: .context_pack.budget.selected_ranges,
        estimated_tokens: .context_pack.estimated_tokens,
        reading_plan_steps: (.context_pack.reading_plan | length),
        execution_plan_steps: (.execution_plan | length),
        seed_strategy: (.context_pack.seed_strategy // ""),
        selected_seed_count: (.context_pack.selected_seeds | length),
        first_seed_source: (.context_pack.selected_seeds[0].source // ""),
        first_seed_value: (.context_pack.selected_seeds[0].value // ""),
        companion_entrypoint: (([.context_pack.selected_seeds[1:][]? | select(.source == "overview_entrypoint") | .value] | first) // ""),
        first_file: (.context_pack.files[0].file // ""),
        first_reading_question: (.context_pack.reading_plan[0].question // ""),
        first_next_action: (.context_pack.reading_plan[0].next_action // ""),
        first_suggested_tool: (.execution_plan[1].suggested_tool.tool // ""),
        continuation_status: (.context_pack.continuation_summary.status // ""),
        risk_level: (.impact_analysis.risk_level // "not_available"),
        impacted_files: (.impact_analysis.impact_counts.impacted_files // 0),
        suggested_checks: (.impact_analysis.suggested_checks | length)
      },
      artifacts: {
        markdown: $markdown,
        raw_agent_route_json: $raw_agent_route_json
      }
    }' "$route_json" >"$target"

  jq -e \
    '.status == "pass"
      and .route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and (.metrics.total_lines | type == "number")
      and (.metrics.selected_lines | type == "number")
      and (.metrics.line_reduction | type == "string" and length > 0)
      and (.metrics.selected_seed_count | type == "number")
      and (.metrics.first_seed_source | type == "string")
      and (.metrics.companion_entrypoint | type == "string")
      and (.metrics.first_file | type == "string" and length > 0)
      and (.metrics.first_reading_question | type == "string" and length > 0)
      and (.metrics.risk_level | type == "string" and length > 0)' \
    "$target" >/dev/null ||
    fail "summary JSON does not match the local evidence contract"
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

  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local route_json markdown_file args
  route_json="$TEMP_DIR/agent-route.json"
  markdown_file="${OUTPUT_FILE:-$TEMP_DIR/evidence.md}"

  args=(
    "agent-route"
    "$REPO_ROOT"
    "--task"
    "$TASK"
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

  jq -e \
    '(.route | map(.tool)) == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and (.context_pack.files | length) > 0
      and (.context_pack.reading_plan | length) > 0
      and (.execution_plan | length) >= 4' \
    "$route_json" >/dev/null ||
    fail "agent_route did not return the expected first-read evidence contract"

  if [ -n "$JSON_FILE" ]; then
    mkdir -p "$(dirname "$JSON_FILE")"
    cp "$route_json" "$JSON_FILE"
  fi

  if [ -n "$OUTPUT_FILE" ]; then
    mkdir -p "$(dirname "$OUTPUT_FILE")"
  fi
  write_markdown "$route_json" "$markdown_file"

  if [ -n "$SUMMARY_JSON" ]; then
    write_summary_json "$route_json" "$SUMMARY_JSON"
  fi

  if [ -n "$OUTPUT_FILE" ]; then
    echo "local repo evidence written to $OUTPUT_FILE"
    if [ -n "$JSON_FILE" ]; then
      echo "raw agent_route JSON written to $JSON_FILE"
    fi
    if [ -n "$SUMMARY_JSON" ]; then
      echo "local repo evidence summary JSON written to $SUMMARY_JSON"
    fi
  else
    cat "$markdown_file"
    if [ -n "$SUMMARY_JSON" ]; then
      echo
      echo "local repo evidence summary JSON written to $SUMMARY_JSON"
    fi
  fi
}

main "$@"
