#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run_step() {
  local index="$1"
  local label="$2"
  shift 2

  echo "[$index/6] $label"
  "$@"
}

main() {
  cd "$ROOT_DIR"

  run_step 1 "cargo fmt" cargo fmt --check
  run_step 2 "cargo test" cargo test --locked
  run_step 3 "script syntax smoke" scripts/script-syntax-smoke.sh
  run_step 4 "release tooling smoke" scripts/release-tooling-smoke.sh
  run_step 5 "docs smoke" scripts/docs-smoke.sh
  run_step 6 "git diff whitespace check" git diff --check

  echo "local CI smoke passed"
}

main "$@"
