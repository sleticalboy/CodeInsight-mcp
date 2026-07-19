#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${CODEINSIGHT_ADOPTION_COMPARE_ROOT:-}"
TASK="${CODEINSIGHT_ADOPTION_COMPARE_TASK:-understand the main application entrypoint}"
TOKEN_BUDGET="${CODEINSIGHT_ADOPTION_COMPARE_TOKEN_BUDGET:-6000}"
OUTPUT_DIR="${CODEINSIGHT_ADOPTION_COMPARE_OUTPUT_DIR:-}"
OUTPUT_FILE="${CODEINSIGHT_ADOPTION_COMPARE_OUTPUT:-}"
SUMMARY_JSON="${CODEINSIGHT_ADOPTION_COMPARE_SUMMARY_JSON:-}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
FORCE_INDEX="${CODEINSIGHT_ADOPTION_COMPARE_FORCE_INDEX:-1}"
LOCAL_EVIDENCE_SCRIPT="${CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT:-$ROOT_DIR/scripts/local-repo-evidence.sh}"

usage() {
  cat <<'EOF'
usage: scripts/adoption-comparison.sh [REPO_ROOT] [options]

Builds a copyable adoption comparison that contrasts a blind first read of the
repository with CodeInsight's routed first-read context.

Options:
  --root PATH           Repository root. Also accepted as the first argument.
  --task TEXT           Task for agent_route.
  --token-budget N      Token budget for context routing. Default: 6000.
  --output-dir PATH     Output directory. Default: /tmp/codeinsight-adoption-comparison.
  --output PATH         Markdown comparison path.
  --summary-json PATH   Machine-readable comparison summary path.
  --bin PATH            Use a specific codeinsight binary.
  --no-force-index      Reuse the existing index when available.
  -h, --help            Show this help text.

Environment:
  CODEINSIGHT_ADOPTION_COMPARE_ROOT
  CODEINSIGHT_ADOPTION_COMPARE_TASK
  CODEINSIGHT_ADOPTION_COMPARE_TOKEN_BUDGET
  CODEINSIGHT_ADOPTION_COMPARE_OUTPUT_DIR
  CODEINSIGHT_ADOPTION_COMPARE_OUTPUT
  CODEINSIGHT_ADOPTION_COMPARE_SUMMARY_JSON
  CODEINSIGHT_ADOPTION_COMPARE_FORCE_INDEX
  CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "adoption comparison failed: $*" >&2
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
      --output)
        [ "$#" -ge 2 ] || fail "--output requires a path"
        OUTPUT_FILE="$2"
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

read_less_ratio() {
  local baseline_lines="$1"
  local routed_lines="$2"

  awk -v baseline="$baseline_lines" -v routed="$routed_lines" 'BEGIN {
    if (baseline <= 0 || routed <= 0) {
      printf "n/a"
    } else {
      printf "%.1fx", baseline / routed
    }
  }'
}

