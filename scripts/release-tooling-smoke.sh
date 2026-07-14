#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

run_step() {
  local index="$1"
  local label="$2"
  shift 2

  echo "[$index/12] $label"
  "$@"
}

main() {
  run_step 1 "install fallback smoke" "$ROOT_DIR/scripts/install-fallback-smoke.sh"
  run_step 2 "verify release GitHub failure smoke" "$ROOT_DIR/scripts/verify-release-gh-failure-smoke.sh"
  run_step 3 "verify release asset download smoke" "$ROOT_DIR/scripts/verify-release-asset-download-smoke.sh"
  run_step 4 "verify release asset unreachable smoke" "$ROOT_DIR/scripts/verify-release-asset-unreachable-smoke.sh"
  run_step 5 "verify release Docker failure smoke" "$ROOT_DIR/scripts/verify-release-docker-failure-smoke.sh"
  run_step 6 "verify release Homebrew failure smoke" "$ROOT_DIR/scripts/verify-release-homebrew-failure-smoke.sh"
  run_step 7 "verify release help smoke" "$ROOT_DIR/scripts/verify-release-help-smoke.sh"
  run_step 8 "verify release summary smoke" "$ROOT_DIR/scripts/verify-release-summary-smoke.sh"
  run_step 9 "prepare release smoke" "$ROOT_DIR/scripts/prepare-release-smoke.sh"
  run_step 10 "update Homebrew formula smoke" "$ROOT_DIR/scripts/update-homebrew-formula-smoke.sh"
  run_step 11 "post-release verify smoke" "$ROOT_DIR/scripts/post-release-verify-smoke.sh"
  run_step 12 "update release status smoke" "$ROOT_DIR/scripts/update-release-status-smoke.sh"

  echo "release tooling smoke passed"
}

main "$@"
