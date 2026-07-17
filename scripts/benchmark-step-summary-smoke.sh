#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT_FILE="${1:-$ROOT_DIR/docs/benchmark-v0.1.md}"

fail() {
  echo "benchmark step summary smoke failed: $*" >&2
  exit 1
}

require_literal() {
  local file="$1"
  local literal="$2"
  local description="$3"

  if ! grep -Fq -- "$literal" "$file"; then
    fail "$file is missing $description"
  fi
}

main() {
  local summary_file
  local summary_json

  summary_file="$(mktemp)"
  summary_json="$(mktemp)"
  trap "rm -f '$summary_file' '$summary_json'" EXIT

  cat >"$summary_json" <<'JSON'
{
  "report": "/tmp/codeinsight-benchmark-subset.md",
  "profile": "smoke",
  "repository_subset": "p-limit",
  "repositories": 1,
  "routing": {
    "context_pack_first": 1,
    "total": 1
  },
  "context": {
    "total_repo_lines": 1200,
    "selected_lines": 12,
    "line_reduction": "99.0%",
    "estimated_tokens_total": 250,
    "estimated_tokens_average": 250,
    "truncated_packs": 0
  },
  "indexing": {
    "total_ms": 42,
    "average_ms": 42
  },
  "failures": {
    "total": 0
  },
  "next_steps": {
    "open_report": "/tmp/codeinsight-benchmark-subset.md",
    "inspect": "Key Results, Summary, and each Context reading plan table",
    "continue_with": "file_outline for first files, dependency_graph for imports, impact_analysis before edits"
  }
}
JSON

  GITHUB_STEP_SUMMARY="$summary_file" \
    "$ROOT_DIR/scripts/benchmark-step-summary.sh" \
    "$REPORT_FILE" \
    codeinsight-benchmark-subset \
    "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/1/artifacts/2" \
    "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/1" \
    "$summary_json" >/dev/null

  require_literal "$summary_file" "## CodeInsight v0.1" "benchmark title"
  require_literal "$summary_file" "Workflow run: [open run](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/1)" "run link"
  require_literal "$summary_file" 'Workflow artifact: [`codeinsight-benchmark-subset`](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/1/artifacts/2)' "artifact link"
  require_literal "$summary_file" "## CodeInsight Benchmark Summary" "machine summary heading"
  require_literal "$summary_file" 'Routing: `context_pack` first for 1/1 repositories' "machine summary routing"
  require_literal "$summary_file" "Download the full Markdown report from the workflow artifact" "download guidance"
  require_literal "$summary_file" "### Key Results" "key results heading"
  require_literal "$summary_file" "Agent routing: \`context_pack\` was the first recommended tool" "routing key result"
  require_literal "$summary_file" "### Summary" "summary heading"
  require_literal "$summary_file" "| Repository | Focus | Commit | Files | Lines | Symbols |" "summary table"

  echo "benchmark step summary smoke passed"
}

main "$@"
