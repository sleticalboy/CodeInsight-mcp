#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

fail() {
  echo "release metadata summary smoke failed: $*" >&2
  exit 1
}

write_fixture() {
  local dir="$1"
  local version="$2"
  local tag="v$version"

  mkdir -p "$dir/docs"
  cat >"$dir/Cargo.toml" <<EOF
[package]
name = "codeinsight"
version = "$version"
edition = "2021"
EOF
  cat >"$dir/docs/install.md" <<EOF
Install:

\`\`\`bash
CODEINSIGHT_VERSION=$tag sh scripts/install.sh
\`\`\`
EOF
  cat >"$dir/CHANGELOG.md" <<EOF
# Changelog

## [Unreleased]

## [$version] - 2026-07-16

- Smoke fixture release notes.
EOF
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  write_fixture "$TEMP_DIR/repo" "99.88.77"

  CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    "$ROOT_DIR/scripts/release-metadata-summary.sh" v99.88.77 >"$TEMP_DIR/output.log"

  grep -Fq 'metadata_cargo: 99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing Cargo metadata output"
  grep -Fq 'metadata_install: v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing install metadata output"
  grep -Fq 'metadata_changelog: 99.88.77 (2026-07-16)' "$TEMP_DIR/output.log" ||
    fail "missing changelog metadata output"

  write_fixture "$TEMP_DIR/mismatch" "99.88.76"
  if CODEINSIGHT_ROOT_DIR="$TEMP_DIR/mismatch" \
    CODEINSIGHT_RELEASE_METADATA_CONTEXT="release metadata smoke" \
    "$ROOT_DIR/scripts/release-metadata-summary.sh" v99.88.77 \
    >"$TEMP_DIR/mismatch.out" 2>"$TEMP_DIR/mismatch.err"; then
    fail "version mismatch should fail"
  fi
  grep -Fq 'release metadata smoke failed: Cargo.toml version 99.88.76 does not match 99.88.77' "$TEMP_DIR/mismatch.err" ||
    fail "missing context-aware mismatch diagnostic"

  echo "release metadata summary smoke passed"
}

main "$@"
