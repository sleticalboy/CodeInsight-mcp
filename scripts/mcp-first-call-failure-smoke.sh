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
    'usage: scripts/mcp-first-call-smoke.sh [--help]' \
    'CODEINSIGHT_BIN' \
    'CODEINSIGHT_FIRST_CALL_ROOT' \
    'CODEINSIGHT_FIRST_CALL_TASK' \
    'CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET' \
    '[agent_route_contract]' \
    '[suggested_tool]'; do
    if ! grep -Fq "$expected" "$stdout_file"; then
      echo "stdout:" >&2
      cat "$stdout_file" >&2
      fail "--help is missing: $expected"
    fi
  done
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  expect_help

  expect_failure \
    invalid-binary \
    'mcp first-call smoke failed [binary]: CODEINSIGHT_BIN is not executable' \
    env CODEINSIGHT_BIN="$TEMP_DIR/not-codeinsight" "$ROOT_DIR/scripts/mcp-first-call-smoke.sh"

  expect_failure \
    unknown-argument \
    'mcp first-call smoke failed [usage]: unknown argument: --bad-option' \
    "$ROOT_DIR/scripts/mcp-first-call-smoke.sh" --bad-option

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
