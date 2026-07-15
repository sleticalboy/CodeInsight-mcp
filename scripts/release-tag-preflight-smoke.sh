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
  echo "release tag preflight smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  cat >"$TEMP_DIR/release-workflow-guard" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG:?}"
printf 'guard %s\n' "$*" >>"$log"
EOF
  chmod +x "$TEMP_DIR/release-workflow-guard"

  cat >"$TEMP_DIR/release-pretag-check" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG:?}"
printf 'pretag %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--head-sha"
test "$4" = "abc123"
test "$5" = "main"
EOF
  chmod +x "$TEMP_DIR/release-pretag-check"

  CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT="$TEMP_DIR/release-workflow-guard" \
    CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT="$TEMP_DIR/release-pretag-check" \
    "$ROOT_DIR/scripts/release-tag-preflight.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/output.log"

  grep -Fq 'tag: v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing normalized tag output"
  grep -Fq 'head_sha: abc123' "$TEMP_DIR/output.log" ||
    fail "missing head SHA output"
  grep -Fq 'guard ' "$TEMP_DIR/calls.log" ||
    fail "missing workflow guard call"
  grep -Fq 'pretag --repo sleticalboy/CodeInsight-mcp --head-sha abc123 main' "$TEMP_DIR/calls.log" ||
    fail "missing pretag check call"
  grep -Fq 'next: git tag -a v99.88.77 -m "v99.88.77" && git push origin v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing next tag command"

  echo "release tag preflight smoke passed"
}

main "$@"
