#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

fail() {
  echo "mcp first-call failure smoke failed: $*" >&2
  exit 1
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

expect_failure() {
  local label="$1"
  local expected="$2"
  shift 2

  local stdout_file="$TEMP_DIR/$label.out"
  local stderr_file="$TEMP_DIR/$label.err"
  local exit_code=0

  "$@" >"$stdout_file" 2>"$stderr_file" || exit_code=$?
  if [ "$exit_code" -eq 0 ]; then
    fail "$label should fail"
  fi
  if ! grep -Fq "$expected" "$stderr_file"; then
    echo "stderr:" >&2
    cat "$stderr_file" >&2
    fail "$label did not report expected failure prefix: $expected"
  fi
  if [ -s "$stdout_file" ]; then
    fail "$label should not write stdout on failure"
  fi
}

expect_help() {
  local stdout_file="$TEMP_DIR/help.out"
  local stderr_file="$TEMP_DIR/help.err"

  "$ROOT_DIR/scripts/mcp-first-call-smoke.sh" --help >"$stdout_file" 2>"$stderr_file"
  if [ -s "$stderr_file" ]; then
    echo "stderr:" >&2
    cat "$stderr_file" >&2
    fail "--help should not write stderr"
  fi
  for expected in \
    'usage: scripts/mcp-first-call-smoke.sh [--summary-json PATH] [--help]' \
    '--summary-json PATH' \
    'CODEINSIGHT_BIN' \
    'CODEINSIGHT_FIRST_CALL_ROOT' \
    'CODEINSIGHT_FIRST_CALL_TASK' \
    'CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET' \
    '[agent_route_contract]' \
    '[suggested_tool]'; do
    if ! grep -Fq -- "$expected" "$stdout_file"; then
      echo "stdout:" >&2
      cat "$stdout_file" >&2
      fail "--help is missing: $expected"
    fi
  done
}

expect_external_outline_without_main() {
  local repo="$TEMP_DIR/external-repo"
  local summary="$TEMP_DIR/external-summary.json"

  mkdir -p "$repo/src"
  cat >"$repo/src/router.ts" <<'EOF'
export function routes() {
  return ["/health"];
}

export function registerRoute(path: string) {
  return { path };
}
EOF

  CODEINSIGHT_FIRST_CALL_ROOT="$repo" \
  CODEINSIGHT_FIRST_CALL_TASK="understand route registration behavior" \
  CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET=1600 \
    "$ROOT_DIR/scripts/mcp-first-call-smoke.sh" --summary-json "$summary" >"$TEMP_DIR/external.out" 2>"$TEMP_DIR/external.err" ||
    {
      echo "stderr:" >&2
      cat "$TEMP_DIR/external.err" >&2
      fail "external first-call root without a main symbol should pass"
    }

  jq -e \
    '.status == "pass"
      and .suggested_tool.tool == "file_outline"
      and (.suggested_tool_result_names | index("routes"))
      and (.suggested_tool_result_names | index("main") | not)' \
    "$summary" >/dev/null ||
    fail "external first-call summary should accept non-main outline symbols"
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  expect_help
  expect_external_outline_without_main

  expect_failure \
    invalid-binary \
    'mcp first-call smoke failed [binary]: CODEINSIGHT_BIN is not executable' \
    env CODEINSIGHT_BIN="$TEMP_DIR/not-codeinsight" "$ROOT_DIR/scripts/mcp-first-call-smoke.sh"

  expect_failure \
    unknown-argument \
    'mcp first-call smoke failed [usage]: unknown argument: --bad-option' \
    "$ROOT_DIR/scripts/mcp-first-call-smoke.sh" --bad-option

  expect_failure \
    summary-json-missing-path \
    'mcp first-call smoke failed [usage]: --summary-json requires a path' \
    "$ROOT_DIR/scripts/mcp-first-call-smoke.sh" --summary-json

  if [ ! -x /usr/bin/true ]; then
    fail "/usr/bin/true is unavailable for MCP server failure coverage"
  fi

  expect_failure \
    non-mcp-binary \
    'mcp first-call smoke failed [mcp_server]:' \
    env CODEINSIGHT_BIN=/usr/bin/true "$ROOT_DIR/scripts/mcp-first-call-smoke.sh"

  echo "mcp first-call failure smoke passed"
}

main "$@"
