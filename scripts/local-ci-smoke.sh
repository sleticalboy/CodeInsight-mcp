#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TOTAL=11
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
  smoke_run_step "$SMOKE_TOTAL" 6 "context pack quality step summary smoke" scripts/context-pack-quality-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 7 "release tooling smoke" scripts/release-tooling-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 8 "docs smoke" scripts/docs-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 9 "context pack quality smoke" context_pack_quality_smoke
  smoke_run_step "$SMOKE_TOTAL" 10 "agent router demo" scripts/agent-router-demo.sh
  smoke_run_step "$SMOKE_TOTAL" 11 "git diff whitespace check" git diff --check

  echo "local CI smoke passed"
}

main "$@"
