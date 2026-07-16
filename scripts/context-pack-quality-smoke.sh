#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""
SCENARIOS_PASSED=0

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

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

run_polyglot_symbol_scenario() {
  local symbol="$1"
  local task="$2"
  local expected_file="$3"
  local output="$4"

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
  require_jq "$output" '.budget.applied_token_budget <= 1600' "$symbol should respect token budget"
  require_jq "$output" '.budget.selected_files >= 1' "$symbol should select files"
  require_jq "$output" '.budget.selected_ranges >= 1' "$symbol should select ranges"
  require_jq "$output" '.truncated == false' "$symbol should fit without truncation in fixture"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
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

  mkdir -p "$root/src"
  for index in $(seq 1 80); do
    cat >"$root/src/feature$index.py" <<EOF
def feature_$index():
    value = "$index"
    detail = "feature $index has enough text to consume some token budget for context selection"
    return value + detail
EOF
  done
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

  mkdir -p "$root/src"
  for index in $(seq 1 12); do
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

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
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

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
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
  require_jq "$context_json" '.budget.applied_token_budget <= 1800' \
    "dependency continuation should respect token budget"
  require_jq "$context_json" '.truncated == false' \
    "dependency continuation should fit without truncation in fixture"

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
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

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
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

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
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

  SCENARIOS_PASSED=$((SCENARIOS_PASSED + 1))
  echo "  pass: token exhaustion ($(json_value "$context_json" '.budget.selected_files') files truncated)"
}

main() {
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  echo "context_pack quality smoke"
  echo "binary: $CODEINSIGHT_BIN"

  "$CODEINSIGHT_BIN" index "$ROOT_DIR/tests/fixtures/polyglot" --force >"$TEMP_DIR/polyglot-index.json"
  require_jq "$TEMP_DIR/polyglot-index.json" '.indexed_files >= 10' "polyglot fixture should index source files"

  run_polyglot_symbol_scenario \
    WebController \
    "understand dashboard rendering behavior" \
    "src/app.ts" \
    "$TEMP_DIR/web-controller-context.json"
  run_polyglot_symbol_scenario \
    PhpService.render \
    "understand PHP render behavior" \
    "src/PhpService.php" \
    "$TEMP_DIR/php-service-context.json"
  run_reference_ranking_scenarios
  run_dependency_continuation_scenario
  run_budget_continuation_scenario
  run_minimum_budget_scenario
  run_token_exhaustion_scenario

  echo "context-pack quality smoke passed"
  echo "scenarios: $SCENARIOS_PASSED"
}

main "$@"
