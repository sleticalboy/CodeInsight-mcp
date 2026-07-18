#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TOTAL=19
TEMP_FILES=()

source "$ROOT_DIR/scripts/smoke-lib.sh"

cleanup() {
  local file

  for file in "${TEMP_FILES[@]}"; do
    rm -f "$file"
  done
}

context_pack_quality_smoke() {
  local summary_json

  summary_json="$(mktemp "${TMPDIR:-/tmp}/codeinsight-context-pack-quality.XXXXXX")"
  TEMP_FILES+=("$summary_json")

  scripts/context-pack-quality-smoke.sh --summary-json "$summary_json"
  jq -e \
    '.status == "pass"
      and .scenarios_passed == 8
      and (.scenarios | length) == 8
      and all(.scenarios[]; .status == "pass")
      and (.scenarios[] | select(.name == "budget_continuation"))
      and (.scenarios[] | select(.name == "minimum_budget"))
      and (.scenarios[] | select(.name == "token_exhaustion"))' \
    "$summary_json" >/dev/null
}

main() {
  trap cleanup EXIT INT TERM
  cd "$ROOT_DIR"

  smoke_run_step "$SMOKE_TOTAL" 1 "cargo fmt" cargo fmt --check
  smoke_run_step "$SMOKE_TOTAL" 2 "cargo test" cargo test --locked
  smoke_run_step "$SMOKE_TOTAL" 3 "script syntax smoke" scripts/script-syntax-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 4 "workflow actions smoke" scripts/workflow-actions-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 5 "benchmark step summary smoke" scripts/benchmark-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 6 "benchmark summary text smoke" scripts/benchmark-summary-text-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 7 "benchmark local smoke" scripts/benchmark-local-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 8 "context pack quality step summary smoke" scripts/context-pack-quality-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 9 "agent route step summary smoke" scripts/agent-route-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 10 "MCP first-call step summary smoke" scripts/mcp-first-call-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 11 "release tooling smoke" scripts/release-tooling-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 12 "docs smoke" scripts/docs-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 13 "context pack quality smoke" context_pack_quality_smoke
  smoke_run_step "$SMOKE_TOTAL" 14 "agent route smoke" scripts/agent-route-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 15 "MCP first-call smoke" scripts/mcp-first-call-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 16 "MCP first-call failure smoke" scripts/mcp-first-call-failure-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 17 "agent router demo" scripts/agent-router-demo.sh
  smoke_run_step "$SMOKE_TOTAL" 18 "framework entrypoint demo" scripts/framework-entrypoint-demo.sh
  smoke_run_step "$SMOKE_TOTAL" 19 "git diff whitespace check" git diff --check

  echo "local CI smoke passed"
}

main "$@"
