#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${CODEINSIGHT_PUBLIC_TASK_MATRIX_WORK_DIR:-/tmp/codeinsight-public-task-routing-matrix}"
OUTPUT_DIR="${CODEINSIGHT_PUBLIC_TASK_MATRIX_OUTPUT_DIR:-}"
OUTPUT_FILE="${CODEINSIGHT_PUBLIC_TASK_MATRIX_OUTPUT:-}"
SUMMARY_JSON="${CODEINSIGHT_PUBLIC_TASK_MATRIX_SUMMARY_JSON:-}"
TOKEN_BUDGET="${CODEINSIGHT_PUBLIC_TASK_MATRIX_TOKEN_BUDGET:-6000}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
MATRIX_SCRIPT="${CODEINSIGHT_TASK_ROUTING_MATRIX_SCRIPT:-$ROOT_DIR/scripts/task-routing-matrix.sh}"
FORCE_CLONE="${CODEINSIGHT_PUBLIC_TASK_MATRIX_FORCE_CLONE:-0}"
MIN_ROUTE_QUALITY_SCORE="${CODEINSIGHT_PUBLIC_TASK_MATRIX_MIN_ROUTE_QUALITY_SCORE:-}"
CASES=()
ROOT_OVERRIDES=()
EXPECTATION_OVERRIDES=()
REF_OVERRIDES=()

usage() {
  cat <<'EOF'
usage: scripts/public-task-routing-matrix.sh [options]

Runs checked-in task-routing expectation files against public repository cases
and writes one aggregate route-quality summary.

Options:
  --case NAME          Run one case. Can be repeated. Defaults to pinned fast cases.
                       Supported: django, express, fastapi, flask, gin, requests, streamlit, wouter.
  --root NAME=PATH     Use an existing checkout for a case. Can be repeated.
  --ref NAME=REF       Checkout a specific ref for a case. Can be repeated.
  --expect-file NAME=PATH
                       Override the expectation TSV/JSON for a case.
  --work-dir PATH      Clone workspace. Default: /tmp/codeinsight-public-task-routing-matrix.
  --output-dir PATH    Output directory. Default: <work-dir>/matrix.
  --output PATH        Aggregate Markdown path. Default: <output-dir>/public-task-routing-matrix.md.
  --summary-json PATH  Aggregate JSON path. Default: <output-dir>/summary.json.
  --token-budget N     Token budget per route. Default: 6000.
  --min-route-quality-score N
                       Fail when any case route reports route_quality_score below N.
  --bin PATH           Use a specific codeinsight binary.
  --force-clone        Reclone public repositories even when the clone exists.
  -h, --help           Show this help text.

Environment:
  CODEINSIGHT_PUBLIC_TASK_MATRIX_WORK_DIR
  CODEINSIGHT_PUBLIC_TASK_MATRIX_OUTPUT_DIR
  CODEINSIGHT_PUBLIC_TASK_MATRIX_OUTPUT
  CODEINSIGHT_PUBLIC_TASK_MATRIX_SUMMARY_JSON
  CODEINSIGHT_PUBLIC_TASK_MATRIX_TOKEN_BUDGET
  CODEINSIGHT_PUBLIC_TASK_MATRIX_FORCE_CLONE
  CODEINSIGHT_PUBLIC_TASK_MATRIX_MIN_ROUTE_QUALITY_SCORE
  CODEINSIGHT_TASK_ROUTING_MATRIX_SCRIPT
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "public task routing matrix failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

add_case() {
  local case_name="$1"

  validate_case "$case_name"
  local existing
  for existing in ${CASES[@]+"${CASES[@]}"}; do
    if [ "$existing" = "$case_name" ]; then
      return
    fi
  done
  CASES+=("$case_name")
}

validate_case() {
  case "$1" in
    django|express|fastapi|flask|gin|requests|streamlit|wouter) ;;
    *) fail "unsupported case: $1" ;;
  esac
}

