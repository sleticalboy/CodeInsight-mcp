#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${CODEINSIGHT_BENCH_WORKDIR:-${TMPDIR:-/tmp}/codeinsight-benchmark}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-$ROOT_DIR/target/release/codeinsight}"
BENCH_PROFILE="${CODEINSIGHT_BENCH_PROFILE:-smoke}"
DISABLE_BUDGETS="${CODEINSIGHT_BENCH_DISABLE_BUDGETS:-0}"
REPORT_FILE=""

REPO_NAMES=()
REPO_URLS=()
REPO_LANGUAGES=()
REPO_CONTEXT_FILES=()
REPO_CONTEXT_TASKS=()
REPO_MAX_INDEX_MS=()
REPO_SYMBOL_TARGETS=()
REPO_CALL_TARGETS=()
REPO_CALL_EDGES=()
OUTPUT=""
BUDGET_FAILURES=0
SYMBOL_TARGET_FAILURES=0
CALL_TARGET_FAILURES=0
CALL_EDGE_FAILURES=0

configure_profile() {
  case "$BENCH_PROFILE" in
    smoke)
      OUTPUT="${CODEINSIGHT_BENCH_OUTPUT:-$ROOT_DIR/docs/benchmark-v0.1.md}"
      REPO_NAMES=(
        "p-limit"
        "itsdangerous"
        "go-example"
        "memchr"
      )
      REPO_URLS=(
        "https://github.com/sindresorhus/p-limit.git"
        "https://github.com/pallets/itsdangerous.git"
        "https://github.com/golang/example.git"
        "https://github.com/BurntSushi/memchr.git"
      )
      REPO_LANGUAGES=(
        "TypeScript"
        "Python"
        "Go"
        "Rust"
      )
      REPO_CONTEXT_FILES=(
        "index.js"
        "src/itsdangerous/serializer.py"
        "hello/hello.go"
        "src/lib.rs"
      )
      REPO_CONTEXT_TASKS=(
        "understand limit scheduling behavior"
        "understand serializer signing behavior"
        "understand hello server behavior"
        "understand memchr finder API"
      )
      REPO_MAX_INDEX_MS=(
        5000
        5000
        5000
        10000
      )
      REPO_SYMBOL_TARGETS=(
        ""
        ""
        ""
        ""
      )
      REPO_CALL_TARGETS=(
        ""
        ""
        ""
        ""
      )
      REPO_CALL_EDGES=(
        ""
        ""
        ""
        ""
      )
      ;;
    large)
      OUTPUT="${CODEINSIGHT_BENCH_OUTPUT:-$ROOT_DIR/docs/benchmark-large.md}"
      REPO_NAMES=(
        "express"
        "flask"
        "gin"
        "tokio"
      )
      REPO_URLS=(
        "https://github.com/expressjs/express.git"
        "https://github.com/pallets/flask.git"
        "https://github.com/gin-gonic/gin.git"
        "https://github.com/tokio-rs/tokio.git"
      )
      REPO_LANGUAGES=(
        "JavaScript"
        "Python"
        "Go"
        "Rust"
      )
      REPO_CONTEXT_FILES=(
        "lib/application.js"
        "src/flask/app.py"
        "gin.go"
        "tokio/src/lib.rs"
      )
      REPO_CONTEXT_TASKS=(
        "understand express application routing behavior"
        "understand flask application dispatch behavior"
        "understand gin engine routing behavior"
        "understand tokio runtime public API"
      )
      REPO_MAX_INDEX_MS=(
        10000
        5000
        5000
        20000
      )
      REPO_SYMBOL_TARGETS=(
        "createError:1|handleError:1|User.index:1|User.range:1|users.list:1|METHODS:1|Buffer:1|address:1|port:1"
        ""
        ""
        ""
      )
      REPO_CALL_TARGETS=(
        "app.get:1|app.<dynamic>:1|app.route.get:1|router.route.get:1"
        ""
        ""
        ""
      )
      REPO_CALL_EDGES=(
        "it.<callback>->app.route.get:1|app.get.<callback>->res.send:1"
        ""
        ""
        ""
      )
      ;;
    *)
      echo "unknown benchmark profile: $BENCH_PROFILE" >&2
      echo "supported profiles: smoke, large" >&2
      exit 1
      ;;
  esac
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

