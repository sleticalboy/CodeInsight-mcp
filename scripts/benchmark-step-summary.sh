#!/usr/bin/env bash
set -euo pipefail

REPORT_FILE="${1:-}"
ARTIFACT_NAME="${2:-codeinsight-benchmark-subset}"
SUMMARY_FILE="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

usage() {
  cat >&2 <<'EOF'
usage: scripts/benchmark-step-summary.sh REPORT_FILE [ARTIFACT_NAME]

Appends the benchmark report's Summary and Key Results sections to the GitHub
Actions step summary. When GITHUB_STEP_SUMMARY is not set, writes to stdout.
EOF
}

fail() {
  echo "benchmark step summary failed: $*" >&2
  exit 1
}

extract_section() {
  local heading="$1"

  awk -v heading="$heading" '
    $0 == heading { in_section = 1; next }
    in_section && /^## / { exit }
    in_section { print }
  ' "$REPORT_FILE"
}

require_section() {
  local heading="$1"
  local description="$2"

  if ! grep -Fxq -- "$heading" "$REPORT_FILE"; then
    fail "$REPORT_FILE is missing $description"
  fi
}

main() {
  local title

  if [ -z "$REPORT_FILE" ] || [ "$REPORT_FILE" = "-h" ] || [ "$REPORT_FILE" = "--help" ]; then
    usage
    exit 2
  fi
  if [ ! -s "$REPORT_FILE" ]; then
    fail "$REPORT_FILE does not exist or is empty"
  fi

  require_section "## Summary" "summary section"
  require_section "## Key Results" "key results section"

  title="$(sed -n '1s/^# //p' "$REPORT_FILE")"
  if [ -z "$title" ]; then
    fail "$REPORT_FILE is missing a title"
  fi

  {
    printf "## %s\n\n" "$title"
    printf 'Full report: `%s`\n\n' "$REPORT_FILE"
    printf 'Workflow artifact: `%s`\n\n' "$ARTIFACT_NAME"
    printf "Download the full Markdown report from the workflow artifact when you need detail rows, guardrail tables, or context-pack file lists.\n\n"
    printf "### Key Results\n\n"
    extract_section "## Key Results"
    printf "\n### Summary\n\n"
    extract_section "## Summary"
  } >>"$SUMMARY_FILE"

  echo "benchmark step summary written to $SUMMARY_FILE"
}

main "$@"
