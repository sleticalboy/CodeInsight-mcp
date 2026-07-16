#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WORKFLOW_DIR="$ROOT_DIR/.github/workflows"

require_pattern() {
  local file="$1"
  local pattern="$2"
  local description="$3"

  if ! grep -Eq -- "$pattern" "$ROOT_DIR/$file"; then
    echo "$file is missing $description" >&2
    echo "expected pattern: $pattern" >&2
    exit 1
  fi
}

forbid_pattern() {
  local pattern="$1"
  local description="$2"

  if grep -RHEq -- "$pattern" "$WORKFLOW_DIR"; then
    echo ".github/workflows contains forbidden $description" >&2
    grep -RHEn -- "$pattern" "$WORKFLOW_DIR" >&2 || true
    exit 1
  fi
}

main() {
  require_pattern ".github/workflows/ci.yml" "concurrency:" "CI concurrency guard"
  require_pattern ".github/workflows/ci.yml" "cancel-in-progress: true" "CI stale run cancellation"
  require_pattern ".github/workflows/ci.yml" "actions/checkout@v7" "CI checkout action v7"
  require_pattern ".github/workflows/ci.yml" "dtolnay/rust-toolchain@stable" "CI Rust toolchain action"
  require_pattern ".github/workflows/ci.yml" "Swatinem/rust-cache@v2" "CI Rust cache action v2"
  require_pattern ".github/workflows/ci.yml" "actions/upload-artifact@v7" "CI artifact upload action v7"
  require_pattern ".github/workflows/ci.yml" "agent-route-smoke:" "agent-route CI job"
  require_pattern ".github/workflows/ci.yml" "scripts/agent-route-smoke\\.sh" "agent-route CI script"
  require_pattern ".github/workflows/ci.yml" "codeinsight-agent-route-smoke" "agent-route CI artifact"
  require_pattern ".github/workflows/ci.yml" "agent-route-step-summary\\.sh" "agent-route CI step summary"
  require_pattern ".github/workflows/ci.yml" "context-pack-quality-smoke:" "context-pack quality CI job"
  require_pattern ".github/workflows/ci.yml" "codeinsight-context-pack-quality" "context-pack quality artifact"
  require_pattern ".github/workflows/ci.yml" "context-pack-quality-step-summary\\.sh" "context-pack quality step summary"

  require_pattern ".github/workflows/release-build.yml" "actions/checkout@v7" "release checkout action v7"
  require_pattern ".github/workflows/release-build.yml" "dtolnay/rust-toolchain@stable" "release Rust toolchain action"
  require_pattern ".github/workflows/release-build.yml" "Swatinem/rust-cache@v2" "release Rust cache action v2"
  require_pattern ".github/workflows/release-build.yml" "actions/upload-artifact@v7" "release artifact upload action v7"
  require_pattern ".github/workflows/release-build.yml" "actions/download-artifact@v8" "release artifact download action v8"
  require_pattern ".github/workflows/release-build.yml" "verify-pretag-ci" "release pretag gate job"
  require_pattern ".github/workflows/release-build.yml" "release-pretag-check\\.sh --repo .* --head-sha" "release pretag head SHA gate"

  require_pattern ".github/workflows/docker-image.yml" "actions/checkout@v7" "Docker checkout action v7"
  require_pattern ".github/workflows/docker-image.yml" "docker/setup-buildx-action@v4" "Docker Buildx setup action v4"
  require_pattern ".github/workflows/docker-image.yml" "docker/login-action@v4" "Docker login action v4"
  require_pattern ".github/workflows/docker-image.yml" "docker/metadata-action@v6" "Docker metadata action v6"
  require_pattern ".github/workflows/docker-image.yml" "docker/build-push-action@v7" "Docker build/push action v7"

  forbid_pattern "actions/checkout@v[1-6]([^0-9]|$)" "checkout action major"
  forbid_pattern "actions/upload-artifact@v[1-6]([^0-9]|$)" "upload-artifact action major"
  forbid_pattern "actions/download-artifact@v[1-7]([^0-9]|$)" "download-artifact action major"
  forbid_pattern "docker/setup-buildx-action@v[1-3]([^0-9]|$)" "Docker Buildx action major"
  forbid_pattern "docker/login-action@v[1-3]([^0-9]|$)" "Docker login action major"
  forbid_pattern "docker/metadata-action@v[1-5]([^0-9]|$)" "Docker metadata action major"
  forbid_pattern "docker/build-push-action@v[1-6]([^0-9]|$)" "Docker build/push action major"

  echo "workflow actions smoke passed"
}

main "$@"
