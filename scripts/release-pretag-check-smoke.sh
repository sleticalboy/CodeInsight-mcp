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
  cat >"$TEMP_DIR/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_PRETAG_SMOKE_LOG:?}"
printf 'gh %s\n' "$*" >>"$log"

if [ "$1" = "run" ] && [ "$2" = "list" ]; then
  printf '123456\n'
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

  CODEINSIGHT_PRETAG_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/agent-route-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-pretag-check.sh" --repo sleticalboy/CodeInsight-mcp main >/dev/null

  CODEINSIGHT_PRETAG_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/agent-route-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-pretag-check.sh" --repo sleticalboy/CodeInsight-mcp --head-sha abc123 main >/dev/null

  grep -Fq 'gh run list --repo sleticalboy/CodeInsight-mcp --workflow CI --branch main --limit 1 --json databaseId,headSha --jq .[0].databaseId // ""' "$TEMP_DIR/calls.log" ||
    fail "missing latest CI run lookup"
  grep -Fq 'gh run list --repo sleticalboy/CodeInsight-mcp --workflow CI --branch main --limit 20 --json databaseId,headSha --jq map(select(.headSha == "abc123"))[0].databaseId // ""' "$TEMP_DIR/calls.log" ||
    fail "missing head SHA CI run lookup"
  grep -Fq 'gh run watch 123456 --repo sleticalboy/CodeInsight-mcp --exit-status' "$TEMP_DIR/calls.log" ||
    fail "missing CI watch"
  grep -Fq 'benchmark-artifact --repo sleticalboy/CodeInsight-mcp 123456' "$TEMP_DIR/calls.log" ||
    fail "missing benchmark artifact smoke"
  grep -Fq 'context-pack-quality-artifact --repo sleticalboy/CodeInsight-mcp 123456' "$TEMP_DIR/calls.log" ||
    fail "missing context-pack quality artifact smoke"
  grep -Fq 'agent-route-artifact --repo sleticalboy/CodeInsight-mcp 123456' "$TEMP_DIR/calls.log" ||
    fail "missing agent-route artifact smoke"

  echo "release pretag check smoke passed"
}

main "$@"
