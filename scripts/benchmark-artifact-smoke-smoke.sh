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
  echo "benchmark artifact smoke smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/bin" "$TEMP_DIR/artifact"

  cat >"$TEMP_DIR/artifact/codeinsight-benchmark-subset.md" <<'EOF'
# CodeInsight v0.1 Smoke Benchmark

This is a benchmark fixture report, not a controlled performance benchmark.

- Profile: `smoke`
- Repository subset: `p-limit`
- Context pack mode: default 6000 token budget

## Summary

| Repository | Focus | Commit | Files | Lines | Symbols | Index ms | Entrypoints | First entrypoint | Context files | First recommended tool | Reading plan steps | Selected files | Selected ranges | Line reduction | Estimated tokens | Token budget | Guardrail failures | Continuation | Truncated |
|---|---|---:|---:|---:|---:|---:|---:|---|---:|---|---:|---:|---:|---:|---:|---:|---:|---|---|
| p-limit | JS | `1234567` | 1 | 100 | 5 | 10 | 1 | `index.js` | 1 | `context_pack` | 1 | 1 | 1 | 99.0% | 100 | 6000 | 0 | complete | false |

## Key Results

- Repositories benchmarked: 1 (`p-limit` subset).
- Agent routing: `context_pack` was the first recommended tool for 1/1 repositories.
- Context compression: selected 1 of 100 source lines (99.0% reduction) across 1 files and 1 ranges.
- Token budget: 100 estimated tokens total, 100 average tokens per repository, with a 6000 token budget per context pack.
- Guardrails: 0 context, 0 symbol, 0 call target, and 0 call edge failures.

## Details

## p-limit

- Context continuation next action: read_selected_context
- First omitted candidate: none

Recommended next tools:

| Tool | Score | Reason |
|---|---:|---|
| `context_pack` | 1.0 | first read |

Context pack files:

| File | Lines | Reason |
|---|---:|---|
| `index.js` | 1 | selected |

Context reading plan:

| File | Rank | Question | Next action | Suggested tool | Reason | Selection reason |
|---|---:|---|---|---|---|---|
| `index.js` | 1 | What entrypoints, exported symbols, or setup code define the main flow here? | inspect_seed_file | file_outline | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? | seed file |

Context pack guardrails:

| Check | Expected | Actual | Status |
|---|---|---|---|
| `first_recommended_tool` | context_pack | context_pack | pass |
| `selected_files` | >= 1 | 1 | pass |
| `reading_plan_steps` | >= 1 | 1 | pass |
| `first_reading_question` | present | What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_reading_reason` | present | Read this step to answer: What entrypoints, exported symbols, or setup code define the main flow here? | pass |
| `first_selection_rank` | >= 1 | 1 | pass |
| `first_selection_reason` | present | seed file | pass |
| `line_reduction` | >= 50% | 99.0% | pass |
EOF

  cat >"$TEMP_DIR/artifact/codeinsight-benchmark-subset.json" <<'JSON'
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
    "total_repo_lines": 100,
    "selected_lines": 1,
    "line_reduction": "99.0%",
    "estimated_tokens_total": 100,
    "estimated_tokens_average": 100,
    "truncated_packs": 0
  },
  "indexing": {
    "total_ms": 10,
    "average_ms": 10
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

  cat >"$TEMP_DIR/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [ "$1" = "run" ] && [ "$2" = "download" ]; then
  shift 2
  output_dir=""
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --dir)
        shift
        output_dir="$1"
        ;;
    esac
    shift
  done
  if [ -z "$output_dir" ]; then
    exit 12
  fi
  cp "${CODEINSIGHT_BENCHMARK_ARTIFACT_FIXTURE:?}"/* "$output_dir"/
  exit 0
fi

exit 13
EOF
  chmod +x "$TEMP_DIR/bin/gh"

  CODEINSIGHT_BENCHMARK_ARTIFACT_FIXTURE="$TEMP_DIR/artifact" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/benchmark-artifact-smoke.sh" \
      --dir "$TEMP_DIR/download" \
      123456 >"$TEMP_DIR/output.log"

  grep -Fq "benchmark artifact smoke passed" "$TEMP_DIR/output.log" ||
    fail "missing pass output"
  grep -Fq "report: $TEMP_DIR/download/codeinsight-benchmark-subset.md" "$TEMP_DIR/output.log" ||
    fail "missing report path"
  grep -Fq "summary: $TEMP_DIR/download/codeinsight-benchmark-subset.json" "$TEMP_DIR/output.log" ||
    fail "missing summary path"

  echo "benchmark artifact smoke smoke passed"
}

main "$@"
