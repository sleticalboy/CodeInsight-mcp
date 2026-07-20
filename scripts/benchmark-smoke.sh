#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORK_DIR="${CODEINSIGHT_BENCH_WORKDIR:-${TMPDIR:-/tmp}/codeinsight-benchmark}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
BENCH_PROFILE="${CODEINSIGHT_BENCH_PROFILE:-smoke}"
DISABLE_BUDGETS="${CODEINSIGHT_BENCH_DISABLE_BUDGETS:-0}"
REUSE_REPOS="${CODEINSIGHT_BENCH_REUSE_REPOS:-0}"
BENCH_REPOS="${CODEINSIGHT_BENCH_REPOS:-}"
OUTPUT_WAS_SET="${CODEINSIGHT_BENCH_OUTPUT+x}"
PRINT_CONFIG="${CODEINSIGHT_BENCH_PRINT_CONFIG:-0}"
SUMMARY_JSON="${CODEINSIGHT_BENCH_SUMMARY_JSON:-}"
LOCAL_ROOT="${CODEINSIGHT_BENCH_LOCAL_ROOT:-}"
LOCAL_NAME="${CODEINSIGHT_BENCH_LOCAL_NAME:-}"
LOCAL_LANGUAGE="${CODEINSIGHT_BENCH_LOCAL_LANGUAGE:-Local}"
LOCAL_CONTEXT_FILE="${CODEINSIGHT_BENCH_LOCAL_CONTEXT_FILE:-}"
LOCAL_CONTEXT_TASK="${CODEINSIGHT_BENCH_LOCAL_TASK:-understand the local repository first-read path}"
LOCAL_CONTEXT_GUARDRAILS="${CODEINSIGHT_BENCH_LOCAL_GUARDRAILS:-selected_files:1|selected_ranges:1|reading_plan_steps:1|max_tokens:6000|min_line_reduction:0}"
LOCAL_MAX_INDEX_MS="${CODEINSIGHT_BENCH_LOCAL_MAX_INDEX_MS:-10000}"
REPORT_FILE=""

