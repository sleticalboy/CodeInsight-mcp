#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$SMOKE_TEMP_DIR"' EXIT

main() {
  local output="$SMOKE_TEMP_DIR/verify-release-help.txt"

  "$ROOT_DIR/scripts/verify-release.sh" --help >"$output"

  grep -q 'usage: scripts/verify-release.sh \[--json\] <tag-or-version>' "$output"
  grep -q 'Homebrew tap update is still waiting in an open PR' "$output"
  grep -q 'CODEINSIGHT_ALLOW_ASSET_DOWNLOAD_UNREACHABLE=1' "$output"
  grep -q 'CODEINSIGHT_SKIP_INSTALLED_QUICKSTART=1' "$output"

  echo "verify-release help smoke passed"
}

main "$@"
