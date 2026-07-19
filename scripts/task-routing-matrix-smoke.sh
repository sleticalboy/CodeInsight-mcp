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

  local repo output_dir summary_json
  repo="$TEMP_DIR/repo"
  output_dir="$TEMP_DIR/matrix"
  summary_json="$output_dir/summary.json"
  create_fixture "$repo"

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/task-routing-matrix.sh" "$repo" \
    --output-dir "$output_dir" \
    --token-budget 1600 \
    --task "understand routing behavior" \
    --task "understand authentication behavior" \
    --task "understand application settings" \
    --task "understand startup flow" \
    --task "understand middleware behavior"

  require_jq "$summary_json" '.status == "pass" and .task_count == 5' "matrix summary should pass"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand routing behavior" and .first_file == "src/router.ts")' "routing task should choose router"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand authentication behavior" and .first_file == "src/auth.ts")' "authentication task should choose auth"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand application settings" and .first_file == "src/config.ts")' "settings task should choose config"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand startup flow" and .first_file == "src/startup.ts")' "startup task should choose startup"
  require_jq "$summary_json" '.tasks[] | select(.task == "understand middleware behavior" and .first_file == "src/application.ts")' "middleware task should choose application"

  echo "task routing matrix smoke passed"
  echo "summary: $summary_json"
  jq -r '.tasks[] | "\(.task): \(.first_file) (\(.line_reduction))"' "$summary_json"
}

main "$@"
