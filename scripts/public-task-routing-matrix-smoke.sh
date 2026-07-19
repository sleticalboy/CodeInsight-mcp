#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "public task routing matrix smoke failed: $*" >&2
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

create_express_like_fixture() {
  local repo="$1"

  write_file "$repo/package.json" '{
  "name": "express-like-route-fixture",
  "main": "index.js"
}'
  write_file "$repo/index.js" 'const factory = require("./lib/express");
const application = require("./lib/application");

module.exports = function startup() {
  return factory.createApplication(application.settings());
};'
  write_file "$repo/lib/express.js" 'exports.createApplication = function createApplication(settings) {
  return createExpressApplicationRoutingBehavior(settings);
};

exports.createExpressApplicationRoutingBehavior = function createExpressApplicationRoutingBehavior(settings) {
  // Express application routing behavior mounts the router and route table.
  return { route: "/health", router: "express", application: "app", settings };
};'
  write_file "$repo/lib/application.js" 'exports.settings = function settings() {
  return { env: "test", middleware: ["logger"] };
};

exports.middleware = function middleware(request, next) {
  return next(request);
};'
}

main() {
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local repo output_dir summary_json
  repo="$TEMP_DIR/repo"
  output_dir="$TEMP_DIR/output"
  summary_json="$output_dir/summary.json"
  create_express_like_fixture "$repo"

  CODEINSIGHT_BIN="$CODEINSIGHT_BIN" "$ROOT_DIR/scripts/public-task-routing-matrix.sh" \
    --case express \
    --root "express=$repo" \
    --output-dir "$output_dir" \
    --token-budget 1600

  require_jq "$summary_json" '.status == "pass" and .case_count == 1' "aggregate summary should pass"
  require_jq "$summary_json" '.aggregate.task_count == 4 and .aggregate.expectation_count == 4' "express expectation count should be aggregated"
  require_jq "$summary_json" '.cases[] | select(.case == "express" and .task_count == 4)' "express case should be present"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand express application routing behavior" and .first_file == "lib/express.js")' "routing task should choose express entry"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand middleware behavior" and .first_file == "lib/application.js")' "middleware task should choose application"
  require_jq "$summary_json" '.cases[].routes[] | select(.task == "understand startup flow" and .first_file == "index.js")' "startup task should choose index"

  echo "public task routing matrix smoke passed"
}

main "$@"