REPO_NAMES=()
REPO_URLS=()
REPO_LOCAL_ROOTS=()
REPO_LANGUAGES=()
REPO_CONTEXT_FILES=()
REPO_CONTEXT_TASKS=()
REPO_CONTEXT_GUARDRAILS=()
REPO_MAX_INDEX_MS=()
REPO_SYMBOL_TARGETS=()
REPO_CALL_TARGETS=()
REPO_CALL_EDGES=()
OUTPUT=""
BUDGET_FAILURES=0
CONTEXT_GUARDRAIL_FAILURES=0
SYMBOL_TARGET_FAILURES=0
CALL_TARGET_FAILURES=0
CALL_EDGE_FAILURES=0
BENCHMARKED_REPOS=0
TOTAL_REPO_LINES=0
TOTAL_CONTEXT_LINES=0
TOTAL_CONTEXT_TOKENS=0
TOTAL_CONTEXT_FILES=0
TOTAL_CONTEXT_RANGES=0
TOTAL_INDEX_MS=0
CONTEXT_PACK_FIRST_RECOMMENDATIONS=0
TRUNCATED_CONTEXT_PACKS=0

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
      REPO_LOCAL_ROOTS=(
        ""
        ""
        ""
        ""
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
      REPO_CONTEXT_GUARDRAILS=(
        "selected_files:1|selected_ranges:3|reading_plan_steps:1|max_tokens:1200|min_line_reduction:80"
        "selected_files:3|selected_ranges:6|reading_plan_steps:3|max_tokens:3000|min_line_reduction:80"
        "selected_files:3|selected_ranges:4|reading_plan_steps:3|max_tokens:1000|min_line_reduction:90"
        "selected_files:5|selected_ranges:5|reading_plan_steps:5|max_tokens:2500|min_line_reduction:95"
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
      REPO_LOCAL_ROOTS=(
        ""
        ""
        ""
        ""
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
      REPO_CONTEXT_GUARDRAILS=(
        "selected_files:3|selected_ranges:10|reading_plan_steps:3|max_tokens:3000|min_line_reduction:95"
        "selected_files:8|selected_ranges:10|reading_plan_steps:6|max_tokens:6000|min_line_reduction:90"
        "selected_files:3|selected_ranges:10|reading_plan_steps:3|max_tokens:3500|min_line_reduction:95"
        "selected_files:12|selected_ranges:18|reading_plan_steps:6|max_tokens:5500|min_line_reduction:95"
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
    local)
      configure_local_profile
      ;;
    *)
      echo "unknown benchmark profile: $BENCH_PROFILE" >&2
      echo "supported profiles: smoke, large, local" >&2
      exit 1
      ;;
  esac
}

configure_local_profile() {
  local root name

  if [ -z "$LOCAL_ROOT" ]; then
    echo "CODEINSIGHT_BENCH_LOCAL_ROOT is required when CODEINSIGHT_BENCH_PROFILE=local" >&2
    exit 1
  fi
  if [ -z "$LOCAL_CONTEXT_FILE" ]; then
    echo "CODEINSIGHT_BENCH_LOCAL_CONTEXT_FILE is required when CODEINSIGHT_BENCH_PROFILE=local" >&2
    exit 1
  fi
  if [ ! -d "$LOCAL_ROOT" ]; then
    echo "CODEINSIGHT_BENCH_LOCAL_ROOT is not a directory: $LOCAL_ROOT" >&2
    exit 1
  fi
  if [ ! -f "$LOCAL_ROOT/$LOCAL_CONTEXT_FILE" ]; then
    echo "CODEINSIGHT_BENCH_LOCAL_CONTEXT_FILE does not exist under local root: $LOCAL_CONTEXT_FILE" >&2
    exit 1
  fi

  root="$(cd "$LOCAL_ROOT" && pwd)"
  name="$LOCAL_NAME"
  if [ -z "$name" ]; then
    name="$(basename "$root")"
  fi

  OUTPUT="${CODEINSIGHT_BENCH_OUTPUT:-$WORK_DIR/results/benchmark-local.md}"
  REPO_NAMES=("$name")
  REPO_URLS=("local:$root")
  REPO_LOCAL_ROOTS=("$root")
  REPO_LANGUAGES=("$LOCAL_LANGUAGE")
  REPO_CONTEXT_FILES=("$LOCAL_CONTEXT_FILE")
  REPO_CONTEXT_TASKS=("$LOCAL_CONTEXT_TASK")
  REPO_CONTEXT_GUARDRAILS=("$LOCAL_CONTEXT_GUARDRAILS")
  REPO_MAX_INDEX_MS=("$LOCAL_MAX_INDEX_MS")
  REPO_SYMBOL_TARGETS=("")
  REPO_CALL_TARGETS=("")
  REPO_CALL_EDGES=("")
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

repo_selected() {
  local name="$1"

  if [ -z "$BENCH_REPOS" ]; then
    return 0
  fi

  case ",$BENCH_REPOS," in
    *",$name,"*) return 0 ;;
    *) return 1 ;;
  esac
}

validate_repo_subset() {
  local requested known name found selected_count

  if [ -z "$BENCH_REPOS" ]; then
    return
  fi

  selected_count=0
  IFS="," read -r -a requested <<<"$BENCH_REPOS"
  for name in "${requested[@]}"; do
    found=0
    for known in "${REPO_NAMES[@]}"; do
      if [ "$name" = "$known" ]; then
        found=1
        selected_count=$((selected_count + 1))
        break
      fi
    done

    if [ "$found" -eq 0 ]; then
      echo "unknown benchmark repository in CODEINSIGHT_BENCH_REPOS: $name" >&2
      echo "available repositories: ${REPO_NAMES[*]}" >&2
      exit 1
    fi
  done

  if [ "$selected_count" -eq 0 ]; then
    echo "CODEINSIGHT_BENCH_REPOS did not select any repositories" >&2
    exit 1
  fi
}

repo_subset_label() {
  if [ -z "$BENCH_REPOS" ]; then
    printf "all"
  else
    printf "%s" "$BENCH_REPOS"
  fi
}

print_benchmark_config() {
  local i name

  printf "name\tcontext_guardrails\n"
  for i in "${!REPO_NAMES[@]}"; do
    name="${REPO_NAMES[$i]}"
    if ! repo_selected "$name"; then
      continue
    fi

    printf "%s\t%s\n" "$name" "${REPO_CONTEXT_GUARDRAILS[$i]}"
  done
}