write_markdown() {
  local local_summary="$1"
  local target="$2"
  local baseline_lines routed_lines saved_lines ratio

  baseline_lines="$(json_value "$local_summary" '.metrics.total_lines')"
  routed_lines="$(json_value "$local_summary" '.metrics.selected_lines')"
  saved_lines=$((baseline_lines - routed_lines))
  if [ "$saved_lines" -lt 0 ]; then
    saved_lines=0
  fi
  ratio="$(read_less_ratio "$baseline_lines" "$routed_lines")"

  {
    echo "# CodeInsight Adoption Comparison"
    echo
    echo "- Repository: \`${REPO_ROOT}\`"
    echo "- Task: \`${TASK}\`"
    echo "- Token budget: \`${TOKEN_BUDGET}\`"
    echo "- Route: \`$(json_value "$local_summary" '.route_tools | join(" -> ")')\`"
    echo
    echo "## Key Results"
    echo
    echo "- Blind first-read baseline: \`${baseline_lines}\` source lines"
    echo "- CodeInsight routed first-read: \`${routed_lines}/${baseline_lines}\` source lines"
    echo "- Source lines avoided: \`${saved_lines}\`"
    echo "- First-read reduction: \`$(json_value "$local_summary" '.metrics.line_reduction')\`"
    echo "- Read less: \`${ratio}\`"
    echo "- Selected files: \`$(json_value "$local_summary" '.metrics.selected_files')\`"
    echo "- Selected ranges: \`$(json_value "$local_summary" '.metrics.selected_ranges')\`"
    echo "- Estimated tokens: \`$(json_value "$local_summary" '.metrics.estimated_tokens')\`"
    echo "- Seed strategy: \`$(json_value "$local_summary" '.metrics.seed_strategy // "-"')\`"
    echo "- First seed source: \`$(json_value "$local_summary" '.metrics.first_seed_source // "-"')\`"
    echo "- Companion entrypoint: \`$(json_value "$local_summary" '(.metrics.companion_entrypoint // "") as $value | if $value == "" then "-" else $value end')\`"
    echo "- First selected file: \`$(json_value "$local_summary" '.metrics.first_file')\`"
    echo "- First reading focus: $(json_value "$local_summary" '.metrics.first_reading_focus')"
    echo "- First reading question: $(json_value "$local_summary" '.metrics.first_reading_question')"
    echo "- First selection rank: \`$(json_value "$local_summary" '.metrics.first_selection_rank // "-"')\`"
    echo "- First selection reason: $(json_value "$local_summary" '.metrics.first_selection_reason // "-"')"
    echo "- First suggested tool: \`$(json_value "$local_summary" '.metrics.first_suggested_tool')\`"
    echo "- Continuation status: \`$(json_value "$local_summary" '.metrics.continuation_status // "-"')\`"
    echo "- Continuation next action: \`$(json_value "$local_summary" '.metrics.continuation_next_action // "-"')\`"
    if [ -n "$(json_value "$local_summary" '.metrics.first_omitted_file // ""')" ]; then
      echo "- First omitted candidate: \`$(json_value "$local_summary" '.metrics.first_omitted_file')\` (candidate rank $(json_value "$local_summary" '.metrics.first_omitted_selection_rank // "-"'))"
      echo "- First omitted reason: $(json_value "$local_summary" '.metrics.first_omitted_omission_reason // "-"')"
      echo "- First omitted next action: \`$(json_value "$local_summary" '.metrics.first_omitted_next_action // "-"')\`"
    else
      echo "- First omitted candidate: none"
    fi
    echo "- Impact risk: \`$(json_value "$local_summary" '.metrics.risk_level')\`"
    echo "- Impacted files: \`$(json_value "$local_summary" '.metrics.impacted_files')\`"
    echo
    echo "## Interpretation"
    echo
    echo "Use the routed first-read count as the context an AI agent should inspect before broad file reading. The blind baseline is the repository source-line total from \`project_overview\`; the delta is the amount of source text avoided for the first pass."
    echo
    echo "## Artifacts"
    echo
    echo "- Local evidence Markdown: \`${OUTPUT_DIR}/local-repo-evidence.md\`"
    echo "- Local evidence JSON: \`${OUTPUT_DIR}/local-repo-evidence.json\`"
    echo "- Raw agent route JSON: \`${OUTPUT_DIR}/agent-route.json\`"
  } >"$target"
}

