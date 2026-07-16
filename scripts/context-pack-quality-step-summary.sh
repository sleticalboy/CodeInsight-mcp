#!/usr/bin/env bash
set -euo pipefail

SUMMARY_JSON="${1:-}"
ARTIFACT_NAME="${2:-codeinsight-context-pack-quality}"
ARTIFACT_URL="${3:-}"
RUN_URL="${4:-}"
SUMMARY_FILE="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

usage() {
  cat >&2 <<'EOF'
usage: scripts/context-pack-quality-step-summary.sh SUMMARY_JSON [ARTIFACT_NAME] [ARTIFACT_URL] [RUN_URL]

Appends the context-pack quality smoke summary JSON to the GitHub Actions step
summary. When GITHUB_STEP_SUMMARY is not set, writes to stdout.
RUN_URL defaults to the current GitHub Actions run when standard GITHUB_*
environment variables are available.
EOF
}

fail() {
  echo "context-pack quality step summary failed: $*" >&2
  exit 1
}

require_summary_contract() {
  if ! jq -e \
    '.status == "pass"
      and (.scenarios_passed | type == "number")
      and (.scenarios | type == "array")
      and (.scenarios | length) == .scenarios_passed
      and all(.scenarios[]; .status == "pass" and (.name | type == "string") and (.metrics | type == "object"))' \
    "$SUMMARY_JSON" >/dev/null; then
    fail "$SUMMARY_JSON does not match the context-pack quality summary contract"
  fi
}

scenario_rows() {
  jq -r '
    def metric_value:
      tostring
      | gsub("\\|"; "\\\\|")
      | gsub("\n"; " ");
    .scenarios[]
    | .metrics as $metrics
    | "| `\(.name)` | `\(.status)` | \($metrics | to_entries | map("`\(.key)=\(.value | metric_value)`") | join("<br>")) |"
  ' "$SUMMARY_JSON"
}

main() {
  local scenarios_passed

  if [ -z "$SUMMARY_JSON" ] || [ "$SUMMARY_JSON" = "-h" ] || [ "$SUMMARY_JSON" = "--help" ]; then
    usage
    exit 2
  fi
  if [ ! -s "$SUMMARY_JSON" ]; then
    fail "$SUMMARY_JSON does not exist or is empty"
  fi
  if ! command -v jq >/dev/null 2>&1; then
    fail "missing required command: jq"
  fi

  require_summary_contract

  if [ -z "$RUN_URL" ] &&
    [ -n "${GITHUB_SERVER_URL:-}" ] &&
    [ -n "${GITHUB_REPOSITORY:-}" ] &&
    [ -n "${GITHUB_RUN_ID:-}" ]; then
    RUN_URL="${GITHUB_SERVER_URL}/${GITHUB_REPOSITORY}/actions/runs/${GITHUB_RUN_ID}"
  fi

  scenarios_passed="$(jq -r '.scenarios_passed' "$SUMMARY_JSON")"

  {
    printf "## Context Pack Quality Smoke\n\n"
    printf 'Scenarios passed: `%s`\n\n' "$scenarios_passed"
    printf 'Full JSON summary: `%s`\n\n' "$SUMMARY_JSON"
    if [ -n "$RUN_URL" ]; then
      printf 'Workflow run: [open run](%s)\n\n' "$RUN_URL"
    fi
    if [ -n "$ARTIFACT_URL" ]; then
      printf 'Workflow artifact: [`%s`](%s)\n\n' "$ARTIFACT_NAME" "$ARTIFACT_URL"
    else
      printf 'Workflow artifact: `%s`\n\n' "$ARTIFACT_NAME"
    fi
    printf "| Scenario | Status | Key Metrics |\n"
    printf "| --- | --- | --- |\n"
    scenario_rows
    printf "\n"
  } >>"$SUMMARY_FILE"

  echo "context-pack quality step summary written to $SUMMARY_FILE"
}

main "$@"