parse_mapping() {
  local label="$1"
  local value="$2"
  local name path

  case "$value" in
    *=*)
      name="${value%%=*}"
      path="${value#*=}"
      ;;
    *)
      fail "$label must use NAME=PATH: $value"
      ;;
  esac
  validate_case "$name"
  if [ -z "$path" ]; then
    fail "$label path must not be empty: $value"
  fi
  printf "%s=%s\n" "$name" "$path"
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --case)
        [ "$#" -ge 2 ] || fail "--case requires a name"
        add_case "$2"
        shift 2
        ;;
      --root)
        [ "$#" -ge 2 ] || fail "--root requires NAME=PATH"
        ROOT_OVERRIDES+=("$(parse_mapping "--root" "$2")")
        shift 2
        ;;
      --ref)
        [ "$#" -ge 2 ] || fail "--ref requires NAME=REF"
        REF_OVERRIDES+=("$(parse_mapping "--ref" "$2")")
        shift 2
        ;;
      --expect-file)
        [ "$#" -ge 2 ] || fail "--expect-file requires NAME=PATH"
        EXPECTATION_OVERRIDES+=("$(parse_mapping "--expect-file" "$2")")
        shift 2
        ;;
      --work-dir)
        [ "$#" -ge 2 ] || fail "--work-dir requires a path"
        WORK_DIR="$2"
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
      --token-budget)
        [ "$#" -ge 2 ] || fail "--token-budget requires a number"
        TOKEN_BUDGET="$2"
        shift 2
        ;;
      --min-route-quality-score)
        [ "$#" -ge 2 ] || fail "--min-route-quality-score requires a number"
        MIN_ROUTE_QUALITY_SCORE="$2"
        shift 2
        ;;
      --bin)
        [ "$#" -ge 2 ] || fail "--bin requires a path"
        CODEINSIGHT_BIN="$2"
        shift 2
        ;;
      --force-clone)
        FORCE_CLONE="1"
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
        fail "unexpected positional argument: $1"
        ;;
    esac
  done
}

case_repo_url() {
  case "$1" in
    express) printf "https://github.com/expressjs/express.git" ;;
    django) printf "https://github.com/django/django.git" ;;
    fastapi) printf "https://github.com/fastapi/fastapi.git" ;;
    flask) printf "https://github.com/pallets/flask.git" ;;
    gin) printf "https://github.com/gin-gonic/gin.git" ;;
    requests) printf "https://github.com/psf/requests.git" ;;
    streamlit) printf "https://github.com/streamlit/streamlit.git" ;;
    wouter) printf "https://github.com/molefrog/wouter.git" ;;
    *) fail "unsupported case: $1" ;;
  esac
}

case_default_ref() {
  case "$1" in
    express) printf "ae6dd37680e3a00618d6c8a3e522f0ee4eeba1a4" ;;
    django) printf "dca76b15c62a1118325b71678ce3235e2231198d" ;;
    fastapi) printf "7d210a4a9f54f2744e50ce55c65eb852958478c5" ;;
    flask) printf "36e4a824f340fdee7ed50937ba8e7f6bc7d17f81" ;;
    gin) printf "34dac209ffb6ef85cc78c5d217bbb7ad001d68fd" ;;
    requests) printf "f361ead047be5cb873174218582f7d8b9fcd9f49" ;;
    streamlit) printf "a45904234a8f438f615ac7f11b0a96ce07e06721" ;;
    wouter) printf "e74a8095601d028234e4bbd2dc9ef0849f5cea8f" ;;
    *) fail "unsupported case: $1" ;;
  esac
}

case_ref() {
  local case_name="$1"
  local override name ref

  for override in ${REF_OVERRIDES[@]+"${REF_OVERRIDES[@]}"}; do
    name="${override%%=*}"
    ref="${override#*=}"
    if [ "$name" = "$case_name" ]; then
      printf "%s" "$ref"
      return
    fi
  done

  case_default_ref "$case_name"
}

