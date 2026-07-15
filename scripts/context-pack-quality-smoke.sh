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

  echo "context-pack quality smoke passed"
  echo "scenarios: $SCENARIOS_PASSED"
}

main "$@"
