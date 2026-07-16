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
  echo "release dry run smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo/docs"
  cat >"$TEMP_DIR/repo/Cargo.toml" <<'EOF'
[package]
name = "codeinsight"
version = "1.2.3"
edition = "2021"
EOF
  touch "$TEMP_DIR/repo/Cargo.lock"
  cat >"$TEMP_DIR/repo/README.md" <<'EOF'
# Test README

CODEINSIGHT_VERSION=v1.2.3 sh scripts/install.sh
EOF
  cat >"$TEMP_DIR/repo/docs/install.md" <<'EOF'
CODEINSIGHT_VERSION=v1.2.3 sh scripts/install.sh
EOF
  cat >"$TEMP_DIR/repo/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

### Added

- Test release dry run.

## [1.2.3] - 2026-01-01

### Added

- Previous release.
EOF

  cat >"$TEMP_DIR/release-workflow-guard" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
log="${CODEINSIGHT_DRY_RUN_SMOKE_LOG:?}"
printf 'guard %s\n' "$*" >>"$log"
EOF
  chmod +x "$TEMP_DIR/release-workflow-guard"

  cat >"$TEMP_DIR/release-pretag-check" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
log="${CODEINSIGHT_DRY_RUN_SMOKE_LOG:?}"
printf 'pretag-check %s\n' "$*" >>"$log"
EOF
  chmod +x "$TEMP_DIR/release-pretag-check"

  cat >"$TEMP_DIR/benchmark-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
log="${CODEINSIGHT_DRY_RUN_SMOKE_LOG:?}"
printf 'benchmark-artifact %s\n' "$*" >>"$log"
EOF
  chmod +x "$TEMP_DIR/benchmark-artifact-smoke"

  cat >"$TEMP_DIR/context-pack-quality-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
log="${CODEINSIGHT_DRY_RUN_SMOKE_LOG:?}"
printf 'context-pack-quality-artifact %s\n' "$*" >>"$log"
EOF
  chmod +x "$TEMP_DIR/context-pack-quality-artifact-smoke"

  cat >"$TEMP_DIR/release-tag-preflight" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_DRY_RUN_SMOKE_LOG:?}"
printf 'tag-preflight %s\n' "$*" >>"$log"
test "$CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT" = "${CODEINSIGHT_DRY_RUN_BENCHMARK_ARTIFACT_SMOKE_SCRIPT:?}"
test "$CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" = "${CODEINSIGHT_DRY_RUN_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT:?}"
test "$CODEINSIGHT_ROOT_DIR" != "${CODEINSIGHT_DRY_RUN_SOURCE_ROOT:?}"
grep -q 'version = "9.8.7"' "$CODEINSIGHT_ROOT_DIR/Cargo.toml"
grep -q 'CODEINSIGHT_VERSION=v9.8.7' "$CODEINSIGHT_ROOT_DIR/docs/install.md"
grep -q '## \[9.8.7\] - 2026-07-15' "$CODEINSIGHT_ROOT_DIR/CHANGELOG.md"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--head-sha"
test "$4" = "abc123"
test "$5" = "v9.8.7"
test "$6" = "main"
echo "release tag preflight passed"
EOF
  chmod +x "$TEMP_DIR/release-tag-preflight"

  cat >"$TEMP_DIR/release-evidence-summary" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_DRY_RUN_SMOKE_LOG:?}"
printf 'evidence-summary %s\n' "$*" >>"$log"
test "$CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT" = "${CODEINSIGHT_DRY_RUN_BENCHMARK_ARTIFACT_SMOKE_SCRIPT:?}"
test "$CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" = "${CODEINSIGHT_DRY_RUN_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT:?}"
test "$CODEINSIGHT_ROOT_DIR" != "${CODEINSIGHT_DRY_RUN_SOURCE_ROOT:?}"
grep -q 'version = "9.8.7"' "$CODEINSIGHT_ROOT_DIR/Cargo.toml"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--head-sha"
test "$4" = "abc123"
test "$5" = "v9.8.7"
test "$6" = "main"
cat <<'SUMMARY'
release evidence summary
tag: v9.8.7
ci_run: 123456
release_notes_block:
## v9.8.7 release evidence
SUMMARY
EOF
  chmod +x "$TEMP_DIR/release-evidence-summary"

    CODEINSIGHT_DRY_RUN_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_DRY_RUN_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_DRY_RUN_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    CODEINSIGHT_DRY_RUN_SOURCE_ROOT="$TEMP_DIR/repo" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_RELEASE_DATE=2026-07-15 \
    CODEINSIGHT_RELEASE_TAG_PREFLIGHT_SCRIPT="$TEMP_DIR/release-tag-preflight" \
    CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT="$TEMP_DIR/release-evidence-summary" \
    CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT="$TEMP_DIR/release-workflow-guard" \
    CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT="$TEMP_DIR/release-pretag-check" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    "$ROOT_DIR/scripts/release-dry-run.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      --evidence-file "$TEMP_DIR/evidence/release-evidence.md" \
      v9.8.7 \
      main >"$TEMP_DIR/output.log"

  grep -Fq 'release dry run' "$TEMP_DIR/output.log" ||
    fail "missing dry run header"
  grep -Fq '[1/4] prepare release diff' "$TEMP_DIR/output.log" ||
    fail "missing prepare diff step"
  grep -Fq 'version = "9.8.7"' "$TEMP_DIR/output.log" ||
    fail "missing dry-run version diff"
  grep -Fq '[3/4] release tag preflight' "$TEMP_DIR/output.log" ||
    fail "missing tag preflight step"
  grep -Fq 'release tag preflight passed' "$TEMP_DIR/output.log" ||
    fail "missing tag preflight output"
  grep -Fq '[4/4] release evidence summary' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary step"
  grep -Fq '## v9.8.7 release evidence' "$TEMP_DIR/output.log" ||
    fail "missing evidence block"
  grep -Fq "release evidence written: $TEMP_DIR/evidence/release-evidence.md" "$TEMP_DIR/output.log" ||
    fail "missing evidence file output"
  grep -Fq 'release dry run passed' "$TEMP_DIR/output.log" ||
    fail "missing success output"

  grep -Fq 'release evidence summary' "$TEMP_DIR/evidence/release-evidence.md" ||
    fail "missing evidence file summary"
  grep -Fq '## v9.8.7 release evidence' "$TEMP_DIR/evidence/release-evidence.md" ||
    fail "missing evidence file block"

  grep -Fq 'tag-preflight --repo sleticalboy/CodeInsight-mcp --head-sha abc123 v9.8.7 main' "$TEMP_DIR/calls.log" ||
    fail "missing tag preflight call"
  grep -Fq 'evidence-summary --repo sleticalboy/CodeInsight-mcp --head-sha abc123 v9.8.7 main' "$TEMP_DIR/calls.log" ||
    fail "missing evidence summary call"

  grep -q 'version = "1.2.3"' "$TEMP_DIR/repo/Cargo.toml" ||
    fail "source Cargo.toml was modified"
  grep -q 'CODEINSIGHT_VERSION=v1.2.3' "$TEMP_DIR/repo/docs/install.md" ||
    fail "source install docs were modified"

  echo "release dry run smoke passed"
}

main "$@"
