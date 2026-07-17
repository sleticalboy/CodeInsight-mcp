#!/usr/bin/env bash
set -euo pipefail

SUMMARY_JSON="${1:-}"

usage() {
  cat >&2 <<'EOF'
usage: scripts/benchmark-summary-text.sh SUMMARY_JSON

Prints a compact Markdown benchmark summary from a JSON file produced by
CODEINSIGHT_BENCH_SUMMARY_JSON=... scripts/benchmark-smoke.sh.
EOF
}

fail() {
  echo "benchmark summary text failed: $*" >&2
  exit 1
}

main() {
  if [ -z "$SUMMARY_JSON" ] || [ "$SUMMARY_JSON" = "-h" ] || [ "$SUMMARY_JSON" = "--help" ]; then
    usage
    exit 2
  fi
  if [ ! -s "$SUMMARY_JSON" ]; then
    fail "$SUMMARY_JSON does not exist or is empty"
  fi

  jq -r '
    "## CodeInsight Benchmark Summary",
    "",
    "- Report: `" + (.report // "-") + "`",
    "- Profile: `" + (.profile // "-") + "` (`" + (.repository_subset // "-") + "` subset)",
    "- Repositories: " + ((.repositories // 0) | tostring),
    "- Routing: `context_pack` first for " + ((.routing.context_pack_first // 0) | tostring) + "/" + ((.routing.total // 0) | tostring) + " repositories",
    "- Context compression: " + ((.context.selected_lines // 0) | tostring) + " of " + ((.context.total_repo_lines // 0) | tostring) + " lines selected (" + (.context.line_reduction // "n/a") + " reduction)",
    "- Token budget: " + ((.context.estimated_tokens_total // 0) | tostring) + " estimated tokens total, " + ((.context.estimated_tokens_average // 0) | tostring) + " average",
    "- Indexing: " + ((.indexing.total_ms // 0) | tostring) + " ms total, " + ((.indexing.average_ms // 0) | tostring) + " ms average",
    "- Guardrail failures: " + ((.failures.total // 0) | tostring),
    "- Truncated context packs: " + ((.context.truncated_packs // 0) | tostring),
    "",
    "Next steps:",
    "- Open report: `" + (.next_steps.open_report // (.report // "-")) + "`",
    "- Inspect: " + (.next_steps.inspect // "Key Results, Summary, and Context reading plan"),
    "- Continue with: " + (.next_steps.continue_with // "file_outline, dependency_graph, impact_analysis")
  ' "$SUMMARY_JSON"
}

main "$@"