write_summary_json() {
  local local_summary="$1"
  local target="$2"
  local baseline_lines routed_lines saved_lines ratio

  baseline_lines="$(json_value "$local_summary" '.metrics.total_lines')"
  routed_lines="$(json_value "$local_summary" '.metrics.selected_lines')"
  saved_lines=$((baseline_lines - routed_lines))
  if [ "$saved_lines" -lt 0 ]; then
    saved_lines=0
  fi
  ratio="$(read_less_ratio "$baseline_lines" "$routed_lines")"

  jq \
    --arg repository "$REPO_ROOT" \
    --arg task "$TASK" \
    --arg output "$OUTPUT_FILE" \
    --arg local_markdown "$OUTPUT_DIR/local-repo-evidence.md" \
    --arg local_summary "$local_summary" \
    --arg raw_agent_route_json "$OUTPUT_DIR/agent-route.json" \
    --argjson baseline_lines "$baseline_lines" \
    --argjson routed_lines "$routed_lines" \
    --argjson saved_lines "$saved_lines" \
    --arg read_less_ratio "$ratio" \
    '{
      status: "pass",
      repository: $repository,
      task: $task,
      route_tools,
      metrics: {
        blind_first_read_lines: $baseline_lines,
        routed_first_read_lines: $routed_lines,
        source_lines_avoided: $saved_lines,
        line_reduction: .metrics.line_reduction,
        read_less_ratio: $read_less_ratio,
        selected_files: .metrics.selected_files,
        selected_ranges: .metrics.selected_ranges,
        estimated_tokens: .metrics.estimated_tokens,
        seed_strategy: .metrics.seed_strategy,
        selected_seed_count: .metrics.selected_seed_count,
        first_seed_source: .metrics.first_seed_source,
        first_seed_value: .metrics.first_seed_value,
        companion_entrypoint: .metrics.companion_entrypoint,
        first_file: .metrics.first_file,
        first_reading_focus: .metrics.first_reading_focus,
        first_reading_question: .metrics.first_reading_question,
        first_selection_rank: .metrics.first_selection_rank,
        first_selection_reason: .metrics.first_selection_reason,
        first_suggested_tool: .metrics.first_suggested_tool,
        continuation_status: .metrics.continuation_status,
        continuation_next_action: .metrics.continuation_next_action,
        first_omitted_file: .metrics.first_omitted_file,
        first_omitted_selection_rank: .metrics.first_omitted_selection_rank,
        first_omitted_omission_reason: .metrics.first_omitted_omission_reason,
        first_omitted_next_action: .metrics.first_omitted_next_action,
        risk_level: .metrics.risk_level,
        impacted_files: .metrics.impacted_files
      },
      artifacts: {
        markdown: $output,
        local_evidence_markdown: $local_markdown,
        local_evidence_summary: $local_summary,
        raw_agent_route_json: $raw_agent_route_json
      }
    }' "$local_summary" >"$target"

  jq -e \
    '.status == "pass"
      and (.metrics.blind_first_read_lines | type == "number")
      and (.metrics.routed_first_read_lines | type == "number")
      and (.metrics.source_lines_avoided | type == "number")
      and (.metrics.line_reduction | type == "string" and length > 0)
      and (.metrics.read_less_ratio | type == "string" and length > 0)
      and (.metrics.first_file | type == "string" and length > 0)
      and (.metrics.first_selection_rank | type == "number" and . >= 1)
      and (.metrics.first_selection_reason | type == "string" and length > 0)
      and (.metrics.continuation_status | type == "string" and length > 0)
      and (.metrics.continuation_next_action | type == "string" and length > 0)
      and (.artifacts.local_evidence_summary | type == "string" and length > 0)' \
    "$target" >/dev/null ||
    fail "summary JSON does not match the adoption comparison contract"
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
  if [ ! -x "$LOCAL_EVIDENCE_SCRIPT" ]; then
    fail "local evidence script is not executable: $LOCAL_EVIDENCE_SCRIPT"
  fi
  case "$TOKEN_BUDGET" in
    ''|*[!0-9]*)
      fail "--token-budget must be a positive integer"
      ;;
  esac
  if [ "$TOKEN_BUDGET" -le 0 ]; then
    fail "--token-budget must be greater than zero"
  fi

  REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
  OUTPUT_DIR="${OUTPUT_DIR:-/tmp/codeinsight-adoption-comparison}"
  OUTPUT_FILE="${OUTPUT_FILE:-$OUTPUT_DIR/adoption-comparison.md}"
  SUMMARY_JSON="${SUMMARY_JSON:-$OUTPUT_DIR/summary.json}"
  mkdir -p "$OUTPUT_DIR" "$(dirname "$OUTPUT_FILE")" "$(dirname "$SUMMARY_JSON")"

  local local_args
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

  "$LOCAL_EVIDENCE_SCRIPT" "${local_args[@]}" \
    >"$OUTPUT_DIR/local-repo-evidence.out" \
    2>"$OUTPUT_DIR/local-repo-evidence.err"

  jq -e \
    '.status == "pass"
      and (.metrics.total_lines | type == "number")
      and (.metrics.selected_lines | type == "number")
      and (.metrics.first_file | type == "string" and length > 0)' \
    "$OUTPUT_DIR/local-repo-evidence.json" >/dev/null ||
    fail "local evidence summary does not contain comparison metrics"

  write_markdown "$OUTPUT_DIR/local-repo-evidence.json" "$OUTPUT_FILE"
  write_summary_json "$OUTPUT_DIR/local-repo-evidence.json" "$SUMMARY_JSON"

  echo "adoption comparison written to $OUTPUT_FILE"
  echo "summary: $SUMMARY_JSON"
  echo "local_evidence: $OUTPUT_DIR/local-repo-evidence.md"
  echo "raw_agent_route_json: $OUTPUT_DIR/agent-route.json"
}

main "$@"
