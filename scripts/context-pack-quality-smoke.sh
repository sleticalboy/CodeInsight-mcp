#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""
SCENARIOS_PASSED=0
QUESTION_CHECKS_PASSED=0
SCENARIO_RESULTS_FILE=""
QUESTION_CHECKS_FILE=""
SUMMARY_JSON=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

fail() {
  echo "context-pack quality smoke failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
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

require_jq() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query" "$file" >/dev/null; then
    echo "query: $query" >&2
    fail "$description"
  fi
}

require_actionable_reading_plan() {
  local file="$1"
  local step_query="$2"
  local description="$3"

  require_jq "$file" \
    "$step_query as \$step
      | (\$step.reason | contains(\"Read this step to answer:\"))
      and (\$step.reason | contains(\$step.question))
      and (\$step.reason | contains(\"If deeper evidence is needed, call \"))
      and (\$step.reason | contains(\$step.suggested_tool.tool))
      and (\$step.reason | contains(\"Selection reason:\"))
      and (\$step.selection_reason | type == \"string\" and length > 0)" \
    "$description"
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

usage() {
  cat <<'EOF'
usage: scripts/context-pack-quality-smoke.sh [--summary-json PATH]

Options:
  --summary-json PATH  Write a machine-readable quality summary JSON.
  -h, --help           Show this help text.
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --summary-json)
        if [ "$#" -lt 2 ]; then
          fail "--summary-json requires a path"
        fi
        SUMMARY_JSON="$2"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fail "unknown argument: $1"
        ;;
    esac
  done
}

record_scenario() {
  local name="$1"
  local file="$2"
  local metrics_query="$3"

  jq -c \
    --arg name "$name" \
    "{
      name: \$name,
      status: \"pass\",
      metrics: (($metrics_query) + (
        if (.read_less | type) == \"object\" then
          {
            baseline_source_lines: .read_less.baseline_source_lines,
            selected_source_lines: .read_less.selected_source_lines,
            source_lines_avoided: .read_less.source_lines_avoided,
            line_reduction: .read_less.line_reduction,
            read_less_ratio: .read_less.read_less_ratio
          }
        else
          {}
        end
      ) + (
        if (.reading_plan | length) > 0 then
          {
            first_reading_focus: .reading_plan[0].focus,
            first_reading_question: .reading_plan[0].question,
            first_reading_reason: .reading_plan[0].reason,
            first_selection_reason: .reading_plan[0].selection_reason
          }
        else
          {}
        end
      ))
    }" \
    "$file" >>"$SCENARIO_RESULTS_FILE"
}

record_question_check() {
  local name="$1"
  local file="$2"
  local step_query="$3"

  jq -c \
    --arg name "$name" \
    "$step_query as \$step
      | {
        name: \$name,
        status: \"pass\",
        file: \$step.file,
        next_action: \$step.next_action,
        focus: \$step.focus,
        question: \$step.question,
        suggested_tool: \$step.suggested_tool.tool
      }" \
    "$file" >>"$QUESTION_CHECKS_FILE"
}

write_summary_json() {
  if [ -z "$SUMMARY_JSON" ]; then
    return
  fi

  mkdir -p "$(dirname "$SUMMARY_JSON")"
  jq -n \
    --arg status "pass" \
    --argjson scenarios_passed "$SCENARIOS_PASSED" \
    --argjson question_checks_passed "$QUESTION_CHECKS_PASSED" \
    --slurpfile scenarios "$SCENARIO_RESULTS_FILE" \
    --slurpfile question_checks "$QUESTION_CHECKS_FILE" \
    '{
      status: $status,
      scenarios_passed: $scenarios_passed,
      scenarios: $scenarios,
      question_checks_passed: $question_checks_passed,
      question_checks: $question_checks
    }' \
    >"$SUMMARY_JSON"

  if ! jq -e \
    --argjson expected "$SCENARIOS_PASSED" \
    --argjson expected_question_checks "$QUESTION_CHECKS_PASSED" \
    '.status == "pass"
      and .scenarios_passed == $expected
      and (.scenarios | length) == $expected
      and all(.scenarios[]; .status == "pass")
      and .question_checks_passed == $expected_question_checks
      and (.question_checks | length) == $expected_question_checks
      and all(.question_checks[]; .status == "pass")' \
    "$SUMMARY_JSON" >/dev/null; then
    fail "summary JSON failed contract validation: $SUMMARY_JSON"
  fi
}

run_polyglot_symbol_scenario() {
  local name="$1"
  local symbol="$2"
  local task="$3"
  local expected_file="$4"
  local output="$5"

  "$CODEINSIGHT_BIN" context-pack "$ROOT_DIR/tests/fixtures/polyglot" \
    --task "$task" \
    --symbol "$symbol" \
    --token-budget 1600 \
    >"$output"

  require_jq "$output" '.seed_strategy == "explicit"' "$symbol should use explicit seed strategy"
  require_jq "$output" ".files[0].file == \"$expected_file\"" "$symbol should rank $expected_file first"
  require_jq "$output" ".reading_plan[0].file == \"$expected_file\"" "$symbol reading plan should start with $expected_file"
  require_jq "$output" '.reading_plan[0].next_action == "inspect_symbol_definition"' "$symbol should inspect the definition first"
  require_jq "$output" '.reading_plan[0].suggested_tool.tool == "file_outline"' "$symbol should suggest file_outline"
  require_actionable_reading_plan "$output" '.reading_plan[0]' \
    "$symbol reading-plan reason should be actionable"
  require_jq "$output" '.budget.applied_token_budget <= 1600' "$symbol should respect token budget"
  require_jq "$output" '.budget.selected_files >= 1' "$symbol should select files"
  require_jq "$output" '.budget.selected_ranges >= 1' "$symbol should select ranges"
  require_jq "$output" '.truncated == false' "$symbol should fit without truncation in fixture"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "$name" "$output" \
    '{first_file: .files[0].file, estimated_tokens, selected_files: .budget.selected_files, selected_ranges: .budget.selected_ranges, first_next_action: .reading_plan[0].next_action, first_suggested_tool: .reading_plan[0].suggested_tool.tool, first_reason_actionable: true, first_selection_reason: .reading_plan[0].selection_reason}'
  echo "  pass: $symbol -> $expected_file ($(json_value "$output" '.estimated_tokens') tokens)"
}

write_ranking_fixture() {
  local root="$1"

  mkdir -p "$root/src"
  cat >"$root/src/core.ts" <<'EOF'
export function leaf() {
  return "ok";
}
EOF
  cat >"$root/src/route.ts" <<'EOF'
import { leaf } from "./core";

export function route() {
  return leaf();
}
EOF
  cat >"$root/src/core.test.ts" <<'EOF'
import { leaf } from "./core";

export function spec() {
  return leaf();
}
EOF
}

write_dependency_continuation_fixture() {
  local root="$1"

  mkdir -p "$root/app"
  cat >"$root/app/main.py" <<'EOF'
from . import support

class Entry:
    def render(self):
        return support.helper()
EOF
  cat >"$root/app/support.py" <<'EOF'
def helper():
    return "ok"
EOF
}

write_budget_continuation_fixture() {
  local root="$1"

  write_feature_budget_fixture "$root" 80
}

write_minimum_budget_fixture() {
  local root="$1"

  mkdir -p "$root/app"
  cat >"$root/app/tiny.py" <<'EOF'
def tiny():
    return "ok"
EOF
}

write_token_exhaustion_fixture() {
  local root="$1"

  write_feature_budget_fixture "$root" 12
}

write_feature_budget_fixture() {
  local root="$1"
  local count="$2"

  mkdir -p "$root/src"
  for index in $(seq 1 "$count"); do
    cat >"$root/src/feature$index.py" <<EOF
def feature_$index():
    value = "$index"
    detail = "feature $index has enough text to consume some token budget for context selection"
    return value + detail
EOF
  done
}

require_file_before() {
  local file="$1"
  local earlier="$2"
  local later="$3"
  local description="$4"

  require_jq "$file" \
    "[.files[].file] as \$files | (\$files | index(\"$earlier\")) != null and (\$files | index(\"$later\")) != null and ((\$files | index(\"$earlier\")) < (\$files | index(\"$later\")))" \
    "$description"
}

run_reference_ranking_scenarios() {
  local project="$TEMP_DIR/ranking"
  local production_json="$TEMP_DIR/production-context.json"
  local test_json="$TEMP_DIR/test-context.json"

  write_ranking_fixture "$project"
  "$CODEINSIGHT_BIN" index "$project" --force >"$TEMP_DIR/ranking-index.json"
  require_jq "$TEMP_DIR/ranking-index.json" '.indexed_files == 3' "ranking fixture should index three files"

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "understand leaf production behavior" \
    --symbol leaf \
    --token-budget 1600 \
    >"$production_json"

  require_file_before "$production_json" "src/route.ts" "src/core.test.ts" \
    "production task should rank production caller before test reference"
  require_jq "$production_json" '.files[] | select(.file == "src/route.ts" and .source == "call_graph")' \
    "production caller should be selected via call_graph"
  require_jq "$production_json" '.reading_plan[] | select(.file == "src/route.ts")' \
    "production caller should appear in reading plan"
  require_actionable_reading_plan "$production_json" \
    '.reading_plan[] | select(.file == "src/route.ts")' \
    "production caller reading-plan reason should be actionable"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "production_reference_ranking" "$production_json" \
    '{first_file: .files[0].file, selected_files: .budget.selected_files, continuation_status: .continuation_summary.status, first_next_action: .reading_plan[0].next_action, first_suggested_tool: .reading_plan[0].suggested_tool.tool, first_reason_actionable: true, first_selection_reason: .reading_plan[0].selection_reason}'
  echo "  pass: production reference ranking"

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "understand leaf test coverage" \
    --symbol leaf \
    --token-budget 1600 \
    >"$test_json"

  require_file_before "$test_json" "src/core.test.ts" "src/route.ts" \
    "test task should rank test reference before production caller"
  require_jq "$test_json" '.files[] | select(.file == "src/core.test.ts" and .source == "call_graph")' \
    "test caller should be selected via call_graph"
  require_jq "$test_json" '.budget.applied_token_budget <= 1600' \
    "test ranking scenario should respect token budget"
  require_actionable_reading_plan "$test_json" \
    '.reading_plan[] | select(.file == "src/core.test.ts")' \
    "test caller reading-plan reason should be actionable"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "test_reference_ranking" "$test_json" \
    '{first_file: .files[0].file, selected_files: .budget.selected_files, continuation_status: .continuation_summary.status, first_next_action: .reading_plan[0].next_action, first_suggested_tool: .reading_plan[0].suggested_tool.tool, first_reason_actionable: true, first_selection_reason: .reading_plan[0].selection_reason}'
  echo "  pass: test reference ranking"
}

run_dependency_continuation_scenario() {
  local project="$TEMP_DIR/dependency-continuation"
  local context_json="$TEMP_DIR/dependency-continuation-context.json"

  write_dependency_continuation_fixture "$project"
  "$CODEINSIGHT_BIN" index "$project" --force >"$TEMP_DIR/dependency-continuation-index.json"
  require_jq "$TEMP_DIR/dependency-continuation-index.json" '.indexed_files == 2' \
    "dependency continuation fixture should index two files"

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "understand local support dependency" \
    --file app/main.py \
    --token-budget 1800 \
    >"$context_json"

  require_jq "$context_json" '.seed_strategy == "explicit"' \
    "dependency continuation should use explicit seed strategy"
  require_jq "$context_json" '.files[] | select(.file == "app/main.py" and .source == "seed_file")' \
    "dependency continuation should include seeded entry file"
  require_jq "$context_json" '.files[] | select(.file == "app/support.py" and .source == "dependency")' \
    "dependency continuation should include resolved local dependency"
  require_jq "$context_json" '.reading_plan[] | select(.file == "app/support.py" and .next_action == "inspect_dependency")' \
    "dependency continuation should mark support file as dependency follow-up"
  require_jq "$context_json" '.reading_plan[] | select(.file == "app/support.py" and .suggested_tool.tool == "dependency_graph" and .suggested_tool.suggested_arguments.files[0] == "app/support.py" and .suggested_tool.suggested_arguments.limit == 100)' \
    "dependency continuation should suggest file-scoped dependency_graph"
  require_actionable_reading_plan "$context_json" \
    '.reading_plan[] | select(.file == "app/support.py")' \
    "dependency reading-plan reason should be actionable"
  require_jq "$context_json" '.budget.applied_token_budget <= 1800' \
    "dependency continuation should respect token budget"
  require_jq "$context_json" '.truncated == false' \
    "dependency continuation should fit without truncation in fixture"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "dependency_continuation" "$context_json" \
    '{selected_files: .budget.selected_files, dependency_file: "app/support.py", dependency_next_action: (.reading_plan[] | select(.file == "app/support.py") | .next_action), dependency_suggested_tool: (.reading_plan[] | select(.file == "app/support.py") | .suggested_tool.tool), dependency_reason_actionable: true, dependency_selection_reason: (.reading_plan[] | select(.file == "app/support.py") | .selection_reason), continuation_status: .continuation_summary.status}'
  echo "  pass: dependency continuation"
}

run_budget_continuation_scenario() {
  local project="$TEMP_DIR/budget-continuation"
  local context_json="$TEMP_DIR/budget-continuation-context.json"
  local args=()

  write_budget_continuation_fixture "$project"
  "$CODEINSIGHT_BIN" index "$project" --force >"$TEMP_DIR/budget-continuation-index.json"
  require_jq "$TEMP_DIR/budget-continuation-index.json" '.indexed_files == 80' \
    "budget continuation fixture should index seed files"

  args=(context-pack "$project" --task "understand feature modules" --token-budget 500)
  for index in $(seq 1 80); do
    args+=(--file "src/feature$index.py")
  done
  "$CODEINSIGHT_BIN" "${args[@]}" >"$context_json"

  require_jq "$context_json" '.seed_strategy == "explicit"' \
    "budget continuation should use explicit seed strategy"
  require_jq "$context_json" '.budget.requested_token_budget == 500 and .budget.applied_token_budget == 500' \
    "budget continuation should preserve the requested low budget"
  require_jq "$context_json" '.budget.candidate_files == 80' \
    "budget continuation should expose candidate file count"
  require_jq "$context_json" '.budget.selected_files < .budget.candidate_files' \
    "budget continuation should omit lower-ranked files"
  require_jq "$context_json" '.budget.omitted_files == (.budget.candidate_files - .budget.selected_files)' \
    "budget continuation omitted file count should match candidate minus selected"
  require_jq "$context_json" '.continuation_summary.status == "omitted_candidates_available"' \
    "budget continuation should expose omitted candidates"
  require_jq "$context_json" '.continuation_summary.next_action == "run_omitted_candidate_context_pack"' \
    "budget continuation should recommend an omitted-candidate follow-up"
  require_jq "$context_json" '.omitted_candidates | length > 0 and length <= 8' \
    "budget continuation should return bounded omitted candidates"
  require_jq "$context_json" '.continuation_summary.omitted_candidate_count == (.omitted_candidates | length)' \
    "budget continuation summary should match omitted candidate count"
  require_jq "$context_json" '.continuation_summary.first_omitted_file == .omitted_candidates[0].file' \
    "budget continuation summary should name first omitted file"
  require_jq "$context_json" '.continuation_summary.suggested_tool.suggested_arguments.files[0] == .omitted_candidates[0].file' \
    "budget continuation summary should point to first omitted file"
  require_jq "$context_json" '.omitted_candidates[0].suggested_tool.tool == "context_pack" and .omitted_candidates[0].suggested_tool.suggested_arguments.token_budget == 4000' \
    "budget continuation omitted candidate should suggest focused context_pack"
  require_jq "$context_json" '.omitted_candidates[0].ranges[0].excerpt == null' \
    "budget continuation omitted candidates should stay excerpt-free"
  require_actionable_reading_plan "$context_json" '.reading_plan[0]' \
    "budget continuation first reading-plan reason should be actionable"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "budget_continuation" "$context_json" \
    '{candidate_files: .budget.candidate_files, selected_files: .budget.selected_files, omitted_files: .budget.omitted_files, omitted_candidates: (.omitted_candidates | length), first_next_action: .reading_plan[0].next_action, first_suggested_tool: .reading_plan[0].suggested_tool.tool, first_reason_actionable: true, continuation_status: .continuation_summary.status}'
  echo "  pass: budget continuation ($(json_value "$context_json" '.budget.selected_files')/$(json_value "$context_json" '.budget.candidate_files') files selected)"
}

run_minimum_budget_scenario() {
  local project="$TEMP_DIR/minimum-budget"
  local context_json="$TEMP_DIR/minimum-budget-context.json"

  write_minimum_budget_fixture "$project"
  "$CODEINSIGHT_BIN" index "$project" --force >"$TEMP_DIR/minimum-budget-index.json"
  require_jq "$TEMP_DIR/minimum-budget-index.json" '.indexed_files == 1' \
    "minimum budget fixture should index one file"

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "understand tiny helper" \
    --file app/tiny.py \
    --token-budget 20 \
    >"$context_json"

  require_jq "$context_json" '.seed_strategy == "explicit"' \
    "minimum budget should use explicit seed strategy"
  require_jq "$context_json" '.budget.requested_token_budget == 20 and .budget.applied_token_budget == 500' \
    "minimum budget should report requested and applied budgets"
  require_jq "$context_json" '.budget.truncation_reason == "minimum_budget_applied"' \
    "minimum budget should report minimum budget truncation reason"
  require_jq "$context_json" '.continuation_summary.status == "minimum_budget_applied"' \
    "minimum budget should expose minimum-budget continuation status"
  require_jq "$context_json" '.continuation_summary.next_action == "read_selected_context"' \
    "minimum budget should recommend reading selected context"
  require_jq "$context_json" '.continuation_summary.omitted_candidate_count == 0 and .continuation_summary.suggested_tool == null' \
    "minimum budget should not suggest omitted follow-up"
  require_jq "$context_json" '.files[0].file == "app/tiny.py" and .budget.selected_files == 1 and .budget.omitted_files == 0' \
    "minimum budget should keep the tiny seed file selected"
  require_jq "$context_json" '.truncated == false' \
    "minimum budget fixture should not truncate after applying minimum budget"
  require_actionable_reading_plan "$context_json" '.reading_plan[0]' \
    "minimum budget reading-plan reason should be actionable"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "minimum_budget" "$context_json" \
    '{requested_token_budget: .budget.requested_token_budget, applied_token_budget: .budget.applied_token_budget, first_next_action: .reading_plan[0].next_action, first_suggested_tool: .reading_plan[0].suggested_tool.tool, first_reason_actionable: true, continuation_status: .continuation_summary.status}'
  echo "  pass: minimum budget ($(json_value "$context_json" '.budget.requested_token_budget')->$(json_value "$context_json" '.budget.applied_token_budget') tokens)"
}

run_token_exhaustion_scenario() {
  local project="$TEMP_DIR/token-exhaustion"
  local context_json="$TEMP_DIR/token-exhaustion-context.json"
  local args=()

  write_token_exhaustion_fixture "$project"
  "$CODEINSIGHT_BIN" index "$project" --force >"$TEMP_DIR/token-exhaustion-index.json"
  require_jq "$TEMP_DIR/token-exhaustion-index.json" '.indexed_files == 12' \
    "token exhaustion fixture should index seed files"

  args=(context-pack "$project" --task "understand feature modules" --token-budget 500)
  for index in $(seq 1 12); do
    args+=(--file "src/feature$index.py")
  done
  "$CODEINSIGHT_BIN" "${args[@]}" >"$context_json"

  require_jq "$context_json" '.seed_strategy == "explicit"' \
    "token exhaustion should use explicit seed strategy"
  require_jq "$context_json" '.budget.requested_token_budget == 500 and .budget.applied_token_budget == 500' \
    "token exhaustion should preserve requested budget"
  require_jq "$context_json" '.budget.candidate_files == 12 and .budget.selected_files == 12 and .budget.omitted_files == 0' \
    "token exhaustion should select every candidate file"
  require_jq "$context_json" '.truncated == true and .budget.truncated == true' \
    "token exhaustion should report truncated ranges"
  require_jq "$context_json" '.budget.truncation_reason == "token_budget_exhausted"' \
    "token exhaustion should report token budget exhausted"
  require_jq "$context_json" '.continuation_summary.status == "token_budget_exhausted"' \
    "token exhaustion should expose token-budget continuation status"
  require_jq "$context_json" '.continuation_summary.next_action == "increase_token_budget_or_narrow_task"' \
    "token exhaustion should recommend increasing budget or narrowing task"
  require_jq "$context_json" '.continuation_summary.omitted_candidate_count == 0 and .continuation_summary.suggested_tool == null' \
    "token exhaustion should not expose omitted-candidate follow-up"
  require_jq "$context_json" '.omitted_candidates | length == 0' \
    "token exhaustion omitted candidates should be empty"
  require_actionable_reading_plan "$context_json" '.reading_plan[0]' \
    "token exhaustion first reading-plan reason should be actionable"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "token_exhaustion" "$context_json" \
    '{selected_files: .budget.selected_files, truncated: .truncated, truncation_reason: .budget.truncation_reason, first_next_action: .reading_plan[0].next_action, first_suggested_tool: .reading_plan[0].suggested_tool.tool, first_reason_actionable: true, continuation_status: .continuation_summary.status}'
  echo "  pass: token exhaustion ($(json_value "$context_json" '.budget.selected_files') files truncated)"
}

write_question_coverage_fixture() {
  local root="$1"

  mkdir -p "$root/src" "$root/app"
  cat >"$root/src/auth.ts" <<'EOF'
export function authenticate() {
  return true;
}
EOF
  cat >"$root/src/router.ts" <<'EOF'
import { authenticate } from "./auth";

export function route() {
  return authenticate();
}
EOF
  cat >"$root/src/tokens.ts" <<'EOF'
export const AUTH_TOKEN = "x-session";
EOF
  cat >"$root/src/session.ts" <<'EOF'
import { AUTH_TOKEN } from "./tokens";

export const sessionHeader = AUTH_TOKEN;
EOF
  cat >"$root/src/auth_notes.py" <<'EOF'
# Session cookie behavior note.
# Refresh cookie expiry should stay aligned with login state.
EOF
  cat >"$root/src/auth_service.py" <<'EOF'
class AuthService:
    def login(self):
        return "ok"
EOF
  cat >"$root/app/main.py" <<'EOF'
from . import support

class Entry:
    def render(self):
        return support.helper()
EOF
  cat >"$root/app/support.py" <<'EOF'
def helper():
    return "ok"
EOF
}

run_question_coverage_scenario() {
  local project="$TEMP_DIR/question-coverage"
  local seed_json="$TEMP_DIR/question-seed-context.json"
  local call_json="$TEMP_DIR/question-call-context.json"
  local reference_json="$TEMP_DIR/question-reference-context.json"
  local dependency_json="$TEMP_DIR/question-dependency-context.json"
  local semantic_json="$TEMP_DIR/question-semantic-context.json"

  write_question_coverage_fixture "$project"
  "$CODEINSIGHT_BIN" index "$project" --force >"$TEMP_DIR/question-coverage-index.json"
  require_jq "$TEMP_DIR/question-coverage-index.json" '.indexed_files == 8' \
    "question coverage fixture should index eight files"

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "understand authentication session behavior" \
    --file src/auth.ts \
    --token-budget 1200 \
    >"$seed_json"
  require_jq "$seed_json" \
    '.reading_plan[0].next_action == "inspect_seed_file"
      and (.reading_plan[0].focus | contains("authentication"))
      and (.reading_plan[0].focus | contains("session"))
      and (.reading_plan[0].question | contains("authentication decisions"))
      and (.reading_plan[0].question | contains("session boundaries"))' \
    "seed reading question should be authentication-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "seed_file_auth_question" "$seed_json" '.reading_plan[0]'

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "understand authentication call path" \
    --symbol authenticate \
    --token-budget 1600 \
    >"$call_json"
  require_jq "$call_json" \
    '.reading_plan[]
      | select(.file == "src/router.ts"
        and .next_action == "follow_call_graph"
        and (.focus | contains("authentication"))
        and (.focus | contains("session"))
        and (.question | contains("authentication decisions"))
        and (.question | contains("session state")))' \
    "call graph reading question should be authentication-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "call_graph_auth_question" "$call_json" \
    '.reading_plan[] | select(.file == "src/router.ts")'

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "understand authentication session token usage" \
    --symbol AUTH_TOKEN \
    --token-budget 1600 \
    >"$reference_json"
  require_jq "$reference_json" \
    '.reading_plan[]
      | select(.file == "src/session.ts"
        and .next_action == "inspect_references"
        and (.focus | contains("authentication"))
        and (.focus | contains("session state"))
        and (.question | contains("authentication decisions"))
        and (.question | contains("session state")))' \
    "reference reading question should be authentication-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "reference_auth_question" "$reference_json" \
    '.reading_plan[] | select(.file == "src/session.ts")'

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "understand authentication support dependency" \
    --file app/main.py \
    --token-budget 1800 \
    >"$dependency_json"
  require_jq "$dependency_json" \
    '.reading_plan[]
      | select(.file == "app/support.py"
        and .next_action == "inspect_dependency"
        and (.focus | contains("authentication"))
        and (.focus | contains("session"))
        and (.question | contains("authentication"))
        and (.question | contains("session boundaries")))' \
    "dependency reading question should be authentication-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "dependency_auth_question" "$dependency_json" \
    '.reading_plan[] | select(.file == "app/support.py")'

  "$CODEINSIGHT_BIN" semantic-index "$project" --chunk-lines 20 \
    >"$TEMP_DIR/question-coverage-semantic-index.json"
  require_jq "$TEMP_DIR/question-coverage-semantic-index.json" '.chunks > 0' \
    "question coverage semantic index should generate local chunks"

  "$CODEINSIGHT_BIN" context-pack "$project" \
    --task "session cookie behavior" \
    --symbol AuthService \
    --token-budget 1600 \
    >"$semantic_json"
  require_jq "$semantic_json" \
    '.reading_plan[]
      | select(.file == "src/auth_notes.py"
        and .next_action == "review_semantic_matches"
        and (.focus | contains("authentication"))
        and (.focus | contains("session"))
        and (.question | contains("cookie"))
        and (.question | contains("session")))' \
    "semantic reading question should be session-cookie-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "semantic_session_cookie_question" "$semantic_json" \
    '.reading_plan[] | select(.file == "src/auth_notes.py")'

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "task_aware_question_coverage" "$semantic_json" \
    "{question_checks: $QUESTION_CHECKS_PASSED, semantic_file: \"src/auth_notes.py\", semantic_next_action: \"review_semantic_matches\", semantic_question: (.reading_plan[] | select(.file == \"src/auth_notes.py\") | .question)}"
  echo "  pass: task-aware question coverage ($QUESTION_CHECKS_PASSED checks)"
}

run_core_analysis_question_scenario() {
  local index_json="$TEMP_DIR/core-analysis-index.json"
  local indexing_json="$TEMP_DIR/core-analysis-indexing-context.json"
  local dependency_json="$TEMP_DIR/core-analysis-dependency-context.json"
  local semantic_json="$TEMP_DIR/core-analysis-semantic-context.json"
  local references_json="$TEMP_DIR/core-analysis-references-context.json"
  local calls_json="$TEMP_DIR/core-analysis-calls-context.json"
  local embedding_status_json="$TEMP_DIR/core-analysis-embedding-status-context.json"

  "$CODEINSIGHT_BIN" index "$ROOT_DIR" --force >"$index_json"
  require_jq "$index_json" '.indexed_files > 0' "core analysis scenario should index this repository"

  "$CODEINSIGHT_BIN" context-pack "$ROOT_DIR" \
    --task "understand indexing pipeline" \
    --token-budget 2600 \
    >"$indexing_json"
  require_jq "$indexing_json" \
    '.reading_plan[0].next_action == "inspect_seed_file"
      and (.reading_plan[0].focus | contains("project indexing"))
      and (.reading_plan[0].question | contains("files scanned"))
      and (.reading_plan[0].question | contains("index records written"))' \
    "indexing pipeline reading question should be core-analysis-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "core_indexing_pipeline_question" "$indexing_json" '.reading_plan[0]'

  "$CODEINSIGHT_BIN" context-pack "$ROOT_DIR" \
    --task "understand dependency graph generation" \
    --token-budget 2600 \
    >"$dependency_json"
  require_jq "$dependency_json" \
    '.reading_plan[0].next_action == "inspect_seed_file"
      and (.reading_plan[0].focus | contains("dependency graph extraction"))
      and (.reading_plan[0].question | contains("dependency edges extracted"))' \
    "dependency graph reading question should be core-analysis-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "core_dependency_graph_question" "$dependency_json" '.reading_plan[0]'

  "$CODEINSIGHT_BIN" context-pack "$ROOT_DIR" \
    --task "understand semantic search fallback" \
    --token-budget 2600 \
    >"$semantic_json"
  require_jq "$semantic_json" \
    '.reading_plan[0].next_action == "inspect_seed_file"
      and (.reading_plan[0].focus | contains("semantic search orchestration"))
      and (.reading_plan[0].question | contains("semantic searches routed"))
      and (.reading_plan[0].question | contains("embedding fallback"))' \
    "semantic search fallback reading question should be core-analysis-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "core_semantic_fallback_question" "$semantic_json" '.reading_plan[0]'

  "$CODEINSIGHT_BIN" context-pack "$ROOT_DIR" \
    --task "understand find references classification" \
    --token-budget 2600 \
    >"$references_json"
  require_jq "$references_json" \
    '.reading_plan[0].next_action == "inspect_seed_file"
      and (.reading_plan[0].focus | contains("reference search"))
      and (.reading_plan[0].question | contains("references found"))
      and (.reading_plan[0].question | contains("usage kinds classified"))' \
    "find references reading question should be core-analysis-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "core_find_references_question" "$references_json" '.reading_plan[0]'

  "$CODEINSIGHT_BIN" context-pack "$ROOT_DIR" \
    --task "understand callers callees call graph traversal" \
    --token-budget 2600 \
    >"$calls_json"
  require_jq "$calls_json" \
    '.reading_plan[0].next_action == "inspect_seed_file"
      and (.reading_plan[0].focus | contains("call graph extraction"))
      and (.reading_plan[0].question | contains("calls extracted"))
      and (.reading_plan[0].question | contains("callers or callees traversed"))' \
    "call graph traversal reading question should be core-analysis-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "core_call_graph_traversal_question" "$calls_json" '.reading_plan[0]'

  "$CODEINSIGHT_BIN" context-pack "$ROOT_DIR" \
    --task "understand embedding provider status reporting" \
    --token-budget 2600 \
    >"$embedding_status_json"
  require_jq "$embedding_status_json" \
    '.reading_plan[0].next_action == "inspect_seed_file"
      and (.reading_plan[0].focus | contains("embedding provider status"))
      and (.reading_plan[0].question | contains("provider status detected"))
      and (.reading_plan[0].question | contains("reported"))' \
    "embedding provider status reading question should be core-analysis-aware"
  QUESTION_CHECKS_PASSED=$((QUESTION_CHECKS_PASSED + 1))
  record_question_check "core_embedding_provider_status_question" "$embedding_status_json" '.reading_plan[0]'

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  record_scenario "core_analysis_question_coverage" "$semantic_json" \
    "{question_checks: $QUESTION_CHECKS_PASSED, semantic_file: .reading_plan[0].file, semantic_next_action: .reading_plan[0].next_action, semantic_question: .reading_plan[0].question}"
  echo "  pass: core analysis question coverage ($QUESTION_CHECKS_PASSED checks)"
}

main() {
  parse_args "$@"
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  SCENARIO_RESULTS_FILE="$TEMP_DIR/scenarios.jsonl"
  QUESTION_CHECKS_FILE="$TEMP_DIR/question-checks.jsonl"
  trap cleanup EXIT INT TERM

  echo "context_pack quality smoke"
  echo "binary: $CODEINSIGHT_BIN"

  "$CODEINSIGHT_BIN" index "$ROOT_DIR/tests/fixtures/polyglot" --force >"$TEMP_DIR/polyglot-index.json"
  require_jq "$TEMP_DIR/polyglot-index.json" '.indexed_files >= 10' "polyglot fixture should index source files"

  run_polyglot_symbol_scenario \
    polyglot_symbol_web_controller \
    WebController \
    "understand dashboard rendering behavior" \
    "src/app.ts" \
    "$TEMP_DIR/web-controller-context.json"
  run_polyglot_symbol_scenario \
    polyglot_symbol_php_service_render \
    PhpService.render \
    "understand PHP render behavior" \
    "src/PhpService.php" \
    "$TEMP_DIR/php-service-context.json"
  run_reference_ranking_scenarios
  run_dependency_continuation_scenario
  run_budget_continuation_scenario
  run_minimum_budget_scenario
  run_token_exhaustion_scenario
  run_question_coverage_scenario
  run_core_analysis_question_scenario

  echo "context-pack quality smoke passed"
  echo "scenarios: $SCENARIOS_PASSED"
  echo "question checks: $QUESTION_CHECKS_PASSED"
  write_summary_json
}

main "$@"
