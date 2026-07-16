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
  echo "archive release evidence smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo"
  cat >"$TEMP_DIR/release-evidence-summary" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_ARCHIVE_EVIDENCE_SMOKE_LOG:?}"
printf 'summary %s\n' "$*" >>"$log"
test "$CODEINSIGHT_ROOT_DIR" = "${CODEINSIGHT_ARCHIVE_EVIDENCE_ROOT:?}"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--run-id"
test "$4" = "123456"
test "$5" = "--head-sha"
test "$6" = "abc123"
test "$7" = "v9.8.7"
test "$8" = "main"
cat <<'SUMMARY'
release evidence summary
tag: v9.8.7
head_sha: abc123
ci_run: 123456
release_notes_block:
## v9.8.7 release evidence
SUMMARY
EOF
  chmod +x "$TEMP_DIR/release-evidence-summary"

  CODEINSIGHT_ARCHIVE_EVIDENCE_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_ARCHIVE_EVIDENCE_ROOT="$TEMP_DIR/repo" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT="$TEMP_DIR/release-evidence-summary" \
    "$ROOT_DIR/scripts/archive-release-evidence.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --run-id 123456 \
      --head-sha abc123 \
      v9.8.7 \
      main >"$TEMP_DIR/output.log"

  grep -Fq "release evidence written: $TEMP_DIR/repo/release-evidence/v9.8.7.md" "$TEMP_DIR/output.log" ||
    fail "missing default archive path output"
  grep -Fq 'next: review' "$TEMP_DIR/output.log" ||
    fail "missing next review hint"
  grep -Fq '## v9.8.7 release evidence' "$TEMP_DIR/repo/release-evidence/v9.8.7.md" ||
    fail "missing archived evidence block"
  grep -Fq 'summary --repo sleticalboy/CodeInsight-mcp --run-id 123456 --head-sha abc123 v9.8.7 main' "$TEMP_DIR/calls.log" ||
    fail "missing evidence summary call"

  if CODEINSIGHT_ARCHIVE_EVIDENCE_SMOKE_LOG="$TEMP_DIR/exists.log" \
    CODEINSIGHT_ARCHIVE_EVIDENCE_ROOT="$TEMP_DIR/repo" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT="$TEMP_DIR/release-evidence-summary" \
    "$ROOT_DIR/scripts/archive-release-evidence.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --run-id 123456 \
      --head-sha abc123 \
      v9.8.7 \
      main >"$TEMP_DIR/exists.out" 2>"$TEMP_DIR/exists.err"; then
    fail "existing archive should fail without --force"
  fi
  grep -Fq "output file already exists: $TEMP_DIR/repo/release-evidence/v9.8.7.md" "$TEMP_DIR/exists.err" ||
    fail "missing existing archive diagnostic"

  CODEINSIGHT_ARCHIVE_EVIDENCE_SMOKE_LOG="$TEMP_DIR/force.log" \
    CODEINSIGHT_ARCHIVE_EVIDENCE_ROOT="$TEMP_DIR/repo" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT="$TEMP_DIR/release-evidence-summary" \
    "$ROOT_DIR/scripts/archive-release-evidence.sh" \
      --force \
      --repo sleticalboy/CodeInsight-mcp \
      --run-id 123456 \
      --head-sha abc123 \
      v9.8.7 \
      main >"$TEMP_DIR/force.out"

  grep -Fq 'release evidence written:' "$TEMP_DIR/force.out" ||
    fail "force overwrite did not write evidence"

  CODEINSIGHT_ARCHIVE_EVIDENCE_SMOKE_LOG="$TEMP_DIR/custom.log" \
    CODEINSIGHT_ARCHIVE_EVIDENCE_ROOT="$TEMP_DIR/repo" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_RELEASE_EVIDENCE_SUMMARY_SCRIPT="$TEMP_DIR/release-evidence-summary" \
    "$ROOT_DIR/scripts/archive-release-evidence.sh" \
      --output "$TEMP_DIR/custom/evidence.md" \
      --repo sleticalboy/CodeInsight-mcp \
      --run-id 123456 \
      --head-sha abc123 \
      v9.8.7 \
      main >"$TEMP_DIR/custom.out"
  grep -Fq 'ci_run: 123456' "$TEMP_DIR/custom/evidence.md" ||
    fail "missing custom output evidence"

  echo "archive release evidence smoke passed"
}

main "$@"
