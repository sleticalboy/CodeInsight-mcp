#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "task routing matrix smoke failed: $*" >&2
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

write_file() {
  local path="$1"
  local content="$2"

  mkdir -p "$(dirname "$path")"
  printf "%s\n" "$content" >"$path"
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
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

create_fixture() {
  local repo="$1"

  write_file "$repo/src/main.ts" 'import { createRouter } from "./router";
import { authenticate } from "./auth";
import { loadConfig } from "./config";
import { bootStartup } from "./startup";

export function main() {
  return bootStartup(createRouter(), authenticate("demo"), loadConfig());
}

main();'
  write_file "$repo/src/router.ts" 'export function createRouter() {
  return { route: "/health" };
}'
  write_file "$repo/src/auth.ts" 'export function authenticate(user: string) {
  return { user, status: "accepted" };
}'
  write_file "$repo/src/config.ts" 'export function loadConfig() {
  return { mode: "test" };
}'
  write_file "$repo/src/database.ts" 'export function connectDatabase() {
  // Persist user records in durable storage.
  return { repository: "users", storage: "postgres" };
}'
  write_file "$repo/src/errors.ts" 'export function handleError(error: Error) {
  // Retry timeout failures before falling back to the caller.
  return { retry: true, timeout: error.message.includes("timeout") };
}'
  write_file "$repo/src/router.test.ts" 'import { createRouter } from "./router";

export function routerRegressionSpec() {
  // Regression coverage for router behavior.
  return createRouter();
}'
  write_file "$repo/src/handler.ts" 'export function handleRequest(request: { path: string }) {
  // API endpoint handler returns the response payload.
  return { response: request.path };
}'
  write_file "$repo/src/startup.ts" 'export function bootStartup(router: unknown, auth: unknown, config: unknown) {
  return { router, auth, config };
}'
  write_file "$repo/src/application.ts" 'export function attach(handler: unknown) {
  // Registers middleware before routes are mounted.
  return { handler, stage: "middleware" };
}'
}

main() {
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local repo output_dir summary_json bad_output_dir bad_summary_json expectations_tsv bad_expectations_json
  repo="$TEMP_DIR/repo"
  output_dir="$TEMP_DIR/matrix"
  summary_json="$output_dir/summary.json"
  bad_output_dir="$TEMP_DIR/matrix-bad"
  bad_summary_json="$bad_output_dir/summary.json"
  expectations_tsv="$TEMP_DIR/expectations.tsv"
  bad_expectations_json="$TEMP_DIR/bad-expectations.json"
  create_fixture "$repo"
  write_file "$expectations_tsv" 'understand routing behavior	src/router.ts
understand authentication behavior	src/auth.ts
understand application settings	src/config.ts
understand startup flow	src/startup.ts
understand persistence behavior	src/database.ts
debug retry timeout handling	src/errors.ts
find regression coverage	src/router.test.ts
understand api handler behavior	src/handler.ts
understand middleware behavior	src/application.ts'
  write_file "$bad_expectations_json" '[
  {
    "task": "understand routing behavior",
    "expected_first_file": "src/auth.ts"
  }
]'

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/task-routing-matrix.sh" "$repo" \
    --output-dir "$output_dir" \
    --token-budget 1600 \
    --expect-file "$expectations_tsv"

  require_jq "$summary_json" '.status == "pass" and .task_count == 9' "matrix summary should pass"
  require_jq "$summary_json" '.expectations.status == "pass" and .expectations.count == 9' "matrix expectations should pass"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand routing behavior" and .first_file == "src/router.ts")' "routing task should choose router"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authentication behavior" and .first_file == "src/auth.ts")' "authentication task should choose auth"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand application settings" and .first_file == "src/config.ts")' "settings task should choose config"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand startup flow" and .first_file == "src/startup.ts")' "startup task should choose startup"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand persistence behavior" and .first_file == "src/database.ts")' "persistence task should choose database"
  require_jq "$summary_json" '.tasks[] | select(.task == "debug retry timeout handling" and .first_file == "src/errors.ts")' "debug task should choose errors"
  require_jq "$summary_json" '.tasks[] | select(.task == "find regression coverage" and .first_file == "src/router.test.ts")' "coverage task should choose test"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand api handler behavior" and .first_file == "src/handler.ts")' "api handler task should choose handler"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand middleware behavior" and .first_file == "src/application.ts")' "middleware task should choose application"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authentication behavior" and (.first_reading_question | contains("authentication decisions")))' "authentication task should report an auth-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authentication behavior" and (.first_reading_focus | contains("authentication")))' "authentication task should report an auth-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand application settings" and (.first_reading_question | contains("configuration options")))' "settings task should report a config-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand application settings" and (.first_reading_focus | contains("configuration")))' "settings task should report a config-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand startup flow" and (.first_reading_question | contains("startup entrypoint")))' "startup task should report a startup-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand startup flow" and (.first_reading_focus | contains("startup")))' "startup task should report a startup-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand persistence behavior" and (.first_reading_question | contains("database access")))' "persistence task should report a database-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand persistence behavior" and (.first_reading_focus | contains("persistence")))' "persistence task should report a persistence-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "debug retry timeout handling" and (.first_reading_question | contains("retries")))' "debug task should report an error-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "debug retry timeout handling" and (.first_reading_focus | contains("error handling")))' "debug task should report an error-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "find regression coverage" and (.first_reading_question | contains("regression cases")))' "coverage task should report a test-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "find regression coverage" and (.first_reading_focus | contains("regression coverage")))' "coverage task should report a test-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand api handler behavior" and (.first_reading_question | contains("API requests")))' "api handler task should report a handler-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand api handler behavior" and (.first_reading_focus | contains("API handler")))' "api handler task should report a handler-specific reading focus"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand middleware behavior" and (.first_reading_question | contains("handler boundaries")))' "middleware task should report a middleware-specific reading question"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand middleware behavior" and (.first_reading_focus | contains("middleware")))' "middleware task should report a middleware-specific reading focus"
  grep -Fq '| Task | Seed strategy | First file | Focus | Question |' "$output_dir/task-routing-matrix.md" ||
    fail "matrix markdown should include the Focus column"

  if CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/task-routing-matrix.sh" "$repo" \
    --output-dir "$bad_output_dir" \
    --token-budget 1600 \
    --expect-file "$bad_expectations_json" >/dev/null 2>&1; then
    fail "matrix should fail when an expected first file does not match"
  fi
  require_jq "$bad_summary_json" '.expectations.status == "fail"' "failed matrix should report expectation failure"
  require_jq "$bad_summary_json" '.expectations.checks[] | select(.task == "understand routing behavior" and .expected_first_file == "src/auth.ts" and .actual_first_file == "src/router.ts" and .status == "fail")' "failed matrix should report expected and actual first files"

  echo "task routing matrix smoke passed"
  echo "summary: $summary_json"
  jq -r '.tasks[] | "\(.task): \(.first_file) (\(.line_reduction))"' "$summary_json"
}

main "$@"
