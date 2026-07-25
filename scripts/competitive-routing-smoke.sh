#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "competitive routing smoke failed: $*" >&2
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

write_file() {
  local path="$1"
  local content="$2"

  mkdir -p "$(dirname "$path")"
  printf "%s\n" "$content" >"$path"
}

build_binary_if_needed() {
  if [ -z "$CODEINSIGHT_BIN" ]; then
    require_command cargo
    require_command jq
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml" >/dev/null
    CODEINSIGHT_BIN="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r '.target_directory')/release/codeinsight"
  fi

  if [ ! -x "$CODEINSIGHT_BIN" ]; then
    fail "CODEINSIGHT_BIN is not executable: $CODEINSIGHT_BIN"
  fi
}

create_fixture() {
  local repo="$1"

  write_file "$repo/src/main.ts" 'import { createRouter } from "./router";
import { authenticate } from "./auth";
import { loadConfig } from "./config";

export function main() {
  return createRouter(authenticate("demo"), loadConfig());
}'
  write_file "$repo/src/router.ts" 'export function createRouter(auth: unknown, config: unknown) {
  // Routing behavior owns HTTP path dispatch and route registration.
  return { route: "/login", auth, config };
}'
  write_file "$repo/src/auth.ts" 'export function authenticate(user: string) {
  // Authentication behavior validates credentials and session ownership.
  return { user, status: "accepted" };
}'
  write_file "$repo/src/config.ts" 'export function loadConfig() {
  // Runtime application settings are loaded before route registration.
  return { mode: "test" };
}'
  write_file "$repo/src/auth.test.ts" 'import { authenticate } from "./auth";

export function authenticationRegressionSpec() {
  return authenticate("demo");
}'
}

require_jq() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query" "$file" >/dev/null; then
    echo "query: $query" >&2
    fail "$description"
  fi
}

