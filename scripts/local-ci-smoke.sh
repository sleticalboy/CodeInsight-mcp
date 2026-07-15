#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TOTAL=10

source "$ROOT_DIR/scripts/smoke-lib.sh"

main() {
  cd "$ROOT_DIR"

  smoke_run_step "$SMOKE_TOTAL" 1 "cargo fmt" cargo fmt --check
  smoke_run_step "$SMOKE_TOTAL" 2 "cargo test" cargo test --locked
  smoke_run_step "$SMOKE_TOTAL" 3 "script syntax smoke" scripts/script-syntax-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 4 "workflow actions smoke" scripts/workflow-actions-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 5 "benchmark step summary smoke" scripts/benchmark-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 6 "release tooling smoke" scripts/release-tooling-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 7 "docs smoke" scripts/docs-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 8 "context pack quality smoke" scripts/context-pack-quality-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 9 "agent router demo" scripts/agent-router-demo.sh
  smoke_run_step "$SMOKE_TOTAL" 10 "git diff whitespace check" git diff --check

  echo "local CI smoke passed"
}

main "$@"
