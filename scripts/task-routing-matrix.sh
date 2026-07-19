#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${CODEINSIGHT_TASK_MATRIX_ROOT:-}"
TOKEN_BUDGET="${CODEINSIGHT_TASK_MATRIX_TOKEN_BUDGET:-6000}"
OUTPUT_DIR="${CODEINSIGHT_TASK_MATRIX_OUTPUT_DIR:-}"
OUTPUT_FILE="${CODEINSIGHT_TASK_MATRIX_OUTPUT:-}"
SUMMARY_JSON="${CODEINSIGHT_TASK_MATRIX_SUMMARY_JSON:-}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
LOCAL_EVIDENCE_SCRIPT="${CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT:-$ROOT_DIR/scripts/local-repo-evidence.sh}"
FORCE_INDEX="${CODEINSIGHT_TASK_MATRIX_FORCE_INDEX:-1}"
TASKS=()
EXPECTATIONS=()
EXPECTATION_FILES=()

usage() {
  cat <<'EOF'
usage: scripts/task-routing-matrix.sh [REPO_ROOT] [options]

Runs local first-read evidence for multiple task prompts against one repository
and writes a compact routing-quality matrix.

Options:
  --root PATH           Repository root. Also accepted as the first argument.
  --task TEXT           Add one task prompt. Can be repeated.
  --token-budget N      Token budget for each route. Default: 6000.
  --output-dir PATH     Output directory. Default: /tmp/codeinsight-task-routing-matrix.
  --output PATH         Markdown matrix path. Default: <output-dir>/task-routing-matrix.md.
  --summary-json PATH   Machine-readable summary path. Default: <output-dir>/summary.json.
  --expect TASK=FILE    Assert that TASK selects FILE as the first file. Can be repeated.
  --expect-file PATH    Read expectations from PATH and add those tasks to the matrix.
                        Supports JSON array objects or line-based TASK=FILE / TSV.
  --bin PATH            Use a specific codeinsight binary.
  --no-force-index      Reuse the existing index for the first task too.
  -h, --help            Show this help text.

Environment:
  CODEINSIGHT_TASK_MATRIX_ROOT
  CODEINSIGHT_TASK_MATRIX_TOKEN_BUDGET
  CODEINSIGHT_TASK_MATRIX_OUTPUT_DIR
  CODEINSIGHT_TASK_MATRIX_OUTPUT
  CODEINSIGHT_TASK_MATRIX_SUMMARY_JSON
  CODEINSIGHT_TASK_MATRIX_FORCE_INDEX
  CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "task routing matrix failed: $*" >&2
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
        add_task "$2"
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
      --expect)
        [ "$#" -ge 2 ] || fail "--expect requires TASK=FILE"
        add_expectation "$2"
        shift 2
        ;;
      --expect-file)
        [ "$#" -ge 2 ] || fail "--expect-file requires a path"
        EXPECTATION_FILES+=("$2")
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

add_task() {
  local task="$1"

  if [ -z "$task" ]; then
    fail "task must not be empty"
  fi
  local existing
  if [ "${#TASKS[@]}" -gt 0 ]; then
    for existing in "${TASKS[@]}"; do
      if [ "$existing" = "$task" ]; then
        return
      fi
    done
  fi
  TASKS+=("$task")
}

add_expectation() {
  local expectation="$1"
  local task expected

  case "$expectation" in
    *=*)
      task="${expectation%%=*}"
      expected="${expectation#*=}"
      ;;
    *)
      fail "expectation must use TASK=FILE: $expectation"
      ;;
  esac
  if [ -z "$task" ] || [ -z "$expected" ]; then
    fail "expectation must include non-empty task and file: $expectation"
  fi
  add_task "$task"
  EXPECTATIONS+=("$task=$expected")
}

