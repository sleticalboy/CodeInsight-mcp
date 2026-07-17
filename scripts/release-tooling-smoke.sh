#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TOTAL=22

source "$ROOT_DIR/scripts/smoke-lib.sh"

main() {
  smoke_run_step "$SMOKE_TOTAL" 1 "install fallback smoke" "$ROOT_DIR/scripts/install-fallback-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 2 "verify release GitHub failure smoke" "$ROOT_DIR/scripts/verify-release-gh-failure-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 3 "verify release asset download smoke" "$ROOT_DIR/scripts/verify-release-asset-download-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 4 "verify release asset unreachable smoke" "$ROOT_DIR/scripts/verify-release-asset-unreachable-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 5 "verify release Docker failure smoke" "$ROOT_DIR/scripts/verify-release-docker-failure-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 6 "verify release Homebrew failure smoke" "$ROOT_DIR/scripts/verify-release-homebrew-failure-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 7 "verify release help smoke" "$ROOT_DIR/scripts/verify-release-help-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 8 "verify release summary smoke" "$ROOT_DIR/scripts/verify-release-summary-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 9 "prepare release smoke" "$ROOT_DIR/scripts/prepare-release-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 10 "update Homebrew formula smoke" "$ROOT_DIR/scripts/update-homebrew-formula-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 11 "post-release verify smoke" "$ROOT_DIR/scripts/post-release-verify-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 12 "update release status smoke" "$ROOT_DIR/scripts/update-release-status-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 13 "MCP first-call artifact smoke smoke" "$ROOT_DIR/scripts/mcp-first-call-artifact-smoke-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 14 "release pretag check smoke" "$ROOT_DIR/scripts/release-pretag-check-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 15 "release metadata summary smoke" "$ROOT_DIR/scripts/release-metadata-summary-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 16 "release workflow guard smoke" "$ROOT_DIR/scripts/release-workflow-guard-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 17 "release tag preflight smoke" "$ROOT_DIR/scripts/release-tag-preflight-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 18 "release evidence summary smoke" "$ROOT_DIR/scripts/release-evidence-summary-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 19 "release dry run smoke" "$ROOT_DIR/scripts/release-dry-run-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 20 "archive release evidence smoke" "$ROOT_DIR/scripts/archive-release-evidence-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 21 "release handoff summary smoke" "$ROOT_DIR/scripts/release-handoff-summary-smoke.sh"
  smoke_run_step "$SMOKE_TOTAL" 22 "release notes draft smoke" "$ROOT_DIR/scripts/release-notes-draft-smoke.sh"

  echo "release tooling smoke passed"
}

main "$@"