clone_repo() {
  local name="$1"
  local url="$2"
  local local_root="${3:-}"
  local repo_dir="$WORK_DIR/repos/$name"
  local attempts=3

  if [ -n "$local_root" ]; then
    rm -rf "$repo_dir"
    mkdir -p "$repo_dir"
    (
      cd "$local_root"
      tar --exclude .codeinsight -cf - .
    ) | (
      cd "$repo_dir"
      tar -xf -
    )
    rm -rf "$repo_dir/.codeinsight"
    return
  fi

  if [ "$REUSE_REPOS" = "1" ] && [ -d "$repo_dir" ]; then
    if git -C "$repo_dir" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
      echo "reusing existing checkout for $name"
      git -C "$repo_dir" reset --hard --quiet HEAD
      git -C "$repo_dir" clean -ffd --quiet
      rm -rf "$repo_dir/.codeinsight"
      return
    fi

    echo "discarding invalid checkout for $name"
    rm -rf "$repo_dir"
  fi

  rm -rf "$repo_dir"

  for attempt in $(seq 1 "$attempts"); do
    if git \
      -c http.version=HTTP/1.1 \
      -c http.lowSpeedLimit=1024 \
      -c http.lowSpeedTime=30 \
      clone --quiet --depth 1 "$url" "$repo_dir"; then
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

repo_commit_short() {
  local repo_dir="$1"

  if git -C "$repo_dir" rev-parse --short HEAD >/dev/null 2>&1; then
    git -C "$repo_dir" rev-parse --short HEAD
  else
    printf "local"
  fi
}

repo_commit_full() {
  local repo_dir="$1"

  if git -C "$repo_dir" rev-parse HEAD >/dev/null 2>&1; then
    git -C "$repo_dir" rev-parse HEAD
  else
    printf "local"
  fi
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
    local) profile_title="Local Repository" ;;
    *) profile_title="$BENCH_PROFILE" ;;
  esac

  cat >"$REPORT_FILE" <<EOF
# CodeInsight v0.1 $profile_title Benchmark

Generated at: $generated_at

This is a benchmark fixture report, not a controlled performance benchmark. It
verifies that CodeInsight can index real repositories across the MVP language
set and produce stable project summaries and context packs without crashing.

Environment:

- Command: \`$display_bin\`
- Profile: \`$BENCH_PROFILE\`
- Work directory: temporary benchmark directory
- Repository subset: \`$(repo_subset_label)\`
- Index mode: forced clean index per repository
- Context pack mode: one stable file seed per repository, 6000 token budget
- Index budget mode: $(budget_mode)

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Skipped | Errors | Index ms | Index budget ms | Budget status | DB size | Entrypoints | First entrypoint | Recommended tools | First recommended tool | Context files | Ranges | Context lines | Line reduction | Tokens | Applied budget | Omitted files | Continuation | Truncated | First context file |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | ---: | ---: | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- | --- | --- |
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

write_context_guardrail() {
  local output="$1"
  local name="$2"
  local check="$3"
  local expectation="$4"
  local observed="$5"
  local status="$6"

  if [ "$status" != "pass" ]; then
    CONTEXT_GUARDRAIL_FAILURES=$((CONTEXT_GUARDRAIL_FAILURES + 1))
  fi

  printf "%s\t%s\t%s\t%s\n" "$check" "$expectation" "$observed" "$status" >>"$output"
  echo "context guardrail $name $check: $observed ($status)"
}

context_guardrail_value() {
  local specs="$1"
  local key="$2"
  local default="$3"
  local checks check

  IFS="|" read -r -a checks <<<"$specs"
  for check in "${checks[@]}"; do
    if [ "${check%%:*}" = "$key" ]; then
      printf "%s" "${check#*:}"
      return
    fi
  done

  printf "%s" "$default"
}

percentage_at_least() {
  local total="$1"
  local selected="$2"
  local minimum="$3"

  awk -v total="$total" -v selected="$selected" -v minimum="$minimum" 'BEGIN {
    if (total <= 0) exit 1
    reduction = (1 - (selected / total)) * 100
    if (reduction < 0) reduction = 0
    exit(reduction >= minimum ? 0 : 1)
  }'
}

validate_context_guardrails() {
  local name="$1"
  local overview_json="$2"
  local context_json="$3"
  local output="$4"
  local specs="$5"
  local total_lines selected_lines first_recommended_tool context_files ranges reading_plan_steps first_next_action first_reading_focus first_reading_question first_reading_reason first_selection_reason first_selection_rank estimated_tokens applied_budget status min_files min_ranges min_reading_plan_steps max_tokens min_line_reduction

  : >"$output"

  total_lines="$(json_value "$overview_json" '[.languages[].lines] | add // 0')"
  selected_lines="$(context_lines "$context_json")"
  first_recommended_tool="$(json_value "$overview_json" '.recommended_next_tools[0].tool // "-"')"
  context_files="$(json_value "$context_json" '.files | length')"
  ranges="$(json_value "$context_json" '[.files[].ranges | length] | add // 0')"
  reading_plan_steps="$(json_value "$context_json" '.reading_plan | length')"
  first_next_action="$(json_value "$context_json" '.reading_plan[0].next_action // "-"')"
  first_reading_focus="$(json_value "$context_json" '.reading_plan[0].focus // "-"')"
  first_reading_question="$(json_value "$context_json" '.reading_plan[0].question // "-"')"
  first_reading_reason="$(json_value "$context_json" '.reading_plan[0].reason // "-"')"
  first_selection_reason="$(json_value "$context_json" '.reading_plan[0].selection_reason // "-"')"
  first_selection_rank="$(json_value "$context_json" '.reading_plan[0].selection_rank // 0')"
  estimated_tokens="$(json_value "$context_json" '.estimated_tokens')"
  applied_budget="$(json_value "$context_json" '.budget.applied_token_budget // 0')"
  min_files="$(context_guardrail_value "$specs" "selected_files" "1")"
  min_ranges="$(context_guardrail_value "$specs" "selected_ranges" "1")"
  min_reading_plan_steps="$(context_guardrail_value "$specs" "reading_plan_steps" "1")"
  max_tokens="$(context_guardrail_value "$specs" "max_tokens" "$applied_budget")"
  min_line_reduction="$(context_guardrail_value "$specs" "min_line_reduction" "50")"

  status="pass"
  if [ "$first_recommended_tool" != "context_pack" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "first_recommended_tool" "context_pack" "$first_recommended_tool" "$status"

  status="pass"
  if [ "$context_files" -lt "$min_files" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "selected_files" ">= $min_files" "$context_files" "$status"

  status="pass"
  if [ "$ranges" -lt "$min_ranges" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "selected_ranges" ">= $min_ranges" "$ranges" "$status"

  status="pass"
  if [ "$reading_plan_steps" -lt "$min_reading_plan_steps" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "reading_plan_steps" ">= $min_reading_plan_steps" "$reading_plan_steps" "$status"

  status="pass"
  if [ -z "$first_next_action" ] || [ "$first_next_action" = "-" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "first_next_action" "present" "$first_next_action" "$status"

  status="pass"
  if [ -z "$first_reading_focus" ] || [ "$first_reading_focus" = "-" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "first_reading_focus" "present" "$first_reading_focus" "$status"

  status="pass"
  if [ -z "$first_reading_question" ] || [ "$first_reading_question" = "-" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "first_reading_question" "present" "$first_reading_question" "$status"

  status="pass"
  if [ -z "$first_reading_reason" ] || [ "$first_reading_reason" = "-" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "first_reading_reason" "present" "$first_reading_reason" "$status"

  status="pass"
  if [ "$first_selection_rank" -lt 1 ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "first_selection_rank" ">= 1" "$first_selection_rank" "$status"

  status="pass"
  if [ -z "$first_selection_reason" ] || [ "$first_selection_reason" = "-" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "first_selection_reason" "present" "$first_selection_reason" "$status"

  status="pass"
  if [ "$applied_budget" -le 0 ] || [ "$estimated_tokens" -gt "$applied_budget" ] || [ "$estimated_tokens" -gt "$max_tokens" ]; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "estimated_tokens" "<= $max_tokens and applied budget" "$estimated_tokens / $applied_budget" "$status"

  status="pass"
  if ! percentage_at_least "$total_lines" "$selected_lines" "$min_line_reduction"; then
    status="fail"
  fi
  write_context_guardrail "$output" "$name" "line_reduction" ">= ${min_line_reduction}%" "$(line_reduction "$total_lines" "$selected_lines")" "$status"
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

average_number() {
  local total="$1"
  local count="$2"
  awk -v total="$total" -v count="$count" 'BEGIN {
    if (count <= 0) {
      printf "0"
    } else {
      printf "%.0f", total / count
    }
  }'
}

append_key_results_section() {
  local context_reduction average_tokens average_index_ms

  context_reduction="$(line_reduction "$TOTAL_REPO_LINES" "$TOTAL_CONTEXT_LINES")"
  average_tokens="$(average_number "$TOTAL_CONTEXT_TOKENS" "$BENCHMARKED_REPOS")"
  average_index_ms="$(average_number "$TOTAL_INDEX_MS" "$BENCHMARKED_REPOS")"

  cat >>"$REPORT_FILE" <<EOF

## Key Results

- Repositories benchmarked: $BENCHMARKED_REPOS (\`$(repo_subset_label)\` subset).
- Agent routing: \`context_pack\` was the first recommended tool for $CONTEXT_PACK_FIRST_RECOMMENDATIONS/$BENCHMARKED_REPOS repositories.
- Context compression: selected $TOTAL_CONTEXT_LINES of $TOTAL_REPO_LINES source lines ($context_reduction reduction) across $TOTAL_CONTEXT_FILES files and $TOTAL_CONTEXT_RANGES ranges.
- Token budget: $TOTAL_CONTEXT_TOKENS estimated tokens total, $average_tokens average tokens per repository, with a 6000 token budget per context pack.
- Indexing: $TOTAL_INDEX_MS ms total, $average_index_ms ms average per repository, with $BUDGET_FAILURES budget failures.
- Guardrails: $CONTEXT_GUARDRAIL_FAILURES context, $SYMBOL_TARGET_FAILURES symbol, $CALL_TARGET_FAILURES call target, and $CALL_EDGE_FAILURES call edge failures.
- Truncation: $TRUNCATED_CONTEXT_PACKS context packs reported truncated output.
EOF
}

print_terminal_summary() {
  local context_reduction average_tokens average_index_ms total_failures

  context_reduction="$(line_reduction "$TOTAL_REPO_LINES" "$TOTAL_CONTEXT_LINES")"
  average_tokens="$(average_number "$TOTAL_CONTEXT_TOKENS" "$BENCHMARKED_REPOS")"
  average_index_ms="$(average_number "$TOTAL_INDEX_MS" "$BENCHMARKED_REPOS")"
  total_failures=$((BUDGET_FAILURES + CONTEXT_GUARDRAIL_FAILURES + SYMBOL_TARGET_FAILURES + CALL_TARGET_FAILURES + CALL_EDGE_FAILURES))

  echo "benchmark summary"
  echo "  report: $OUTPUT"
  echo "  repositories: $BENCHMARKED_REPOS ($(repo_subset_label))"
  echo "  context_pack first: $CONTEXT_PACK_FIRST_RECOMMENDATIONS/$BENCHMARKED_REPOS"
  echo "  context lines: $TOTAL_CONTEXT_LINES / $TOTAL_REPO_LINES ($context_reduction reduction)"
  echo "  estimated tokens: $TOTAL_CONTEXT_TOKENS total, $average_tokens average"
  echo "  indexing: $TOTAL_INDEX_MS ms total, $average_index_ms ms average"
  echo "  guardrail failures: $total_failures"
  echo "  truncated context packs: $TRUNCATED_CONTEXT_PACKS"
  echo "next steps"
  echo "  open report: $OUTPUT"
  echo "  inspect: Key Results, Summary, and each Context reading plan table"
  echo "  continue with: file_outline for first files, dependency_graph for imports, impact_analysis before edits"
}

write_summary_json() {
  local context_reduction average_tokens average_index_ms total_failures

  if [ -z "$SUMMARY_JSON" ]; then
    return
  fi

  context_reduction="$(line_reduction "$TOTAL_REPO_LINES" "$TOTAL_CONTEXT_LINES")"
  average_tokens="$(average_number "$TOTAL_CONTEXT_TOKENS" "$BENCHMARKED_REPOS")"
  average_index_ms="$(average_number "$TOTAL_INDEX_MS" "$BENCHMARKED_REPOS")"
  total_failures=$((BUDGET_FAILURES + CONTEXT_GUARDRAIL_FAILURES + SYMBOL_TARGET_FAILURES + CALL_TARGET_FAILURES + CALL_EDGE_FAILURES))

  mkdir -p "$(dirname "$SUMMARY_JSON")"
  jq -n \
    --arg report "$OUTPUT" \
    --arg profile "$BENCH_PROFILE" \
    --arg subset "$(repo_subset_label)" \
    --arg context_reduction "$context_reduction" \
    --arg next_open_report "$OUTPUT" \
    --arg next_inspect "Key Results, Summary, and each Context reading plan table" \
    --arg next_continue "file_outline for first files, dependency_graph for imports, impact_analysis before edits" \
    --argjson repositories "$BENCHMARKED_REPOS" \
    --argjson context_pack_first "$CONTEXT_PACK_FIRST_RECOMMENDATIONS" \
    --argjson total_repo_lines "$TOTAL_REPO_LINES" \
    --argjson total_context_lines "$TOTAL_CONTEXT_LINES" \
    --argjson total_context_tokens "$TOTAL_CONTEXT_TOKENS" \
    --argjson average_context_tokens "$average_tokens" \
    --argjson total_context_files "$TOTAL_CONTEXT_FILES" \
    --argjson total_context_ranges "$TOTAL_CONTEXT_RANGES" \
    --argjson total_index_ms "$TOTAL_INDEX_MS" \
    --argjson average_index_ms "$average_index_ms" \
    --argjson truncated_context_packs "$TRUNCATED_CONTEXT_PACKS" \
    --argjson budget_failures "$BUDGET_FAILURES" \
    --argjson context_guardrail_failures "$CONTEXT_GUARDRAIL_FAILURES" \
    --argjson symbol_target_failures "$SYMBOL_TARGET_FAILURES" \
    --argjson call_target_failures "$CALL_TARGET_FAILURES" \
    --argjson call_edge_failures "$CALL_EDGE_FAILURES" \
    --argjson total_failures "$total_failures" \
    '{
      report: $report,
      profile: $profile,
      repository_subset: $subset,
      repositories: $repositories,
      routing: {
        context_pack_first: $context_pack_first,
        total: $repositories
      },
      context: {
        total_repo_lines: $total_repo_lines,
        selected_lines: $total_context_lines,
        line_reduction: $context_reduction,
        estimated_tokens_total: $total_context_tokens,
        estimated_tokens_average: $average_context_tokens,
        selected_files: $total_context_files,
        selected_ranges: $total_context_ranges,
        truncated_packs: $truncated_context_packs
      },
      indexing: {
        total_ms: $total_index_ms,
        average_ms: $average_index_ms
      },
      failures: {
        total: $total_failures,
        budget: $budget_failures,
        context_guardrail: $context_guardrail_failures,
        symbol_target: $symbol_target_failures,
        call_target: $call_target_failures,
        call_edge: $call_edge_failures
      },
      next_steps: {
        open_report: $next_open_report,
        inspect: $next_inspect,
        continue_with: $next_continue
      }
    }' >"$SUMMARY_JSON"
  echo "wrote summary $SUMMARY_JSON"
}

append_summary_row() {
  local name="$1"
  local language="$2"
  local repo_dir="$3"
  local index_json="$4"
  local overview_json="$5"
  local context_json="$6"
  local max_index_ms="$7"

  local commit files lines symbols skipped errors duration budget db_size entrypoints first_entrypoint recommended_tools first_recommended_tool context_files ranges selected_lines reduction tokens applied_budget omitted_files continuation_status truncated first_context_file status
  commit="$(repo_commit_short "$repo_dir")"
  files="$(json_value "$index_json" '.indexed_files')"
  lines="$(json_value "$overview_json" '[.languages[].lines] | add // 0')"
  symbols="$(json_value "$index_json" '.symbols')"
  skipped="$(json_value "$index_json" '.skipped_files')"
  errors="$(json_value "$index_json" '.errors | length')"
  duration="$(json_value "$index_json" '.duration_ms')"
  budget="$max_index_ms"
  db_size="$(du -h "$repo_dir/.codeinsight/index.db" | awk '{print $1}')"
  entrypoints="$(json_value "$overview_json" '.entrypoints | length')"
  first_entrypoint="$(json_value "$overview_json" '.entrypoints[0].file // "-"')"
  recommended_tools="$(json_value "$overview_json" '.recommended_next_tools | length')"
  first_recommended_tool="$(json_value "$overview_json" '.recommended_next_tools[0].tool // "-"')"
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
  BENCHMARKED_REPOS=$((BENCHMARKED_REPOS + 1))
  TOTAL_REPO_LINES=$((TOTAL_REPO_LINES + lines))
  TOTAL_CONTEXT_LINES=$((TOTAL_CONTEXT_LINES + selected_lines))
  TOTAL_CONTEXT_TOKENS=$((TOTAL_CONTEXT_TOKENS + tokens))
  TOTAL_CONTEXT_FILES=$((TOTAL_CONTEXT_FILES + context_files))
  TOTAL_CONTEXT_RANGES=$((TOTAL_CONTEXT_RANGES + ranges))
  TOTAL_INDEX_MS=$((TOTAL_INDEX_MS + duration))
  if [ "$first_recommended_tool" = "context_pack" ]; then
    CONTEXT_PACK_FIRST_RECOMMENDATIONS=$((CONTEXT_PACK_FIRST_RECOMMENDATIONS + 1))
  fi
  if [ "$truncated" = "true" ]; then
    TRUNCATED_CONTEXT_PACKS=$((TRUNCATED_CONTEXT_PACKS + 1))
  fi

  printf "| %s | %s | \`%s\` | %s | %s | %s | %s | %s | %s | %s | %s | %s | %s | \`%s\` | %s | \`%s\` | %s | %s | %s | %s | %s | %s | %s | %s | %s | \`%s\` |\n" \
    "$name" "$language" "$commit" "$files" "$lines" "$symbols" "$skipped" "$errors" "$duration" "$budget" "$status" "$db_size" \
    "$entrypoints" "$first_entrypoint" "$recommended_tools" "$first_recommended_tool" \
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
  local context_guardrails_file="${13}"
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
    echo "- Commit: \`$(repo_commit_full "$repo_dir")\`"
    echo "- Indexed files: $(json_value "$index_json" '.indexed_files')"
    echo "- Symbols: $(json_value "$index_json" '.symbols')"
    echo "- Duration: $duration ms"
    echo "- Index budget: $max_index_ms ms ($status)"
    echo "- Entrypoint candidates: $(json_value "$overview_json" '.entrypoints | length')"
    echo "- First entrypoint candidate: \`$(json_value "$overview_json" '.entrypoints[0].file // "-"')\`"
    echo "- Recommended next tools: $(json_value "$overview_json" '.recommended_next_tools | length')"
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
    echo "- Context continuation next action: $(json_value "$context_json" '.continuation_summary.next_action // "-"')"
    if jq -e '(.omitted_candidates // []) | length > 0' "$context_json" >/dev/null; then
      echo "- First omitted candidate: \`$(json_value "$context_json" '.omitted_candidates[0].file // "-"')\` (candidate rank $(json_value "$context_json" '.omitted_candidates[0].selection_rank // "-"'))"
      echo "- First omitted reason: $(json_value "$context_json" '.omitted_candidates[0].omission_reason // "-"')"
      echo "- First omitted next action: $(json_value "$context_json" '.omitted_candidates[0].next_action // "-"')"
    else
      echo "- First omitted candidate: none"
    fi
    echo "- Context truncated: $(json_value "$context_json" '.truncated')"
    echo
    echo "Entrypoint candidates:"
    echo
    echo "| File | Symbol | Role | Confidence | Reason |"
    echo "| --- | --- | --- | ---: | --- |"
  } >>"$REPORT_FILE"

  jq -r '
    def clean: tostring | gsub("\\|"; "\\|");
    (.entrypoints[:5] // [])
    | if length == 0 then
        ["| - | - | - | 0 | none |"]
      else
        map("| `" + (.file // "-" | clean) + "` | `" + (.symbol // "-" | clean) + "` | " + (.role // "-" | clean) + " | " + ((.confidence // 0) | tostring) + " | " + (.reason // "-" | clean) + " |")
      end
    | .[]
  ' "$overview_json" >>"$REPORT_FILE"

  {
    echo
    echo "Recommended next tools:"
    echo
    echo "| Tool | Priority | Reason |"
    echo "| --- | ---: | --- |"
  } >>"$REPORT_FILE"

  jq -r '
    def clean: tostring | gsub("\\|"; "\\|");
    (.recommended_next_tools[:5] // [])
    | if length == 0 then
        ["| - | 0 | none |"]
      else
        map("| `" + (.tool // "-" | clean) + "` | " + ((.priority // 0) | tostring) + " | " + (.reason // "-" | clean) + " |")
      end
    | .[]
  ' "$overview_json" >>"$REPORT_FILE"

  {
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
    echo "Context reading plan:"
    echo
    echo "| File | Rank | Focus | Question | Next action | Suggested tool | Reason | Selection reason |"
    echo "| --- | ---: | --- | --- | --- | --- | --- | --- |"
  } >>"$REPORT_FILE"

  jq -r '
    def clean: tostring | gsub("\\|"; "\\|") | gsub("\n"; " ");
    (.reading_plan[:5] // [])
    | if length == 0 then
        ["| - | 0 | none | none | - | - | none | none |"]
      else
        map("| `" + (.file // "-" | clean) + "` | " + ((.selection_rank // 0) | tostring) + " | " + (.focus // "-" | clean) + " | " + (.question // "-" | clean) + " | `" + (.next_action // "-" | clean) + "` | `" + (.suggested_tool.tool // "-" | clean) + "` | " + (.reason // "-" | clean) + " | " + (.selection_reason // "-" | clean) + " |")
      end
    | .[]
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

  if [ -s "$context_guardrails_file" ]; then
    {
      echo
      echo "Context pack guardrails:"
      echo
      echo "| Check | Expectation | Observed | Status |"
      echo "| --- | --- | --- | --- |"
    } >>"$REPORT_FILE"

    while IFS=$'\t' read -r check expectation observed guardrail_status; do
      printf "| \`%s\` | %s | %s | %s |\n" "$check" "$expectation" "$observed" "$guardrail_status" >>"$REPORT_FILE"
    done <"$context_guardrails_file"
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
  validate_repo_subset

  if [ -n "$BENCH_REPOS" ] && [ -z "$OUTPUT_WAS_SET" ]; then
    OUTPUT="$WORK_DIR/results/benchmark-$BENCH_PROFILE-subset.md"
  fi

  if [ "$PRINT_CONFIG" = "1" ]; then
    print_benchmark_config
    exit 0
  fi

  require_command git
  require_command jq
  require_command cargo
  require_command du
  require_command awk
  require_command tar

  mkdir -p "$WORK_DIR/results" "$(dirname "$OUTPUT")"
  REPORT_FILE="$WORK_DIR/results/benchmark-report.md"

  echo "building release binary"
  cargo build --locked --release --manifest-path "$ROOT_DIR/Cargo.toml"
  if [ -z "$CODEINSIGHT_BIN" ]; then
    CODEINSIGHT_BIN="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r '.target_directory')/release/codeinsight"
  fi

  write_report_header

  for i in "${!REPO_NAMES[@]}"; do
    name="${REPO_NAMES[$i]}"
    if ! repo_selected "$name"; then
      continue
    fi

    url="${REPO_URLS[$i]}"
    local_root="${REPO_LOCAL_ROOTS[$i]}"
    language="${REPO_LANGUAGES[$i]}"
    context_file="${REPO_CONTEXT_FILES[$i]}"
    context_task="${REPO_CONTEXT_TASKS[$i]}"
    context_guardrails="${REPO_CONTEXT_GUARDRAILS[$i]}"
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
    context_guardrails_file="$WORK_DIR/results/$name-context-guardrails.tsv"

    echo "benchmarking $name"
    clone_repo "$name" "$url" "$local_root"
    "$CODEINSIGHT_BIN" index "$repo_dir" --force >"$index_json"
    "$CODEINSIGHT_BIN" overview "$repo_dir" >"$overview_json"
    "$CODEINSIGHT_BIN" context-pack "$repo_dir" \
      --task "$context_task" \
      --file "$context_file" \
      --token-budget 6000 \
      >"$context_json"
    validate_context_guardrails "$name" "$overview_json" "$context_json" "$context_guardrails_file" "$context_guardrails"
    validate_symbol_target_guardrails "$name" "$repo_dir" "$symbol_targets" "$symbol_targets_file"
    validate_call_target_guardrails "$name" "$repo_dir" "$call_targets" "$call_targets_file"
    validate_call_edge_guardrails "$name" "$repo_dir" "$call_edges" "$call_edges_file"
    append_summary_row "$name" "$language" "$repo_dir" "$index_json" "$overview_json" "$context_json" "$max_index_ms"
  done

  append_key_results_section

  cat >>"$REPORT_FILE" <<EOF

## Details
EOF

  for i in "${!REPO_NAMES[@]}"; do
    name="${REPO_NAMES[$i]}"
    if ! repo_selected "$name"; then
      continue
    fi

    url="${REPO_URLS[$i]}"
    local_root="${REPO_LOCAL_ROOTS[$i]}"
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
    context_guardrails_file="$WORK_DIR/results/$name-context-guardrails.tsv"
    append_detail_section "$name" "$url" "$repo_dir" "$index_json" "$overview_json" "$context_json" "$context_file" "$context_task" "$max_index_ms" "$symbol_targets_file" "$call_targets_file" "$call_edges_file" "$context_guardrails_file"
  done

  mv "$REPORT_FILE" "$OUTPUT"
  echo "wrote $OUTPUT"
  print_terminal_summary
  write_summary_json
  if [ "$BUDGET_FAILURES" -gt 0 ]; then
    echo "benchmark budget failures: $BUDGET_FAILURES" >&2
    exit 1
  fi
  if [ "$CONTEXT_GUARDRAIL_FAILURES" -gt 0 ]; then
    echo "context pack guardrail failures: $CONTEXT_GUARDRAIL_FAILURES" >&2
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