load_expectation_file() {
  local file="$1"

  if [ ! -f "$file" ]; then
    fail "expectation file does not exist: $file"
  fi

  case "$file" in
    *.json)
      jq -e '
        type == "array"
        and all(.[]; (.task | type == "string" and length > 0)
          and (((.expected_first_file // .first_file) | type == "string") and ((.expected_first_file // .first_file) | length > 0)))
      ' "$file" >/dev/null ||
        fail "JSON expectation file must be an array of objects with task and expected_first_file or first_file: $file"
      while IFS= read -r expectation; do
        add_expectation "$expectation"
      done < <(jq -r '.[] | .task + "=" + (.expected_first_file // .first_file)' "$file")
      ;;
    *)
      local line task expected
      while IFS= read -r line || [ -n "$line" ]; do
        case "$line" in
          ''|'#'*) continue ;;
        esac
        if [[ "$line" == *$'\t'* ]]; then
          task="${line%%$'\t'*}"
          expected="${line#*$'\t'}"
          add_expectation "$task=$expected"
        else
          add_expectation "$line"
        fi
      done <"$file"
      ;;
  esac
}

load_expectation_files() {
  local file
  if [ "${#EXPECTATION_FILES[@]}" -gt 0 ]; then
    for file in "${EXPECTATION_FILES[@]}"; do
      load_expectation_file "$file"
    done
  fi
}

slugify_task() {
  printf "%s" "$1" \
    | tr '[:upper:]' '[:lower:]' \
    | sed -E 's/[^a-z0-9]+/-/g; s/^-+//; s/-+$//; s/^$/task/'
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

run_task() {
  local index="$1"
  local task="$2"
  local slug task_dir summary_json markdown_json raw_json evidence_markdown

  slug="$(slugify_task "$task")"
  task_dir="$OUTPUT_DIR/tasks/$index-$slug"
  summary_json="$task_dir/summary.json"
  raw_json="$task_dir/agent-route.json"
  evidence_markdown="$task_dir/local-repo-evidence.md"
  mkdir -p "$task_dir"

  if [ "$index" -gt 1 ] || [ "$FORCE_INDEX" = "0" ]; then
    "$LOCAL_EVIDENCE_SCRIPT" "$REPO_ROOT" \
      --task "$task" \
      --token-budget "$TOKEN_BUDGET" \
      --output "$evidence_markdown" \
      --json "$raw_json" \
      --summary-json "$summary_json" \
      --bin "$CODEINSIGHT_BIN" \
      --no-force-index >&2
  else
    "$LOCAL_EVIDENCE_SCRIPT" "$REPO_ROOT" \
      --task "$task" \
      --token-budget "$TOKEN_BUDGET" \
      --output "$evidence_markdown" \
      --json "$raw_json" \
      --summary-json "$summary_json" \
      --bin "$CODEINSIGHT_BIN" >&2
  fi

  markdown_json="$task_dir/row.json"
  jq \
    --arg slug "$slug" \
    --arg task "$task" \
    --arg summary_json "$summary_json" \
    --arg raw_agent_route_json "$raw_json" \
    --arg evidence_markdown "$evidence_markdown" \
    '{
      slug: $slug,
      task: $task,
      seed_strategy: .metrics.seed_strategy,
      first_file: .metrics.first_file,
      first_seed_value: .metrics.first_seed_value,
      companion_entrypoint: .metrics.companion_entrypoint,
      selected_lines: .metrics.selected_lines,
      total_lines: .metrics.total_lines,
      line_reduction: .metrics.line_reduction,
      selected_files: .metrics.selected_files,
      estimated_tokens: .metrics.estimated_tokens,
      first_reading_focus: .metrics.first_reading_focus,
      first_reading_question: .metrics.first_reading_question,
      first_selection_rank: .metrics.first_selection_rank,
      first_selection_reason: .metrics.first_selection_reason,
      continuation_next_action: .metrics.continuation_next_action,
      risk_level: .metrics.risk_level,
      impacted_files: .metrics.impacted_files,
      artifacts: {
        summary_json: $summary_json,
        raw_agent_route_json: $raw_agent_route_json,
        evidence_markdown: $evidence_markdown
      }
    }' "$summary_json" >"$markdown_json"
  printf "%s\n" "$markdown_json"
}

write_summary() {
  local rows_file="$1"

  jq -s \
    --arg repository "$REPO_ROOT" \
    --arg output "$OUTPUT_FILE" \
    --arg output_dir "$OUTPUT_DIR" \
    --argjson token_budget "$TOKEN_BUDGET" \
    '{
      status: "pass",
      repository: $repository,
      token_budget: $token_budget,
      task_count: length,
      output: $output,
      output_dir: $output_dir,
      tasks: .,
      aggregate: {
        total_selected_lines: (map(.selected_lines) | add // 0),
        average_selected_lines: ((map(.selected_lines) | add // 0) / (length | if . == 0 then 1 else . end)),
        total_estimated_tokens: (map(.estimated_tokens) | add // 0),
        max_impacted_files: (map(.impacted_files) | max // 0)
      }
    }' $(cat "$rows_file") >"$SUMMARY_JSON"

  jq -e \
    '.status == "pass"
      and (.task_count | type == "number" and . > 0)
      and all(.tasks[]; (.task | type == "string" and length > 0)
        and (.first_file | type == "string" and length > 0)
        and (.first_reading_focus | type == "string" and length > 0)
        and (.first_reading_question | type == "string" and length > 0)
        and (.seed_strategy | type == "string" and length > 0)
        and (.line_reduction | type == "string" and length > 0))' \
    "$SUMMARY_JSON" >/dev/null ||
    fail "summary JSON does not match the task routing matrix contract"
}

validate_expectations() {
  if [ "${#EXPECTATIONS[@]}" -eq 0 ]; then
    return
  fi

  local expectations_jsonl expectations_json expectation task expected actual status tmp failures
  expectations_jsonl="$OUTPUT_DIR/expectations.jsonl"
  expectations_json="$OUTPUT_DIR/expectations.json"
  : >"$expectations_jsonl"

  for expectation in "${EXPECTATIONS[@]}"; do
    case "$expectation" in
      *=*)
        task="${expectation%%=*}"
        expected="${expectation#*=}"
        ;;
      *)
        fail "--expect must use TASK=FILE: $expectation"
        ;;
    esac
    if [ -z "$task" ] || [ -z "$expected" ]; then
      fail "--expect must include non-empty task and file: $expectation"
    fi

    actual="$(jq -r --arg task "$task" \
      '.tasks[] | select(.task == $task) | .first_file' "$SUMMARY_JSON")"
    if [ -z "$actual" ]; then
      fail "--expect task was not run: $task"
    fi
    if [ "$actual" = "$expected" ]; then
      status="pass"
    else
      status="fail"
    fi
    jq -n \
      --arg task "$task" \
      --arg expected_first_file "$expected" \
      --arg actual_first_file "$actual" \
      --arg status "$status" \
      '{
        task: $task,
        expected_first_file: $expected_first_file,
        actual_first_file: $actual_first_file,
        status: $status
      }' >>"$expectations_jsonl"
  done

  jq -s '.' "$expectations_jsonl" >"$expectations_json"
  tmp="$SUMMARY_JSON.tmp"
  jq --slurpfile checks "$expectations_json" \
    '. + {
      expectations: {
        status: (if ($checks[0] | all(.status == "pass")) then "pass" else "fail" end),
        count: ($checks[0] | length),
        checks: $checks[0]
      }
    }' "$SUMMARY_JSON" >"$tmp"
  mv "$tmp" "$SUMMARY_JSON"

  failures="$(jq -r '.expectations.checks[] | select(.status == "fail") |
    "- " + .task + ": expected `" + .expected_first_file + "`, got `" + .actual_first_file + "`"' \
    "$SUMMARY_JSON")"
  if [ -n "$failures" ]; then
    printf "task routing matrix expectation failures:\n%s\n" "$failures" >&2
    fail "one or more route expectations failed"
  fi
}

write_markdown() {
  {
    echo "# CodeInsight Task Routing Matrix"
    echo
    echo "- Repository: \`$REPO_ROOT\`"
    echo "- Token budget: \`$TOKEN_BUDGET\`"
    echo "- Tasks: \`$(json_value "$SUMMARY_JSON" '.task_count')\`"
    echo "- Summary JSON: \`$SUMMARY_JSON\`"
    echo
    echo "## Results"
    echo
    echo "| Task | Seed strategy | First file | Focus | Question | First seed | Companion | Lines | Reduction | Tokens | Impact |"
    echo "| --- | --- | --- | --- | --- | --- | --- | ---: | ---: | ---: | ---: |"
    jq -r '.tasks[] |
      "| \(.task) | `\(.seed_strategy)` | `\(.first_file)` | \(.first_reading_focus) | \(.first_reading_question) | `\(.first_seed_value)` | `\((.companion_entrypoint // "") as $value | if $value == "" then "-" else $value end)` | `\(.selected_lines)/\(.total_lines)` | `\(.line_reduction)` | `\(.estimated_tokens)` | `\(.risk_level) / \(.impacted_files)` |"' \
      "$SUMMARY_JSON"
    echo
    echo "## Read Order Evidence"
    echo
    jq -r '.tasks[] |
      "- `\(.task)`: read `\(.first_file)` first (rank \(.first_selection_rank)); \(.first_selection_reason)"' \
      "$SUMMARY_JSON"
    if jq -e '.expectations? | type == "object"' "$SUMMARY_JSON" >/dev/null; then
      echo
      echo "## Expectations"
      echo
      echo "| Task | Expected first file | Actual first file | Status |"
      echo "| --- | --- | --- | --- |"
      jq -r '.expectations.checks[] |
        "| \(.task) | `\(.expected_first_file)` | `\(.actual_first_file)` | `\(.status)` |"' \
        "$SUMMARY_JSON"
    fi
    echo
    echo "## Artifacts"
    echo
    jq -r '.tasks[] |
      "- `\(.task)`: [local evidence](\(.artifacts.evidence_markdown)), raw JSON `\(.artifacts.raw_agent_route_json)`"' \
      "$SUMMARY_JSON"
  } >"$OUTPUT_FILE"
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

  load_expectation_files

  if [ "${#TASKS[@]}" -eq 0 ]; then
    TASKS=(
      "understand routing behavior"
      "understand authentication behavior"
      "understand authorization permissions"
      "understand application settings"
      "understand startup flow"
      "understand persistence behavior"
      "debug retry timeout handling"
      "find regression coverage"
      "understand api handler behavior"
      "understand cache performance latency"
      "understand observability telemetry logs"
      "understand security sanitization vulnerabilities"
      "understand checkout subscription payment"
      "understand frontend component rendering"
      "understand background job queue"
      "understand documentation usage"
      "understand request lifecycle before after request handling"
      "understand middleware behavior"
    )
  fi

  OUTPUT_DIR="${OUTPUT_DIR:-/tmp/codeinsight-task-routing-matrix}"
  OUTPUT_FILE="${OUTPUT_FILE:-$OUTPUT_DIR/task-routing-matrix.md}"
  SUMMARY_JSON="${SUMMARY_JSON:-$OUTPUT_DIR/summary.json}"
  rm -rf "$OUTPUT_DIR"
  mkdir -p "$OUTPUT_DIR"

  build_binary_if_needed

  local rows_file index row_file
  rows_file="$OUTPUT_DIR/rows.txt"
  : >"$rows_file"
  index=1
  for task in "${TASKS[@]}"; do
    row_file="$(run_task "$index" "$task")"
    printf "%s\n" "$row_file" >>"$rows_file"
    index=$((index + 1))
  done

  write_summary "$rows_file"
  validate_expectations
  write_markdown

  echo "task routing matrix written to $OUTPUT_FILE"
  echo "summary: $SUMMARY_JSON"
  echo "tasks: ${#TASKS[@]}"
}

main "$@"