write_report() {
  local summary="$1"
  local report="$2"

  {
    echo "# Competitive Routing Smoke"
    echo
    echo "This deterministic fixture records the CodeInsight side of an"
    echo "agent-first routing comparison. A competitor export can be added later"
    echo "without changing the CodeInsight success criteria."
    echo
    echo "| Tool | Task | First File | Expected | Match | Routed Lines | Baseline Lines | Read Less | Quality | Suggested Tool | Impact Risk |"
    echo "| --- | --- | --- | --- | --- | ---: | ---: | --- | --- | --- | --- |"
    jq -r '
      .tasks[]
      | [
          "CodeInsight",
          .task,
          .first_file,
          .expected_first_file,
          (if .first_file == .expected_first_file then "yes" else "no" end),
          (.selected_lines | tostring),
          (.total_lines | tostring),
          .read_less_ratio,
          (.route_quality_level + " / " + (.route_quality_score | tostring)),
          .first_suggested_tool,
          .risk_level
        ]
      | @tsv
    ' "$summary" | while IFS=$'\t' read -r tool task first_file expected match selected total ratio quality suggested_tool risk_level; do
      printf '| %s | `%s` | `%s` | `%s` | %s | %s | %s | `%s` | `%s` | `%s` | `%s` |\n' \
        "$tool" "$task" "$first_file" "$expected" "$match" "$selected" "$total" "$ratio" "$quality" "$suggested_tool" "$risk_level"
    done
    echo
    echo "## Route Quality Evidence"
    echo
    jq -r '
      .tasks[]
      | "- `" + .task + "`: " + .route_quality_decision_summary
        + " Confidence: " + (.route_quality_confidence_factors[0] // "-")
        + " Verification: " + (.route_quality_verification_steps[0] // "-")
        + " Warnings: " + (if (.route_quality_warnings | length) == 0 then "-" else (.route_quality_warnings | join(" | ")) end)
    ' "$summary"
  } >"$report"
}

main() {
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local repo output_dir matrix_summary summary report
  repo="$TEMP_DIR/repo"
  output_dir="$TEMP_DIR/matrix"
  matrix_summary="$output_dir/summary.json"
  summary="$TEMP_DIR/competitive-summary.json"
  report="$TEMP_DIR/competitive-routing.md"

  create_fixture "$repo"

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/task-routing-matrix.sh" "$repo" \
    --output-dir "$output_dir" \
    --token-budget 1600 \
    --expect "understand routing behavior=src/router.ts" \
    --expect "understand authentication behavior=src/auth.ts" \
    --expect "understand application settings=src/config.ts" \
    >/dev/null

  require_jq "$matrix_summary" '.status == "pass"' "task matrix should pass"
  require_jq "$matrix_summary" '.expectations.status == "pass" and .expectations.count == 3' "task matrix expectations should pass"

  jq '
  (.expectations.checks) as $checks
  | {
    status: "pass",
    scope: "agent_first_read_routing",
    competitor_required: false,
    metrics: {
      task_count: .task_count,
      expectation_count: .expectations.count,
      matched_expectations: ([.expectations.checks[] | select(.status == "pass")] | length),
      total_lines: ([.tasks[].total_lines] | add // 0),
      selected_lines: .aggregate.total_selected_lines,
      line_reduction: (
        ([.tasks[].total_lines] | add // 0) as $total
        | .aggregate.total_selected_lines as $selected
        | if $total <= 0 then "n/a"
          else (((1 - ($selected / $total)) * 100 * 10 | round) / 10 | tostring) + "%"
          end
      ),
      estimated_tokens: .aggregate.total_estimated_tokens
    },
    tasks: [
      .tasks[]
      | . as $task
      | {
          task,
          first_file,
          expected_first_file: (($checks[] | select(.task == $task.task) | .expected_first_file) // ""),
          total_lines,
          selected_lines,
          line_reduction,
          read_less_ratio,
          reading_plan_steps,
          first_suggested_tool,
          route_quality_level,
          route_quality_score,
          route_quality_decision_summary,
          route_quality_confidence_factors,
          route_quality_verification_steps,
          route_quality_warnings,
          risk_level,
          impacted_files,
          first_selection_rank,
          first_reading_focus,
          first_reading_question
        }
    ]
  }' "$matrix_summary" >"$summary"

  require_jq "$summary" '.metrics.task_count == 3 and .metrics.matched_expectations == 3' "competitive summary should preserve expectation metrics"
  require_jq "$summary" '.tasks[] | select(.task == "understand routing behavior" and .first_file == "src/router.ts" and .first_suggested_tool == "file_outline")' "routing comparison row should preserve first file and tool"
  require_jq "$summary" '.tasks[] | select(.task == "understand authentication behavior" and .first_file == "src/auth.ts" and (.risk_level | type == "string" and length > 0))' "authentication comparison row should preserve impact risk"
  require_jq "$summary" '.tasks[] | select(.route_quality_level == "high" and .route_quality_score == 100 and (.route_quality_decision_summary | type == "string" and length > 0) and (.route_quality_verification_steps | length > 0))' "competitive summary should preserve route quality evidence"

  write_report "$summary" "$report"
  grep -Fq '| CodeInsight | `understand routing behavior` | `src/router.ts` | `src/router.ts` | yes |' "$report" ||
    fail "report should include routing comparison row"
  grep -Fq '| CodeInsight | `understand authentication behavior` | `src/auth.ts` | `src/auth.ts` | yes |' "$report" ||
    fail "report should include authentication comparison row"
  grep -Fq '| CodeInsight | `understand application settings` | `src/config.ts` | `src/config.ts` | yes |' "$report" ||
    fail "report should include config comparison row"
  grep -Fq '| `high / 100` | `file_outline` | `high` |' "$report" ||
    fail "report should include route quality evidence"
  grep -Fq 'No route-quality warnings were raised.' "$report" ||
    fail "report should include route quality warning summary"

  echo "competitive routing smoke passed"
  echo "summary: $summary"
  echo "report: $report"
}

main "$@"
