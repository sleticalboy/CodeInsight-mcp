#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$SMOKE_TEMP_DIR"' EXIT

main() {
  local latest_version="$("$ROOT_DIR/scripts/latest-changelog-version.sh" "$ROOT_DIR/CHANGELOG.md")"
  local latest_notes="$SMOKE_TEMP_DIR/release-notes.md"
  local explicit_notes="$SMOKE_TEMP_DIR/release-notes-explicit.md"
  local summary_notes="$SMOKE_TEMP_DIR/release-notes-summary.md"

  test -n "$latest_version"

  "$ROOT_DIR/scripts/extract-release-notes.sh" "$ROOT_DIR/CHANGELOG.md" latest "$latest_notes"
  "$ROOT_DIR/scripts/extract-release-notes.sh" "$ROOT_DIR/CHANGELOG.md" "$latest_version" "$explicit_notes"
  "$ROOT_DIR/scripts/extract-release-notes.sh" --summary --max-items 2 "$ROOT_DIR/CHANGELOG.md" "$latest_version" "$summary_notes"

  cmp "$latest_notes" "$explicit_notes"
  ! grep -Eq '^## \[[0-9]+\.[0-9]+\.[0-9]+\]' "$latest_notes"
  ! grep -Eq '^## \[Unreleased\]' "$latest_notes"
  grep -q '^### Highlights' "$summary_notes"
  grep -q 'This release has ' "$summary_notes"
  test "$(grep -c '^- ' "$summary_notes")" -le 2

  echo "release notes smoke passed"
}

main "$@"