clone_repo() {
  local name="$1"
  local url="$2"
  local repo_dir="$WORK_DIR/repos/$name"
  local attempts=3

  rm -rf "$repo_dir"

  for attempt in $(seq 1 "$attempts"); do
    if git -c http.version=HTTP/1.1 clone --quiet --depth 1 "$url" "$repo_dir"; then
      break
    fi

    rm -rf "$repo_dir"
    if [ "$attempt" -eq "$attempts" ]; then
      echo "failed to clone $url after $attempts attempts" >&2
      exit 1
    fi

    echo "clone failed for $name, retrying ($attempt/$attempts)" >&2
    sleep "$attempt"
  done

  rm -rf "$repo_dir/.codeinsight"
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

write_report_header() {
  local generated_at
  local display_bin
  local profile_title
  generated_at="$(date -u +"%Y-%m-%d %H:%M:%S UTC")"
  display_bin="$CODEINSIGHT_BIN"
  display_bin="${display_bin/#$ROOT_DIR\//}"
  case "$BENCH_PROFILE" in
    smoke) profile_title="Smoke" ;;
    large) profile_title="Large Repository" ;;
    *) profile_title="$BENCH_PROFILE" ;;
  esac

  cat >"$REPORT_FILE" <<EOF
# CodeInsight v0.1 $profile_title Benchmark

Generated at: $generated_at

This is a benchmark fixture report, not a controlled performance benchmark. It
verifies that CodeInsight can index real public repositories across the MVP
language set and produce stable project summaries and context packs without
crashing.

Environment:

