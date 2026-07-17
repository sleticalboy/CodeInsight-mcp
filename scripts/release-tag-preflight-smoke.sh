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

  mkdir -p "$TEMP_DIR/repo/docs"
  cat >"$TEMP_DIR/repo/Cargo.toml" <<'EOF'
[package]
name = "codeinsight"
version = "99.88.77"
edition = "2021"
EOF
  cat >"$TEMP_DIR/repo/docs/install.md" <<'EOF'
Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | CODEINSIGHT_VERSION=v99.88.77 sh
```
EOF
  cat >"$TEMP_DIR/repo/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

## [99.88.77] - 2026-07-15

- Smoke fixture release notes.
EOF

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
if [ "${CODEINSIGHT_TAG_PREFLIGHT_PRETAG_MISSING_EVIDENCE:-0}" = "1" ]; then
  echo "release pretag check passed"
  exit 0
fi
cat <<'SUMMARY'
release pretag evidence
branch: main
ci_run: 123456
head_sha: abc123
artifact_gate_benchmark: passed
benchmark_context_pack_first: 1/1
benchmark_line_reduction: 99.0%
benchmark_guardrail_failures: 0
benchmark_truncated_packs: 0
artifact_gate_context_pack_quality: passed
artifact_gate_agent_route: passed
artifact_gate_mcp_first_call: passed
release pretag check passed
SUMMARY
EOF
  chmod +x "$TEMP_DIR/release-pretag-check"

    CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
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
  grep -Fq 'metadata_cargo: 99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing Cargo metadata confirmation"
  grep -Fq 'metadata_install: v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing install metadata confirmation"
  grep -Fq 'metadata_changelog: 99.88.77 (2026-07-15)' "$TEMP_DIR/output.log" ||
    fail "missing changelog metadata confirmation"
  grep -Fq 'git ls-remote --exit-code --tags https://github.com/sleticalboy/CodeInsight-mcp.git refs/tags/v99.88.77' "$TEMP_DIR/calls.log" ||
    fail "missing remote tag check"
  grep -Fq 'gh release view v99.88.77 --repo sleticalboy/CodeInsight-mcp --json tagName --jq .tagName' "$TEMP_DIR/calls.log" ||
    fail "missing remote release check"
  grep -Fq 'guard ' "$TEMP_DIR/calls.log" ||
    fail "missing workflow guard call"
  grep -Fq 'pretag --repo sleticalboy/CodeInsight-mcp --head-sha abc123 main' "$TEMP_DIR/calls.log" ||
    fail "missing pretag check call"
  grep -Fq 'release pretag evidence' "$TEMP_DIR/output.log" ||
    fail "missing pretag evidence heading"
  grep -Fq 'ci_run: 123456' "$TEMP_DIR/output.log" ||
    fail "missing pretag CI run"
  grep -Fq 'artifact_gate_benchmark: passed' "$TEMP_DIR/output.log" ||
    fail "missing benchmark artifact gate status"
  grep -Fq 'benchmark_context_pack_first: 1/1' "$TEMP_DIR/output.log" ||
    fail "missing benchmark routing summary"
  grep -Fq 'benchmark_line_reduction: 99.0%' "$TEMP_DIR/output.log" ||
    fail "missing benchmark line reduction summary"
  grep -Fq 'benchmark_guardrail_failures: 0' "$TEMP_DIR/output.log" ||
    fail "missing benchmark guardrail summary"
  grep -Fq 'artifact_gate_context_pack_quality: passed' "$TEMP_DIR/output.log" ||
    fail "missing context-pack quality artifact gate status"
  grep -Fq 'artifact_gate_agent_route: passed' "$TEMP_DIR/output.log" ||
    fail "missing agent-route artifact gate status"
  grep -Fq 'artifact_gate_mcp_first_call: passed' "$TEMP_DIR/output.log" ||
    fail "missing MCP first-call artifact gate status"
  grep -Fq 'next: git tag -a v99.88.77 -m "v99.88.77" && git push origin v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing next tag command"

  if CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG="$TEMP_DIR/missing-evidence.log" \
    CODEINSIGHT_TAG_PREFLIGHT_PRETAG_MISSING_EVIDENCE=1 \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT="$TEMP_DIR/release-workflow-guard" \
    CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT="$TEMP_DIR/release-pretag-check" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-tag-preflight.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/missing-evidence.out" 2>"$TEMP_DIR/missing-evidence.err"; then
    fail "missing pretag evidence should fail"
  fi
  grep -Fq 'release pretag evidence is missing evidence heading' "$TEMP_DIR/missing-evidence.err" ||
    fail "missing pretag evidence diagnostic"

  if CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG="$TEMP_DIR/remote-tag-exists.log" \
    CODEINSIGHT_TAG_PREFLIGHT_REMOTE_TAG_EXISTS=1 \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
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
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
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

  mkdir -p "$TEMP_DIR/mismatch/docs"
  cp "$TEMP_DIR/repo/CHANGELOG.md" "$TEMP_DIR/mismatch/CHANGELOG.md"
  cp "$TEMP_DIR/repo/docs/install.md" "$TEMP_DIR/mismatch/docs/install.md"
  cat >"$TEMP_DIR/mismatch/Cargo.toml" <<'EOF'
[package]
name = "codeinsight"
version = "99.88.76"
edition = "2021"
EOF

  if CODEINSIGHT_TAG_PREFLIGHT_SMOKE_LOG="$TEMP_DIR/mismatch.log" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/mismatch" \
    CODEINSIGHT_RELEASE_WORKFLOW_GUARD_SCRIPT="$TEMP_DIR/release-workflow-guard" \
    CODEINSIGHT_RELEASE_PRETAG_CHECK_SCRIPT="$TEMP_DIR/release-pretag-check" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-tag-preflight.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/mismatch.out" 2>"$TEMP_DIR/mismatch.err"; then
    fail "version metadata mismatch should fail"
  fi
  grep -Fq 'Cargo.toml version 99.88.76 does not match 99.88.77' "$TEMP_DIR/mismatch.err" ||
    fail "missing version mismatch diagnostic"

  echo "release tag preflight smoke passed"
}

main "$@"
