#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TOTAL=3

source "$ROOT_DIR/scripts/smoke-lib.sh"

main() {
  smoke_run_step "$SMOKE_TOTAL" 1 "docs link smoke" "$ROOT_DIR/scripts/docs-link-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 2 "docs positioning smoke" "$ROOT_DIR/scripts/docs-positioning-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 3 "docs benchmark smoke" "$ROOT_DIR/scripts/docs-benchmark-smoke.sh"

  echo "docs smoke passed"
}

main "$@"
