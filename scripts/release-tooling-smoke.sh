#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

main() {
  "$ROOT_DIR/scripts/install-fallback-smoke.sh"
  "$ROOT_DIR/scripts/verify-release-gh-failure-smoke.sh"
  "$ROOT_DIR/scripts/verify-release-asset-download-smoke.sh"
  "$ROOT_DIR/scripts/verify-release-asset-unreachable-smoke.sh"
  "$ROOT_DIR/scripts/verify-release-docker-failure-smoke.sh"
  "$ROOT_DIR/scripts/verify-release-homebrew-failure-smoke.sh"
  "$ROOT_DIR/scripts/verify-release-help-smoke.sh"
  "$ROOT_DIR/scripts/verify-release-summary-smoke.sh"
  "$ROOT_DIR/scripts/prepare-release-smoke.sh"
  "$ROOT_DIR/scripts/post-release-verify-smoke.sh"
  "$ROOT_DIR/scripts/update-release-status-smoke.sh"

  echo "release tooling smoke passed"
}

main "$@"
