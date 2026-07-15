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

  mkdir -p "$TEMP_DIR/bin"

  cat >"$TEMP_DIR/bin/git" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG:?}"
printf 'git %s\n' "$*" >>"$log"

if [ "$1" = "-C" ]; then
  shift 2
fi

if [ "$1" = "rev-parse" ] && [ "$2" = "-q" ] && [ "$3" = "--verify" ]; then
  exit 1
fi

if [ "$1" = "ls-remote" ]; then
  if [ "${CODEINSIGHT_TAG_PREFLIGHT_REMOTE_TAG_EXISTS:-0}" = "1" ]; then
    exit 0
  fi
  exit 2
fi

exit 12
EOF
  chmod +x "$TEMP_DIR/bin/git"

  cat >"$TEMP_DIR/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG:?}"
printf 'gh %s\n' "$*" >>"$log"

if [ "$1" = "release" ] && [ "$2" = "view" ]; then
  if [ "${CODEINSIGHT_TAG_PREFLIGHT_RELEASE_EXISTS:-0}" = "1" ]; then
    printf 'v99.88.77\n'
    exit 0
  fi
  echo "release not found" >&2
  exit 1
fi

exit 12
EOF
  chmod +x "$TEMP_DIR/bin/gh"

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
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-tag-preflight.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/output.log"

  grep -Fq 'tag: v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing normalized tag output"
  grep -Fq 'head_sha: abc123' "$TEMP_DIR/output.log" ||
    fail "missing head SHA output"
  grep -Fq 'git ls-remote --exit-code --tags https://github.com/sleticalboy/CodeInsight-mcp.git refs/tags/v99.88.77' "$TEMP_DIR/calls.log" ||
    fail "missing remote tag check"
  grep -Fq 'gh release view v99.88.77 --repo sleticalboy/CodeInsight-mcp --json tagName --jq .tagName' "$TEMP_DIR/calls.log" ||
    fail "missing remote release check"
  grep -Fq 'guard ' "$TEMP_DIR/calls.log" ||
    fail "missing workflow guard call"
  grep -Fq 'pretag --repo sleticalboy/CodeInsight-mcp --head-sha abc123 main' "$TEMP_DIR/calls.log" ||
    fail "missing pretag check call"
  grep -Fq 'next: git tag -a v99.88.77 -m "v99.88.77" && git push origin v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing next tag command"

  if CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG="$TEMP_DIR/remote-tag-exists.log" \
    CODEINSIGHT_TAG_PREFLIGHT_REMOTE_TAG_EXISTS=1 \
    CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT="$TEMP_DIR/release-workflow-guard" \
    CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT="$TEMP_DIR/release-pretag-check" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-tag-preflight.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/remote-tag-exists.out" 2>"$TEMP_DIR/remote-tag-exists.err"; then
    fail "remote tag conflict should fail"
  fi
  grep -Fq 'remote tag already exists: v99.88.77' "$TEMP_DIR/remote-tag-exists.err" ||
    fail "missing remote tag conflict diagnostic"

  if CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG="$TEMP_DIR/release-exists.log" \
    CODEINSIGHT_TAG_PREFLIGHT_RELEASE_EXISTS=1 \
    CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT="$TEMP_DIR/release-workflow-guard" \
    CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT="$TEMP_DIR/release-pretag-check" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-tag-preflight.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/release-exists.out" 2>"$TEMP_DIR/release-exists.err"; then
    fail "remote release conflict should fail"
  fi
  grep -Fq 'remote GitHub Release already exists: v99.88.77' "$TEMP_DIR/release-exists.err" ||
    fail "missing remote release conflict diagnostic"

  echo "release tag preflight smoke passed"
}

main "$@"
