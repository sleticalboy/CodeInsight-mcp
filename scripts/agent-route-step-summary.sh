#!/usr/bin/env bash
set -euo pipefail

SUMMARY_JSON="${1:-}"
ARTIFACT_NAME="${2:-codeinsight-agent-route-smoke}"
ARTIFACT_URL="${3:-}"
RUN_URL="${4:-}"
SUMMARY_FILE="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

usage() {
  cat >&2 <<'EOF'
usage: scripts/agent-route-step-summary.sh SUMMARY_JSON [ARTIFACT_NAME] [ARTIFACT_URL] [RUN_URL]

Appends the agent-route smoke evidence summary JSON to the GitHub Actions step
summary. When GITHUB_STEP_SUMMARY is not set, writes to stdout.
RUN_URL defaults to the current GitHub Actions run when standard GITHUB_*
environment variables are available.
EOF
}

fail() {
  echo "agent-route step summary failed: $*" >&2
  exit 1
}

require_summary_contract() {
  if ! jq -e \
    '.status == "pass"
      and .route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and (.metrics | type == "object")
      and (.metrics.indexed_files | type == "number")
      and (.metrics.symbols | type == "number")
      and (.metrics.index_errors | type == "number")
      and (.metrics.entrypoints | type == "number")
      and (.metrics.selected_files | type == "number")
      and (.metrics.selected_ranges | type == "number")
      and (.metrics.reading_plan_steps | type == "number")
      and (.metrics.requested_token_budget | type == "number")
      and (.metrics.applied_token_budget | type == "number")
      and (.metrics.first_context_file | type == "string")
      and (.metrics.first_next_action | type == "string")
      and (.metrics.context_route_reason | type == "string")
      and (.metrics.impact_route_reason | type == "string")
      and (.metrics.impact_status | type == "string")
      and (.metrics.impacted_files | type == "number")
      and (.metrics.suggested_checks | type == "number")' \
    "$SUMMARY_JSON" >/dev/null; then
    fail "$SUMMARY_JSON does not match the agent-route summary contract"
  fi
}

metric() {
  local query="$1"
  jq -r "$query" "$SUMMARY_JSON"
}

main() {
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

  {
    printf "## Agent Route Smoke\n\n"
    printf 'Status: `%s`\n\n' "$(metric '.status')"
    printf 'Task: `%s`\n\n' "$(metric '.task')"
    printf 'Route: `%s`\n\n' "$(metric '.route_tools | join(" -> ")')"
    printf 'Full JSON summary: `%s`\n\n' "$SUMMARY_JSON"
    if [ -n "$RUN_URL" ]; then
      printf 'Workflow run: [open run](%s)\n\n' "$RUN_URL"
    fi
    if [ -n "$ARTIFACT_URL" ]; then
      printf 'Workflow artifact: [`%s`](%s)\n\n' "$ARTIFACT_NAME" "$ARTIFACT_URL"
    else
      printf 'Workflow artifact: `%s`\n\n' "$ARTIFACT_NAME"
    fi
    printf "| Metric | Value |\n"
    printf "| --- | --- |\n"
    printf '| Indexed files | `%s` |\n' "$(metric '.metrics.indexed_files')"
    printf '| Symbols | `%s` |\n' "$(metric '.metrics.symbols')"
    printf '| Index errors | `%s` |\n' "$(metric '.metrics.index_errors')"
    printf '| Entrypoints | `%s` |\n' "$(metric '.metrics.entrypoints')"
    printf '| Selected files | `%s` |\n' "$(metric '.metrics.selected_files')"
    printf '| Selected ranges | `%s` |\n' "$(metric '.metrics.selected_ranges')"
    printf '| Reading-plan steps | `%s` |\n' "$(metric '.metrics.reading_plan_steps')"
    printf '| Token budget | `%s/%s` |\n' "$(metric '.metrics.applied_token_budget')" "$(metric '.metrics.requested_token_budget')"
    printf '| First context file | `%s` |\n' "$(metric '.metrics.first_context_file')"
    printf '| First next action | `%s` |\n' "$(metric '.metrics.first_next_action')"
    printf '| Context route reason | %s |\n' "$(metric '.metrics.context_route_reason')"
    printf '| Impact route reason | %s |\n' "$(metric '.metrics.impact_route_reason')"
    printf '| Impact status | `%s` |\n' "$(metric '.metrics.impact_status')"
    printf '| Impacted files | `%s` |\n' "$(metric '.metrics.impacted_files')"
    printf '| Suggested checks | `%s` |\n' "$(metric '.metrics.suggested_checks')"
    printf "\n"
  } >>"$SUMMARY_FILE"

  echo "agent-route step summary written to $SUMMARY_FILE"
}

main "$@"
