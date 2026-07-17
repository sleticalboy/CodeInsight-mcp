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
  echo "release pretag check smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/bin"
  cat >"$TEMP_DIR/benchmark-summary.json" <<'JSON'
{
  "routing": {
    "context_pack_first": 1,
    "total": 1
  },
  "context": {
    "line_reduction": "99.0%",
    "truncated_packs": 0
  },
  "failures": {
    "total": 0
  }
}
JSON

  cat >"$TEMP_DIR/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_PRETAG_SMOKE_LOG:?}"
printf 'gh %s\n' "$*" >>"$log"

if [ "$1" = "run" ] && [ "$2" = "list" ]; then
  printf '123456\n'
  exit 0
fi

if [ "$1" = "run" ] && [ "$2" = "view" ]; then
  test "$3" = "123456"
  case " $* " in
    *" --json headSha "*) ;;
    *) exit 13 ;;
  esac
  case " $* " in
    *" --jq .headSha // \"\" "*) ;;
    *) exit 14 ;;
  esac
  printf 'def456\n'
  exit 0
fi

if [ "$1" = "run" ] && [ "$2" = "watch" ]; then
  test "$3" = "123456"
  case " $* " in
    *" --exit-status "*) ;;
    *) exit 11 ;;
  esac
  exit 0
fi

exit 12
EOF
  chmod +x "$TEMP_DIR/bin/gh"

  cat >"$TEMP_DIR/benchmark-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_PRETAG_SMOKE_LOG:?}"
printf 'benchmark-artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "123456"
echo "benchmark artifact smoke passed"
echo "summary: ${CODEINSIGHT_PRETAG_BENCHMARK_SUMMARY:?}"
EOF
  chmod +x "$TEMP_DIR/benchmark-artifact-smoke"

  cat >"$TEMP_DIR/context-pack-quality-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_PRETAG_SMOKE_LOG:?}"
printf 'context-pack-quality-artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "123456"
EOF
  chmod +x "$TEMP_DIR/context-pack-quality-artifact-smoke"

  cat >"$TEMP_DIR/agent-route-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_PRETAG_SMOKE_LOG:?}"
printf 'agent-route-artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "123456"
EOF
  chmod +x "$TEMP_DIR/agent-route-artifact-smoke"

  cat >"$TEMP_DIR/mcp-first-call-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_PRETAG_SMOKE_LOG:?}"
printf 'mcp-first-call-artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "123456"
EOF
  chmod +x "$TEMP_DIR/mcp-first-call-artifact-smoke"

  CODEINSIGHT_PRETAG_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_PRETAG_BENCHMARK_SUMMARY="$TEMP_DIR/benchmark-summary.json" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/agent-route-artifact-smoke" \
    CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-pretag-check.sh" --repo sleticalboy/CodeInsight-mcp main >"$TEMP_DIR/latest.out"

  CODEINSIGHT_PRETAG_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_PRETAG_BENCHMARK_SUMMARY="$TEMP_DIR/benchmark-summary.json" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/agent-route-artifact-smoke" \
    CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-pretag-check.sh" --repo sleticalboy/CodeInsight-mcp --head-sha abc123 main >"$TEMP_DIR/head-sha.out"

  grep -Fq 'gh run list --repo sleticalboy/CodeInsight-mcp --workflow CI --branch main --limit 1 --json databaseId,headSha --jq .[0].databaseId // ""' "$TEMP_DIR/calls.log" ||
    fail "missing latest CI run lookup"
  grep -Fq 'gh run list --repo sleticalboy/CodeInsight-mcp --workflow CI --branch main --status success --limit 20 --json databaseId,headSha --jq map(select(.headSha == "abc123"))[0].databaseId // ""' "$TEMP_DIR/calls.log" ||
    fail "missing successful head SHA CI run lookup"
  grep -Fq 'gh run view 123456 --repo sleticalboy/CodeInsight-mcp --json headSha --jq .headSha // ""' "$TEMP_DIR/calls.log" ||
    fail "missing resolved run head SHA lookup"
  grep -Fq 'gh run watch 123456 --repo sleticalboy/CodeInsight-mcp --exit-status' "$TEMP_DIR/calls.log" ||
    fail "missing CI watch"
  grep -Fq 'benchmark-artifact --repo sleticalboy/CodeInsight-mcp 123456' "$TEMP_DIR/calls.log" ||
    fail "missing benchmark artifact smoke"
  grep -Fq 'context-pack-quality-artifact --repo sleticalboy/CodeInsight-mcp 123456' "$TEMP_DIR/calls.log" ||
    fail "missing context-pack quality artifact smoke"
  grep -Fq 'agent-route-artifact --repo sleticalboy/CodeInsight-mcp 123456' "$TEMP_DIR/calls.log" ||
    fail "missing agent-route artifact smoke"
  grep -Fq 'mcp-first-call-artifact --repo sleticalboy/CodeInsight-mcp 123456' "$TEMP_DIR/calls.log" ||
    fail "missing MCP first-call artifact smoke"
  grep -Fq 'release pretag evidence' "$TEMP_DIR/latest.out" ||
    fail "missing latest evidence heading"
  grep -Fq 'branch: main' "$TEMP_DIR/latest.out" ||
    fail "missing latest branch"
  grep -Fq 'ci_run: 123456' "$TEMP_DIR/latest.out" ||
    fail "missing latest CI run"
  grep -Fq 'head_sha: def456' "$TEMP_DIR/latest.out" ||
    fail "missing latest resolved head SHA"
  grep -Fq 'artifact_gate_benchmark: passed' "$TEMP_DIR/latest.out" ||
    fail "missing benchmark gate summary"
  grep -Fq 'benchmark_context_pack_first: 1/1' "$TEMP_DIR/latest.out" ||
    fail "missing benchmark routing summary"
  grep -Fq 'benchmark_line_reduction: 99.0%' "$TEMP_DIR/latest.out" ||
    fail "missing benchmark line reduction summary"
  grep -Fq 'benchmark_guardrail_failures: 0' "$TEMP_DIR/latest.out" ||
    fail "missing benchmark guardrail summary"
  grep -Fq 'benchmark_truncated_packs: 0' "$TEMP_DIR/latest.out" ||
    fail "missing benchmark truncated packs summary"
  grep -Fq 'artifact_gate_context_pack_quality: passed' "$TEMP_DIR/latest.out" ||
    fail "missing context-pack quality gate summary"
  grep -Fq 'artifact_gate_agent_route: passed' "$TEMP_DIR/latest.out" ||
    fail "missing agent-route gate summary"
  grep -Fq 'artifact_gate_mcp_first_call: passed' "$TEMP_DIR/latest.out" ||
    fail "missing MCP first-call gate summary"
  grep -Fq 'head_sha: abc123' "$TEMP_DIR/head-sha.out" ||
    fail "missing supplied head SHA summary"

  echo "release pretag check smoke passed"
}

main "$@"
