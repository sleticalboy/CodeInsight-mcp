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

  summary_file="$(mktemp)"
  trap "rm -f '$summary_file'" EXIT

  GITHUB_STEP_SUMMARY="$summary_file" \
    "$ROOT_DIR/scripts/benchmark-step-summary.sh" "$REPORT_FILE" codeinsight-benchmark-subset >/dev/null

  require_literal "$summary_file" "## CodeInsight v0.1" "benchmark title"
  require_literal "$summary_file" 'Workflow artifact: `codeinsight-benchmark-subset`' "artifact pointer"
  require_literal "$summary_file" "Download the full Markdown report from the workflow artifact" "download guidance"
  require_literal "$summary_file" "### Key Results" "key results heading"
  require_literal "$summary_file" "Agent routing: \`context_pack\` was the first recommended tool" "routing key result"
  require_literal "$summary_file" "### Summary" "summary heading"
  require_literal "$summary_file" "| Repository | Focus | Commit | Files | Lines | Symbols |" "summary table"

  echo "benchmark step summary smoke passed"
}

main "$@"
