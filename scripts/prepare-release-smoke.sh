#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$SMOKE_TEMP_DIR"' EXIT

main() {
  local tmp="$SMOKE_TEMP_DIR/release-fixture"
  mkdir -p "$tmp/docs"

  cat >"$tmp/Cargo.toml" <<'EOF'
[package]
name = "codeinsight"
version = "1.2.3"
EOF
  touch "$tmp/Cargo.lock"
  cat >"$tmp/README.md" <<'EOF'
# Test README
EOF
  cat >"$tmp/docs/install.md" <<'EOF'
CODEINSIGHT_VERSION=v1.2.3 sh scripts/install.sh
EOF
  cat >"$tmp/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

### Added

- Test release automation.

## [1.2.3] - 2026-01-01

### Added

- Previous release.
EOF

  CODEINSIGHT_ROOT_DIR="$tmp" \
    CODEINSIGHT_RELEASE_DATE=2026-07-08 \
    CODEINSIGHT_SKIP_CARGO_CHECK=1 \
    "$ROOT_DIR/scripts/prepare-release.sh" --dry-run v9.8.7 >"$tmp/dry-run.out"

  grep -q '^diff --git' "$tmp/dry-run.out"
  grep -q 'version = "9.8.7"' "$tmp/dry-run.out"
  grep -q '## \[9.8.7\] - 2026-07-08' "$tmp/dry-run.out"
  grep -q 'version = "1.2.3"' "$tmp/Cargo.toml"

  CODEINSIGHT_ROOT_DIR="$tmp" \
    CODEINSIGHT_RELEASE_DATE=2026-07-08 \
    CODEINSIGHT_SKIP_CARGO_CHECK=1 \
    "$ROOT_DIR/scripts/prepare-release.sh" v9.8.7

  grep -q 'version = "9.8.7"' "$tmp/Cargo.toml"
  grep -q 'CODEINSIGHT_VERSION=v9.8.7' "$tmp/docs/install.md"
  grep -q '## \[9.8.7\] - 2026-07-08' "$tmp/CHANGELOG.md"

  "$ROOT_DIR/scripts/extract-release-notes.sh" "$tmp/CHANGELOG.md" latest "$tmp/notes.md"
  grep -q 'Test release automation' "$tmp/notes.md"

  "$ROOT_DIR/scripts/extract-release-notes.sh" --summary --max-items 1 "$tmp/CHANGELOG.md" latest "$tmp/summary-notes.md"
  grep -q 'Test release automation' "$tmp/summary-notes.md"
  ! grep -q 'This release has ' "$tmp/summary-notes.md"

  if CODEINSIGHT_ROOT_DIR="$tmp" CODEINSIGHT_SKIP_CARGO_CHECK=1 "$ROOT_DIR/scripts/prepare-release.sh" v9.8.7 >"$tmp/repeat.out" 2>&1; then
    echo "repeat release unexpectedly succeeded" >&2
    exit 1
  fi
  grep -q 'Cargo.toml is already at version 9.8.7' "$tmp/repeat.out"

  if CODEINSIGHT_ROOT_DIR="$tmp" CODEINSIGHT_SKIP_CARGO_CHECK=1 "$ROOT_DIR/scripts/prepare-release.sh" v9.8.6 >"$tmp/low.out" 2>&1; then
    echo "lower release unexpectedly succeeded" >&2
    exit 1
  fi
  grep -q 'must be greater than current Cargo.toml version 9.8.7' "$tmp/low.out"

  local empty="$SMOKE_TEMP_DIR/empty-changelog-fixture"
  mkdir -p "$empty/docs"
  cat >"$empty/Cargo.toml" <<'EOF'
[package]
name = "codeinsight"
version = "1.2.3"
EOF
  touch "$empty/Cargo.lock"
  printf '# Test README\n' >"$empty/README.md"
  printf 'CODEINSIGHT_VERSION=v1.2.3 sh scripts/install.sh\n' >"$empty/docs/install.md"
  cat >"$empty/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

## [1.2.3] - 2026-01-01

### Added

- Previous release.
EOF

  if CODEINSIGHT_ROOT_DIR="$empty" CODEINSIGHT_SKIP_CARGO_CHECK=1 "$ROOT_DIR/scripts/prepare-release.sh" v1.2.4 >"$empty/empty.out" 2>&1; then
    echo "empty changelog release unexpectedly succeeded" >&2
    exit 1
  fi
  grep -q 'CHANGELOG Unreleased section is empty' "$empty/empty.out"
  grep -q 'version = "1.2.3"' "$empty/Cargo.toml"
  grep -q 'CODEINSIGHT_VERSION=v1.2.3' "$empty/docs/install.md"

  CODEINSIGHT_ROOT_DIR="$empty" \
    CODEINSIGHT_ALLOW_EMPTY_CHANGELOG=1 \
    CODEINSIGHT_SKIP_CARGO_CHECK=1 \
    "$ROOT_DIR/scripts/prepare-release.sh" v1.2.4

  grep -q 'version = "1.2.4"' "$empty/Cargo.toml"
  grep -q '## \[1.2.4\] - ' "$empty/CHANGELOG.md"

  echo "prepare release smoke passed"
}

main "$@"