- Command: \`$display_bin\`
- Profile: \`$BENCH_PROFILE\`
- Work directory: temporary clone directory
- Index mode: forced clean index per repository
- Context pack mode: one stable file seed per repository, 6000 token budget
- Index budget mode: $(budget_mode)

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Context files | Ranges | Context lines | Line reduction | Tokens | Applied budget | Omitted files | Continuation | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
EOF
}

budget_mode() {
  if [ "$DISABLE_BUDGETS" = "1" ]; then
    printf "disabled"
  else
    printf "enabled"
  fi
}

budget_status() {
  local duration="$1"
  local budget="$2"

  if [ "$DISABLE_BUDGETS" = "1" ]; then
    printf "skipped"
  elif [ "$budget" -le 0 ]; then
    printf "n/a"
  elif [ "$duration" -le "$budget" ]; then
    printf "pass"
  else
    printf "fail"
  fi
}

context_lines() {
  local context_json="$1"
  jq -r '[.files[].ranges[] | (.end_line - .start_line + 1)] | add // 0' "$context_json"
}

validate_symbol_target_guardrails() {
  local name="$1"
  local repo_dir="$2"
  local specs="$3"
  local output="$4"
  local checks check target minimum count status

  : >"$output"
  if [ -z "$specs" ]; then
    return
  fi

  IFS="|" read -r -a checks <<<"$specs"
  for check in "${checks[@]}"; do
    target="${check%%:*}"
    minimum="${check##*:}"
    count="$("$CODEINSIGHT_BIN" symbols "$repo_dir" "$target" --limit 1000 | jq -r --arg target "$target" '[.[] | select(.name == $target or .qualified_name == $target)] | length')"
    status="pass"
    if [ "$count" -lt "$minimum" ]; then
      status="fail"
      SYMBOL_TARGET_FAILURES=$((SYMBOL_TARGET_FAILURES + 1))
    fi

    printf "%s\t%s\t%s\t%s\n" "$target" "$minimum" "$count" "$status" >>"$output"
    echo "symbol target guardrail $name $target: $count >= $minimum ($status)"
  done
}

validate_call_target_guardrails() {
  local name="$1"
  local repo_dir="$2"
  local specs="$3"
  local output="$4"
  local checks check target minimum count status

  : >"$output"
  if [ -z "$specs" ]; then
    return
  fi

  IFS="|" read -r -a checks <<<"$specs"
  for check in "${checks[@]}"; do
    target="${check%%:*}"
    minimum="${check##*:}"
    count="$("$CODEINSIGHT_BIN" callers "$repo_dir" "$target" --limit 1000 | jq -r 'length')"
    status="pass"
    if [ "$count" -lt "$minimum" ]; then
      status="fail"
      CALL_TARGET_FAILURES=$((CALL_TARGET_FAILURES + 1))
    fi

    printf "%s\t%s\t%s\t%s\n" "$target" "$minimum" "$count" "$status" >>"$output"
    echo "call target guardrail $name $target: $count >= $minimum ($status)"
  done
}

validate_call_edge_guardrails() {
  local name="$1"
  local repo_dir="$2"
  local specs="$3"
  local output="$4"
  local checks check edge caller callee minimum count status

  : >"$output"
  if [ -z "$specs" ]; then
    return
  fi

  IFS="|" read -r -a checks <<<"$specs"
  for check in "${checks[@]}"; do
    edge="${check%%:*}"
    minimum="${check##*:}"
    caller="${edge%%->*}"
    callee="${edge##*->}"
    count="$("$CODEINSIGHT_BIN" callers "$repo_dir" "$callee" --limit 1000 | jq -r --arg caller "$caller" '[.[] | select(.caller == $caller)] | length')"
    status="pass"
    if [ "$count" -lt "$minimum" ]; then
      status="fail"
      CALL_EDGE_FAILURES=$((CALL_EDGE_FAILURES + 1))
    fi

    printf "%s\t%s\t%s\t%s\t%s\n" "$caller" "$callee" "$minimum" "$count" "$status" >>"$output"
    echo "call edge guardrail $name $caller -> $callee: $count >= $minimum ($status)"
  done
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

append_summary_row() {
  local name="$1"
  local language="$2"
  local repo_dir="$3"
  local index_json="$4"
  local overview_json="$5"
  local context_json="$6"
  local max_index_ms="$7"

  local commit files lines symbols skipped errors duration budget db_size context_files ranges selected_lines reduction tokens applied_budget omitted_files continuation_status truncated first_context_file status
  commit="$(git -C "$repo_dir" rev-parse --short HEAD)"
  files="$(json_value "$index_json" '.indexed_files')"
  lines="$(json_value "$overview_json" '[.languages[].lines] | add // 0')"
  symbols="$(json_value "$index_json" '.symbols')"
  skipped="$(json_value "$index_json" '.skipped_files')"
  errors="$(json_value "$index_json" '.errors | length')"
  duration="$(json_value "$index_json" '.duration_ms')"
  budget="$max_index_ms"
  db_size="$(du -h "$repo_dir/.codeinsight/index.db" | awk '{print $1}')"
  context_files="$(json_value "$context_json" '.files | length')"
  ranges="$(json_value "$context_json" '[.files[].ranges | length] | add // 0')"
  selected_lines="$(context_lines "$context_json")"
  reduction="$(line_reduction "$lines" "$selected_lines")"
  tokens="$(json_value "$context_json" '.estimated_tokens')"
  applied_budget="$(json_value "$context_json" '.budget.applied_token_budget // "-"')"
  omitted_files="$(json_value "$context_json" '.budget.omitted_files // 0')"
  continuation_status="$(json_value "$context_json" '.continuation_summary.status // "-"')"
  truncated="$(json_value "$context_json" '.truncated')"
  first_context_file="$(json_value "$context_json" '.files[0].file // "-"')"
  status="$(budget_status "$duration" "$budget")"

  if [ "$status" = "fail" ]; then
    BUDGET_FAILURES=$((BUDGET_FAILURES + 1))
  fi

  printf "| %s | %s | \`%s\` | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | \`%s\` |\n" \
    "$name" "$language" "$commit" "$files" "$lines" "$symbols" "$skipped" "$errors" "$duration" "$budget" "$status" "$db_size" \
    "$context_files" "$ranges" "$selected_lines" "$reduction" "$tokens" "$applied_budget" "$omitted_files" "$continuation_status" "$truncated" "$first_context_file" \
    >>"$REPORT_FILE"
}

append_detail_section() {
  local name="$1"
  local repo_url="$2"
  local repo_dir="$3"
  local index_json="$4"
  local overview_json="$5"
  local context_json="$6"
  local context_file="$7"
  local context_task="$8"
  local max_index_ms="$9"
  local symbol_targets_file="${10}"
  local call_targets_file="${11}"
  local call_edges_file="${12}"
  local duration total_lines selected_lines reduction status
  duration="$(json_value "$index_json" '.duration_ms')"
  total_lines="$(json_value "$overview_json" '[.languages[].lines] | add // 0')"
  selected_lines="$(context_lines "$context_json")"
  reduction="$(line_reduction "$total_lines" "$selected_lines")"
  status="$(budget_status "$duration" "$max_index_ms")"

  {
    echo
    echo "## $name"
    echo
    echo "- URL: $repo_url"
    echo "- Commit: \`$(git -C "$repo_dir" rev-parse HEAD)\`"
    echo "- Indexed files: $(json_value "$index_json" '.indexed_files')"
    echo "- Symbols: $(json_value "$index_json" '.symbols')"
    echo "- Duration: $duration ms"
    echo "- Index budget: $max_index_ms ms ($status)"
    echo "- Context seed file: \`$context_file\`"
    echo "- Context task: $context_task"
    echo "- Context files: $(json_value "$context_json" '.files | length')"
    echo "- Context ranges: $(json_value "$context_json" '[.files[].ranges | length] | add // 0')"
    echo "- Context lines: $selected_lines of $total_lines ($reduction reduction)"
    echo "- Context estimated tokens: $(json_value "$context_json" '.estimated_tokens')"
    echo "- Context applied token budget: $(json_value "$context_json" '.budget.applied_token_budget // "-"')"
    echo "- Context omitted files: $(json_value "$context_json" '.budget.omitted_files // 0')"
    echo "- Context omitted ranges: $(json_value "$context_json" '.budget.omitted_ranges // 0')"
    echo "- Context truncation reason: $(json_value "$context_json" '.budget.truncation_reason // "none"')"
    echo "- Context continuation status: $(json_value "$context_json" '.continuation_summary.status // "-"')"
    echo "- Context truncated: $(json_value "$context_json" '.truncated')"
    echo
    echo "Context pack files:"
    echo
    echo "| File | Ranges | First range | Importances |"
    echo "| --- | ---: | --- | --- |"
  } >>"$REPORT_FILE"

  jq -r '
    .files[]
    | "| `\(.file)` | \(.ranges | length) | \((.ranges[0].start_line | tostring) + "-" + (.ranges[0].end_line | tostring)) | \([.ranges[].importance] | unique | join(", ")) |"
  ' "$context_json" >>"$REPORT_FILE"

  {
    echo
    echo "Language breakdown:"
    echo
    echo "| Language | Files | Lines |"
    echo "| --- | ---: | ---: |"
  } >>"$REPORT_FILE"

  jq -r '.languages[] | "| \(.language) | \(.files) | \(.lines) |"' "$overview_json" >>"$REPORT_FILE"

  local error_count
  error_count="$(json_value "$index_json" '.errors | length')"
  if [ "$error_count" -gt 0 ]; then
    {
      echo
      echo "Index errors:"
      echo
    } >>"$REPORT_FILE"
    jq -r '.errors[:10][] | "- `\(.file)` during `\(.stage)`: \(.message)"' "$index_json" >>"$REPORT_FILE"
  fi

  if [ -s "$symbol_targets_file" ]; then
    {
      echo
      echo "Symbol target guardrails:"
      echo
      echo "| Target | Minimum symbols | Observed symbols | Status |"
      echo "| --- | ---: | ---: | --- |"
    } >>"$REPORT_FILE"

    while IFS=$'\t' read -r target minimum count guardrail_status; do
      printf "| \`%s\` | %s | %s | %s |\n" "$target" "$minimum" "$count" "$guardrail_status" >>"$REPORT_FILE"
    done <"$symbol_targets_file"
  fi

  if [ -s "$call_targets_file" ]; then
    {
      echo
      echo "Call target guardrails:"
      echo
      echo "| Target | Minimum calls | Observed calls | Status |"
      echo "| --- | ---: | ---: | --- |"
    } >>"$REPORT_FILE"

    while IFS=$'\t' read -r target minimum count guardrail_status; do
      printf "| \`%s\` | %s | %s | %s |\n" "$target" "$minimum" "$count" "$guardrail_status" >>"$REPORT_FILE"
    done <"$call_targets_file"
  fi

  if [ -s "$call_edges_file" ]; then
    {
      echo
      echo "Call edge guardrails:"
      echo
      echo "| Caller | Callee | Minimum calls | Observed calls | Status |"
      echo "| --- | --- | ---: | ---: | --- |"
    } >>"$REPORT_FILE"

    while IFS=$'\t' read -r caller callee minimum count guardrail_status; do
      printf "| \`%s\` | \`%s\` | %s | %s | %s |\n" "$caller" "$callee" "$minimum" "$count" "$guardrail_status" >>"$REPORT_FILE"
    done <"$call_edges_file"
  fi
}

main() {
  configure_profile

  require_command git
  require_command jq
  require_command cargo
  require_command du
  require_command awk

  mkdir -p "$WORK_DIR/results" "$(dirname "$OUTPUT")"
  REPORT_FILE="$WORK_DIR/results/benchmark-report.md"

  echo "building release binary"
  cargo build --locked --release --manifest-path "$ROOT_DIR/Cargo.toml"

  write_report_header

  for i in "${!REPO_NAMES[@]}"; do
    name="${REPO_NAMES[$i]}"
    url="${REPO_URLS[$i]}"
    language="${REPO_LANGUAGES[$i]}"
    context_file="${REPO_CONTEXT_FILES[$i]}"
    context_task="${REPO_CONTEXT_TASKS[$i]}"
    max_index_ms="${REPO_MAX_INDEX_MS[$i]}"
    symbol_targets="${REPO_SYMBOL_TARGETS[$i]}"
    call_targets="${REPO_CALL_TARGETS[$i]}"
    call_edges="${REPO_CALL_EDGES[$i]}"
    repo_dir="$WORK_DIR/repos/$name"
    index_json="$WORK_DIR/results/$name-index.json"
    overview_json="$WORK_DIR/results/$name-overview.json"
    context_json="$WORK_DIR/results/$name-context.json"
    symbol_targets_file="$WORK_DIR/results/$name-symbol-targets.tsv"
    call_targets_file="$WORK_DIR/results/$name-call-targets.tsv"
    call_edges_file="$WORK_DIR/results/$name-call-edges.tsv"

    echo "benchmarking $name"
    clone_repo "$name" "$url"
    "$CODEINSIGHT_BIN" index "$repo_dir" --force >"$index_json"
    "$CODEINSIGHT_BIN" overview "$repo_dir" >"$overview_json"
    "$CODEINSIGHT_BIN" context-pack "$repo_dir" \
      --task "$context_task" \
      --file "$context_file" \
      --token-budget 6000 \
      >"$context_json"
    validate_symbol_target_guardrails "$name" "$repo_dir" "$symbol_targets" "$symbol_targets_file"
    validate_call_target_guardrails "$name" "$repo_dir" "$call_targets" "$call_targets_file"
    validate_call_edge_guardrails "$name" "$repo_dir" "$call_edges" "$call_edges_file"
    append_summary_row "$name" "$language" "$repo_dir" "$index_json" "$overview_json" "$context_json" "$max_index_ms"
  done

  cat >>"$REPORT_FILE" <<EOF

## Details
EOF

  for i in "${!REPO_NAMES[@]}"; do
    name="${REPO_NAMES[$i]}"
    url="${REPO_URLS[$i]}"
    repo_dir="$WORK_DIR/repos/$name"
    index_json="$WORK_DIR/results/$name-index.json"
    overview_json="$WORK_DIR/results/$name-overview.json"
    context_json="$WORK_DIR/results/$name-context.json"
    context_file="${REPO_CONTEXT_FILES[$i]}"
    context_task="${REPO_CONTEXT_TASKS[$i]}"
    max_index_ms="${REPO_MAX_INDEX_MS[$i]}"
    symbol_targets_file="$WORK_DIR/results/$name-symbol-targets.tsv"
    call_targets_file="$WORK_DIR/results/$name-call-targets.tsv"
    call_edges_file="$WORK_DIR/results/$name-call-edges.tsv"
    append_detail_section "$name" "$url" "$repo_dir" "$index_json" "$overview_json" "$context_json" "$context_file" "$context_task" "$max_index_ms" "$symbol_targets_file" "$call_targets_file" "$call_edges_file"
  done

  mv "$REPORT_FILE" "$OUTPUT"
  echo "wrote $OUTPUT"
  if [ "$BUDGET_FAILURES" -gt 0 ]; then
    echo "benchmark budget failures: $BUDGET_FAILURES" >&2
    exit 1
  fi
  if [ "$SYMBOL_TARGET_FAILURES" -gt 0 ]; then
    echo "symbol target guardrail failures: $SYMBOL_TARGET_FAILURES" >&2
    exit 1
  fi
  if [ "$CALL_TARGET_FAILURES" -gt 0 ]; then
    echo "call target guardrail failures: $CALL_TARGET_FAILURES" >&2
    exit 1
  fi
  if [ "$CALL_EDGE_FAILURES" -gt 0 ]; then
    echo "call edge guardrail failures: $CALL_EDGE_FAILURES" >&2
    exit 1
  fi
}

main "$@"