case_expect_file() {
  local case_name="$1"
  local override name path

  for override in ${EXPECTATION_OVERRIDES[@]+"${EXPECTATION_OVERRIDES[@]}"}; do
    name="${override%%=*}"
    path="${override#*=}"
    if [ "$name" = "$case_name" ]; then
      printf "%s" "$path"
      return
    fi
  done

  printf "%s/docs/task-routing-expectations/%s.tsv" "$ROOT_DIR" "$case_name"
}

case_root_override() {
  local case_name="$1"
  local override name path

  for override in ${ROOT_OVERRIDES[@]+"${ROOT_OVERRIDES[@]}"}; do
    name="${override%%=*}"
    path="${override#*=}"
    if [ "$name" = "$case_name" ]; then
      printf "%s" "$path"
      return
    fi
  done
}

prepare_case_repo() {
  local case_name="$1"
  local override repo_dir repo_url ref

  override="$(case_root_override "$case_name")"
  if [ -n "$override" ]; then
    [ -d "$override" ] || fail "case root does not exist for $case_name: $override"
    (cd "$override" && pwd)
    return
  fi

  repo_dir="$WORK_DIR/repos/$case_name"
  repo_url="$(case_repo_url "$case_name")"
  ref="$(case_ref "$case_name")"
  if [ "$FORCE_CLONE" = "1" ]; then
    rm -rf "$repo_dir"
  fi
  if [ ! -d "$repo_dir/.git" ]; then
    rm -rf "$repo_dir"
    mkdir -p "$(dirname "$repo_dir")"
    if [ -n "$ref" ]; then
      mkdir -p "$repo_dir"
      git -C "$repo_dir" init --quiet ||
        fail "failed to initialize repository for $case_name: $repo_dir"
      git -C "$repo_dir" remote add origin "$repo_url" ||
        fail "failed to add origin for $case_name: $repo_url"
    else
      git -c http.version=HTTP/1.1 clone --quiet --depth 1 "$repo_url" "$repo_dir" ||
        fail "failed to clone $case_name: $repo_url"
    fi
  fi
  if [ -n "$ref" ]; then
    git -C "$repo_dir" -c http.version=HTTP/1.1 fetch --quiet --depth 1 origin "$ref" ||
      fail "failed to fetch $case_name ref: $ref"
    git -C "$repo_dir" checkout --quiet FETCH_HEAD ||
      fail "failed to checkout $case_name ref: $ref"
  fi
  printf "%s" "$repo_dir"
}

