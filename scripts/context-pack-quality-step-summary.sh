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
      and all(.scenarios[]; .status == "pass" and (.name | type == "string") and (.metrics | type == "object"))
      and ((.question_checks_passed // 0) | type == "number")
      and ((.question_checks // []) | type == "array")
      and ((.question_checks // []) | length) == (.question_checks_passed // 0)
      and all((.question_checks // [])[]; .status == "pass" and (.name | type == "string") and (.focus | type == "string") and (.question | type == "string"))' \
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

question_check_rows() {
  jq -r '
    def cell:
      tostring
      | gsub("\\|"; "\\\\|")
      | gsub("\n"; " ");
    (.question_checks // [])[]
    | "| `\(.name)` | `\(.next_action)` | `\(.file)` | \(.focus | cell) | \(.question | cell) | `\(.suggested_tool)` |"
  ' "$SUMMARY_JSON"
}

main() {
  local question_checks_passed
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
  question_checks_passed="$(jq -r '.question_checks_passed // 0' "$SUMMARY_JSON")"

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
    if [ "$question_checks_passed" -gt 0 ]; then
      printf 'Question checks passed: `%s`\n\n' "$question_checks_passed"
      printf "| Check | Next Action | File | Focus | Question | Suggested Tool |\n"
      printf "| --- | --- | --- | --- | --- | --- |\n"
      question_check_rows
      printf "\n"
    fi
  } >>"$SUMMARY_FILE"

  echo "context-pack quality step summary written to $SUMMARY_FILE"
}

main "$@"
