#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPORT_FILE="${1:-}"
ARTIFACT_NAME="${2:-codeinsight-benchmark-subset}"
ARTIFACT_URL="${3:-}"
RUN_URL="${4:-}"
SUMMARY_JSON="${5:-}"
SUMMARY_FILE="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

usage() {
  cat >&2 <<'EOF'
usage: scripts/benchmark-step-summary.sh REPORT_FILE [ARTIFACT_NAME] [ARTIFACT_URL] [RUN_URL] [SUMMARY_JSON]

Appends the benchmark report's Summary and Key Results sections to the GitHub
Actions step summary. When GITHUB_STEP_SUMMARY is not set, writes to stdout.
RUN_URL defaults to the current GitHub Actions run when standard GITHUB_*
environment variables are available.
When SUMMARY_JSON is provided, also appends the compact machine-readable
benchmark summary as Markdown.
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
  if [ -z "$RUN_URL" ] &&
    [ -n "${GITHUB_SERVER_URL:-}" ] &&
    [ -n "${GITHUB_REPOSITORY:-}" ] &&
    [ -n "${GITHUB_RUN_ID:-}" ]; then
    RUN_URL="${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"
  fi

  {
    printf "## %s\n\n" "$title"
    printf 'Full report: `%s`\n\n' "$REPORT_FILE"
    if [ -n "$RUN_URL" ]; then
      printf 'Workflow run: [open run](%s)\n\n' "$RUN_URL"
    fi
    if [ -n "$ARTIFACT_URL" ]; then
      printf 'Workflow artifact: [`%s`](%s)\n\n' "$ARTIFACT_NAME" "$ARTIFACT_URL"
    else
      printf 'Workflow artifact: `%s`\n\n' "$ARTIFACT_NAME"
    fi
    if [ -n "$SUMMARY_JSON" ]; then
      "$ROOT_DIR/scripts/benchmark-summary-text.sh" "$SUMMARY_JSON"
      printf "\n"
    fi
    printf "Download the full Markdown report from the workflow artifact when you need detail rows, guardrail tables, or context-pack file lists.\n\n"
    printf "### Key Results\n\n"
    extract_section "## Key Results"
    printf "\n### Summary\n\n"
    extract_section "## Summary"
  } >>"$SUMMARY_FILE"

  echo "benchmark step summary written to $SUMMARY_FILE"
}

main "$@"