run_case() {
  local case_name="$1"
  local repo_root expect_file case_ref_value case_output_dir case_summary row_json

  repo_root="$(prepare_case_repo "$case_name")" ||
    fail "failed to prepare case repository: $case_name"
  [ -d "$repo_root" ] || fail "case repository root does not exist for $case_name: $repo_root"
  expect_file="$(case_expect_file "$case_name")"
  case_ref_value="$(case_ref "$case_name")"
  [ -f "$expect_file" ] || fail "expectation file does not exist for $case_name: $expect_file"

  case_output_dir="$OUTPUT_DIR/$case_name"
  case_summary="$case_output_dir/summary.json"

  if ! "$MATRIX_SCRIPT" "$repo_root" \
    --expect-file "$expect_file" \
    --token-budget "$TOKEN_BUDGET" \
    --output-dir "$case_output_dir" \
    --summary-json "$case_summary" \
    ${MIN_ROUTE_QUALITY_SCORE:+--min-route-quality-score "$MIN_ROUTE_QUALITY_SCORE"} \
    ${CODEINSIGHT_BIN:+--bin "$CODEINSIGHT_BIN"} >&2; then
    fail "task routing matrix failed for case: $case_name"
  fi

  jq -e \
    '.status == "pass"
      and .expectations.status == "pass"
      and (.task_count | type == "number" and . > 0)' \
    "$case_summary" >/dev/null ||
    fail "case summary does not match the public matrix contract: $case_name"

  row_json="$case_output_dir/case-row.json"
  jq \
    --arg case_name "$case_name" \
    --arg repo_root "$repo_root" \
    --arg ref "$case_ref_value" \
    --arg expect_file "$expect_file" \
    --arg summary_json "$case_summary" \
    '{
      case: $case_name,
      repository: $repo_root,
      ref: $ref,
      expect_file: $expect_file,
      summary_json: $summary_json,
      task_count: .task_count,
      expectation_count: .expectations.count,
      total_task_source_lines: (.tasks | map(.total_lines) | add // 0),
      total_selected_lines: .aggregate.total_selected_lines,
      total_estimated_tokens: .aggregate.total_estimated_tokens,
      max_impacted_files: .aggregate.max_impacted_files,
      quality_gate: (.quality_gate // null),
      routes: [.tasks[] | {
        task,
        first_file,
        first_reading_focus,
        first_reading_question,
        first_next_action,
        first_suggested_tool,
        route_quality_level,
        route_quality_score,
        route_quality_evidence_count,
        route_quality_recommended_action,
        route_quality_decision_summary,
        route_quality_confidence_factors,
        route_quality_verification_steps,
        route_quality_warnings,
        seed_strategy,
        first_seed_value,
        total_lines,
        line_reduction,
        estimated_tokens,
        risk_level,
        impacted_files
      }]
    }' "$case_summary" >"$row_json"
  printf "%s\n" "$row_json"
}

write_summary() {
  local rows_file="$1"

  jq -s \
    --arg output "$OUTPUT_FILE" \
    --arg output_dir "$OUTPUT_DIR" \
    --argjson token_budget "$TOKEN_BUDGET" \
    --arg min_route_quality_score "$MIN_ROUTE_QUALITY_SCORE" \
    '{
      status: "pass",
      token_budget: $token_budget,
      output: $output,
      output_dir: $output_dir,
      case_count: length,
      cases: .,
      aggregate: {
        task_count: (map(.task_count) | add // 0),
        expectation_count: (map(.expectation_count) | add // 0),
        total_task_source_lines: (map(.total_task_source_lines) | add // 0),
        total_selected_lines: (map(.total_selected_lines) | add // 0),
        total_estimated_tokens: (map(.total_estimated_tokens) | add // 0),
        max_impacted_files: (map(.max_impacted_files) | max // 0),
        line_reduction: (
          (map(.total_task_source_lines) | add // 0) as $total |
          (map(.total_selected_lines) | add // 0) as $selected |
          if $total > 0 then (((($total - $selected) * 10000 / $total) | floor) / 100) else 0 end
        )
      }
    }
    + (if $min_route_quality_score != "" then {
      quality_gate: {
        min_route_quality_score: ($min_route_quality_score | tonumber),
        status: (if (map(.quality_gate.failure_count // 0) | add // 0) == 0 then "pass" else "fail" end),
        failure_count: (map(.quality_gate.failure_count // 0) | add // 0),
        cases: [.[] | select(.quality_gate != null) | {
          case,
          status: .quality_gate.status,
          failure_count: .quality_gate.failure_count
        }]
      }
    } else {} end)' $(cat "$rows_file") >"$SUMMARY_JSON"

  jq -e \
    '.status == "pass"
      and (.case_count | type == "number" and . > 0)
      and (.aggregate.task_count | type == "number" and . > 0)
      and (.aggregate.total_task_source_lines | type == "number" and . > 0)
      and (.aggregate.line_reduction | type == "number")
      and all(.cases[]; (.case | type == "string" and length > 0)
        and (.expectation_count | type == "number")
        and (.expectation_count == .task_count)
        and (.routes | type == "array" and length > 0)
        and all(.routes[]; (.first_suggested_tool | type == "string" and length > 0)
          and (.route_quality_level | type == "string" and length > 0)
          and (.route_quality_score | type == "number")
          and (.route_quality_evidence_count | type == "number")
          and (.route_quality_recommended_action | type == "string" and length > 0)
          and (.route_quality_decision_summary | type == "string" and length > 0)
          and (.route_quality_confidence_factors | type == "array")
          and (.route_quality_verification_steps | type == "array")
          and (.route_quality_warnings | type == "array")
          and (.first_seed_value | type == "string" and length > 0)))' \
    "$SUMMARY_JSON" >/dev/null ||
    fail "aggregate summary JSON does not match the public matrix contract"
}

write_markdown() {
  {
    echo "# CodeInsight Public Task Routing Matrix"
    echo
    echo "- Cases: \`$(jq -r '.case_count' "$SUMMARY_JSON")\`"
    echo "- Tasks: \`$(jq -r '.aggregate.task_count' "$SUMMARY_JSON")\`"
    echo "- Token budget: \`$TOKEN_BUDGET\`"
    if [ -n "$MIN_ROUTE_QUALITY_SCORE" ]; then
      echo "- Minimum route quality score: \`$MIN_ROUTE_QUALITY_SCORE\`"
    fi
    echo "- Aggregate line reduction: \`$(jq -r '.aggregate.line_reduction' "$SUMMARY_JSON")%\`"
    echo "- Summary JSON: \`$SUMMARY_JSON\`"
    echo
    echo "## Evidence Summary"
    echo
    write_evidence_summary markdown
    echo
    echo "## Cases"
    echo
    echo "| Case | Ref | Tasks | Expectations | Source lines | Selected lines | Reduction | Tokens | Max impact | Expect file |"
    echo "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |"
    jq -r '.cases[] |
      ((if .total_task_source_lines > 0 then (((.total_task_source_lines - .total_selected_lines) * 10000 / .total_task_source_lines | floor) / 100) else 0 end) as $reduction |
      "| \(.case) | `\((.ref // "") as $value | if $value == "" then "-" else $value end)` | `\(.task_count)` | `\(.expectation_count)` | `\(.total_task_source_lines)` | `\(.total_selected_lines)` | `\($reduction)%` | `\(.total_estimated_tokens)` | `\(.max_impacted_files)` | `\(.expect_file)` |")' \
      "$SUMMARY_JSON"
    echo
    echo "## Routes"
    echo
    jq -r '.cases[] as $case |
      "### " + $case.case + "\n\n" +
      "| Task | First file | Focus | Question | Suggested tool | Quality | Seed strategy | First seed | Reduction | Tokens | Impact |\n" +
      "| --- | --- | --- | --- | --- | --- | --- | --- | ---: | ---: | ---: |\n" +
      ($case.routes | map(
        "| \(.task) | `\(.first_file)` | \(.first_reading_focus | gsub("\\|"; "\\|")) | \(.first_reading_question | gsub("\\|"; "\\|")) | `\(.first_suggested_tool)` | `\(.route_quality_level) / \(.route_quality_score)` | `\(.seed_strategy)` | `\(.first_seed_value)` | `\(.line_reduction)` | `\(.estimated_tokens)` | `\(.risk_level) / \(.impacted_files)` |"
      ) | join("\n")) +
      "\n"' "$SUMMARY_JSON"
    echo "## Route Quality Evidence"
    echo
    jq -r '.cases[] as $case |
      "### " + $case.case + "\n\n" +
      ($case.routes | map(
        "- `\(.task)`: \(.route_quality_decision_summary) Confidence: \((.route_quality_confidence_factors[0] // "-")) Verification: \((.route_quality_verification_steps[0] // "-"))"
      ) | join("\n")) +
      "\n"' "$SUMMARY_JSON"
    echo "## Artifacts"
    echo
    jq -r '.cases[] | "- `\(.case)`: `\(.summary_json)`"' "$SUMMARY_JSON"
  } >"$OUTPUT_FILE"
}

write_evidence_summary() {
  local mode="$1"
  local prefix case_prefix

  if [ "$mode" = "markdown" ]; then
    prefix="- "
    case_prefix="  - "
  else
    prefix="  "
    case_prefix="  - "
  fi

  if [ "$mode" != "markdown" ]; then
    echo "evidence summary"
  fi
  echo "${prefix}cases: $(jq -r '.case_count' "$SUMMARY_JSON")"
  echo "${prefix}tasks: $(jq -r '.aggregate.task_count' "$SUMMARY_JSON")"
  echo "${prefix}expectations: $(jq -r '.aggregate.expectation_count' "$SUMMARY_JSON")/$(jq -r '.aggregate.task_count' "$SUMMARY_JSON")"
  echo "${prefix}source_lines: $(jq -r '.aggregate.total_task_source_lines' "$SUMMARY_JSON")"
  echo "${prefix}selected_lines: $(jq -r '.aggregate.total_selected_lines' "$SUMMARY_JSON")"
  echo "${prefix}line_reduction: $(jq -r '.aggregate.line_reduction' "$SUMMARY_JSON")%"
  echo "${prefix}estimated_tokens: $(jq -r '.aggregate.total_estimated_tokens' "$SUMMARY_JSON")"
  echo "${prefix}max_impacted_files: $(jq -r '.aggregate.max_impacted_files' "$SUMMARY_JSON")"
  if jq -e '.quality_gate? | type == "object"' "$SUMMARY_JSON" >/dev/null; then
    echo "${prefix}quality_gate: $(jq -r '.quality_gate.status' "$SUMMARY_JSON") >= $(jq -r '.quality_gate.min_route_quality_score' "$SUMMARY_JSON")"
  fi
  jq -r --arg prefix "$case_prefix" '.cases[] |
    $prefix + .case + ": " + (.task_count | tostring) + " tasks, first files " +
    ([.routes[].first_file] | unique | join(", "))' "$SUMMARY_JSON"
}

main() {
  parse_args "$@"
  require_command jq
  require_command git

  [ -x "$MATRIX_SCRIPT" ] || fail "task routing matrix script is not executable: $MATRIX_SCRIPT"
  case "$TOKEN_BUDGET" in
    ''|*[!0-9]*)
      fail "--token-budget must be a positive integer"
      ;;
  esac
  if [ "$TOKEN_BUDGET" -le 0 ]; then
    fail "--token-budget must be greater than zero"
  fi
  if [ -n "$MIN_ROUTE_QUALITY_SCORE" ]; then
    case "$MIN_ROUTE_QUALITY_SCORE" in
      ''|*[!0-9]*)
        fail "--min-route-quality-score must be a non-negative integer"
        ;;
    esac
  fi
  if [ "${#CASES[@]}" -eq 0 ]; then
    CASES=(express fastapi flask gin requests streamlit wouter)
  fi

  OUTPUT_DIR="${OUTPUT_DIR:-$WORK_DIR/matrix}"
  OUTPUT_FILE="${OUTPUT_FILE:-$OUTPUT_DIR/public-task-routing-matrix.md}"
  SUMMARY_JSON="${SUMMARY_JSON:-$OUTPUT_DIR/summary.json}"
  rm -rf "$OUTPUT_DIR"
  mkdir -p "$OUTPUT_DIR"

  local rows_file case_name row_file
  rows_file="$OUTPUT_DIR/cases.txt"
  : >"$rows_file"
  for case_name in "${CASES[@]}"; do
    row_file="$(run_case "$case_name")"
    printf "%s\n" "$row_file" >>"$rows_file"
  done

  write_summary "$rows_file"
  write_markdown

  echo "public task routing matrix written to $OUTPUT_FILE"
  echo "summary: $SUMMARY_JSON"
  echo "cases: ${#CASES[@]}"
  write_evidence_summary terminal
}

main "$@"
