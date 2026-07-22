#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TOTAL=12

source "$ROOT_DIR/scripts/smoke-lib.sh"

main() {
  smoke_run_step "$SMOKE_TOTAL" 1 "docs link smoke" "$ROOT_DIR/scripts/docs-link-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 2 "docs positioning smoke" "$ROOT_DIR/scripts/docs-positioning-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 3 "docs benchmark smoke" "$ROOT_DIR/scripts/docs-benchmark-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 4 "two-minute demo smoke" "$ROOT_DIR/scripts/two-minute-demo-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 5 "demo output smoke" "$ROOT_DIR/scripts/demo-output-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 6 "local repo evidence smoke" "$ROOT_DIR/scripts/local-repo-evidence-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 7 "adoption comparison smoke" "$ROOT_DIR/scripts/adoption-comparison-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 8 "update adoption case smoke" "$ROOT_DIR/scripts/update-adoption-case-express-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 9 "update self adoption report smoke" "$ROOT_DIR/scripts/update-self-adoption-report-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 10 "adoption evidence smoke" "$ROOT_DIR/scripts/adoption-evidence-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 11 "external beta trial smoke" "$ROOT_DIR/scripts/external-beta-trial-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 12 "adoption report smoke" "$ROOT_DIR/scripts/adoption-report-smoke.sh"

  echo "docs smoke passed"
}

main "$@"
