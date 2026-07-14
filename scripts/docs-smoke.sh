#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run_step() {
  local index="$1"
  local label="$2"
  shift 2

  echo "[$index/3] $label"
  "$@"
}

main() {
  run_step 1 "docs link smoke" "$ROOT_DIR/scripts/docs-link-smoke.sh"
  run_step 2 "docs positioning smoke" "$ROOT_DIR/scripts/docs-positioning-smoke.sh"
  run_step 3 "docs benchmark smoke" "$ROOT_DIR/scripts/docs-benchmark-smoke.sh"

  echo "docs smoke passed"
}

main "$@"
