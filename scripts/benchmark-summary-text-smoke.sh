#!/usr/bin/env bash
set -euo pipefail

TEMP_DIR="$(mktemp -d "${TMPDIR:-/tmp}/codeinsight-benchmark-summary-text.XXXXXX")"

cleanup() {
  rm -rf "$TEMP_DIR"
}

trap cleanup EXIT INT TERM

summary_json="$TEMP_DIR/summary.json"
summary_md="$TEMP_DIR/summary.md"

cat >"$summary_json" <<'JSON'
{
  "report": "/tmp/codeinsight-local-benchmark.md",
  "profile": "local",
  "repository_subset": "all",
  "repositories": 1,
  "routing": {
    "context_pack_first": 1,
    "total": 1
  },
  "context": {
    "total_repo_lines": 1200,
    "selected_lines": 120,
    "line_reduction": "90.0%",
    "estimated_tokens_total": 900,
    "estimated_tokens_average": 900,
    "selected_files": 3,
    "selected_ranges": 5,
    "truncated_packs": 0
  },
  "indexing": {
    "total_ms": 42,
    "average_ms": 42
  },
  "failures": {
    "total": 0,
    "budget": 0,
    "context_guardrail": 0,
    "symbol_target": 0,
    "call_target": 0,
    "call_edge": 0
  },
  "next_steps": {
    "open_report": "/tmp/codeinsight-local-benchmark.md",
    "inspect": "Key Results, Summary, and each Context reading plan table",
    "continue_with": "file_outline for first files, dependency_graph for imports, impact_analysis before edits"
  }
}
JSON

"$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/scripts/benchmark-summary-text.sh" "$summary_json" >"$summary_md"

grep -Fq '## CodeInsight Benchmark Summary' "$summary_md"
grep -Fq 'Report: `/tmp/codeinsight-local-benchmark.md`' "$summary_md"
grep -Fq 'Profile: `local` (`all` subset)' "$summary_md"
grep -Fq 'Routing: `context_pack` first for 1/1 repositories' "$summary_md"
grep -Fq 'Context compression: 120 of 1200 lines selected (90.0% reduction)' "$summary_md"
grep -Fq 'Guardrail failures: 0' "$summary_md"
grep -Fq 'Continue with: file_outline for first files, dependency_graph for imports, impact_analysis before edits' "$summary_md"

echo "benchmark summary text smoke passed"
