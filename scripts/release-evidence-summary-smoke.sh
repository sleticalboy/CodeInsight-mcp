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
  echo "release evidence summary smoke failed: $*" >&2
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

  cat >"$TEMP_DIR/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_EVIDENCE_SMOKE_LOG:?}"
printf 'gh %s\n' "$*" >>"$log"

if [ "$1" = "run" ] && [ "$2" = "list" ]; then
  printf '123456\n'
  exit 0
fi

if [ "$1" = "run" ] && [ "$2" = "view" ]; then
  test "$3" = "123456"
  cat <<'JSON'
{"conclusion":"success","databaseId":123456,"headSha":"abc123","status":"completed","url":"https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456"}
JSON
  exit 0
fi

if [ "$1" = "api" ]; then
  test "$2" = "repos/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts"
  printf '987654\n'
  exit 0
fi

exit 12
EOF
  chmod +x "$TEMP_DIR/bin/gh"

  cat >"$TEMP_DIR/benchmark-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_EVIDENCE_SMOKE_LOG:?}"
printf 'artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--artifact-name"
test "$4" = "codeinsight-benchmark-subset"
test "$5" = "123456"
echo "benchmark artifact smoke passed"
echo "report: /tmp/codeinsight-benchmark-artifact-123456/report.md"
EOF
  chmod +x "$TEMP_DIR/benchmark-artifact-smoke"

  CODEINSIGHT_EVIDENCE_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-evidence-summary.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/output.log"

  grep -Fq 'tag: v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing normalized tag output"
  grep -Fq 'head_sha: abc123' "$TEMP_DIR/output.log" ||
    fail "missing head SHA output"
  grep -Fq 'metadata_cargo: 99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing Cargo metadata output"
  grep -Fq 'metadata_install: v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing install metadata output"
  grep -Fq 'metadata_changelog: 99.88.77 (2026-07-15)' "$TEMP_DIR/output.log" ||
    fail "missing changelog metadata output"
  grep -Fq 'ci_run: 123456' "$TEMP_DIR/output.log" ||
    fail "missing CI run output"
  grep -Fq 'benchmark_artifact_url: https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987654' "$TEMP_DIR/output.log" ||
    fail "missing benchmark artifact URL"
  grep -Fq '## v99.88.77 release evidence' "$TEMP_DIR/output.log" ||
    fail "missing release notes block"
  grep -Fq -- '- CI: [run 123456](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456)' "$TEMP_DIR/output.log" ||
    fail "missing release notes CI link"

  grep -Fq 'gh run list --repo sleticalboy/CodeInsight-mcp --workflow CI --branch main --status success --limit 20 --json databaseId,headSha --jq map(select(.headSha == "abc123"))[0].databaseId // ""' "$TEMP_DIR/calls.log" ||
    fail "missing head SHA CI lookup"
  grep -Fq 'gh run view 123456 --repo sleticalboy/CodeInsight-mcp --json conclusion,databaseId,headSha,status,url' "$TEMP_DIR/calls.log" ||
    fail "missing CI run validation"
  grep -Fq 'gh api repos/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts --jq .artifacts[] | select(.name == "codeinsight-benchmark-subset") | .id' "$TEMP_DIR/calls.log" ||
    fail "missing artifact lookup"
  grep -Fq 'artifact --repo sleticalboy/CodeInsight-mcp --artifact-name codeinsight-benchmark-subset 123456' "$TEMP_DIR/calls.log" ||
    fail "missing benchmark artifact validation"

  mkdir -p "$TEMP_DIR/mismatch/docs"
  cp "$TEMP_DIR/repo/CHANGELOG.md" "$TEMP_DIR/mismatch/CHANGELOG.md"
  cp "$TEMP_DIR/repo/docs/install.md" "$TEMP_DIR/mismatch/docs/install.md"
  cat >"$TEMP_DIR/mismatch/Cargo.toml" <<'EOF'
[package]
name = "codeinsight"
version = "99.88.76"
edition = "2021"
EOF

  if CODEINSIGHT_EVIDENCE_SMOKE_LOG="$TEMP_DIR/mismatch.log" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/mismatch" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-evidence-summary.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/mismatch.out" 2>"$TEMP_DIR/mismatch.err"; then
    fail "version metadata mismatch should fail"
  fi
  grep -Fq 'Cargo.toml version 99.88.76 does not match 99.88.77' "$TEMP_DIR/mismatch.err" ||
    fail "missing version mismatch diagnostic"

  echo "release evidence summary smoke passed"
}

main "$@"
