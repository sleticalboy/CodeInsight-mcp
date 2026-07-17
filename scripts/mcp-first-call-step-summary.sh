#!/usr/bin/env bash
set -euo pipefail

SUMMARY_JSON="${1:-}"
ARTIFACT_NAME="${2:-codeinsight-mcp-first-call}"
ARTIFACT_URL="${3:-}"
RUN_URL="${4:-}"
SUMMARY_FILE="${GITHUB_STEP_SUMMARY:-/dev/stdout}"

usage() {
  cat >&2 <<'EOF'
usage: scripts/mcp-first-call-step-summary.sh SUMMARY_JSON [ARTIFACT_NAME] [ARTIFACT_URL] [RUN_URL]

Appends the MCP first-call smoke summary JSON to the GitHub Actions step
summary. When GITHUB_STEP_SUMMARY is not set, writes to stdout.
RUN_URL defaults to the current GitHub Actions run when standard GITHUB_*
environment variables are available.
EOF
}

fail() {
  echo "mcp first-call step summary failed: $*" >&2
  exit 1
}

require_summary_contract() {
  if ! jq -e \
    '.status == "pass"
      and .server == "codeinsight"
      and .route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .execution_plan_actions == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]
      and (.selected_files | type == "array")
      and (.selected_files | length) >= 1
      and (.suggested_tool.tool | type == "string")
      and .suggested_tool_executed == true
      and .impact_status == "complete"
      and (.impact_counts.impacted_files | type == "number")' \
    "$SUMMARY_JSON" >/dev/null; then
    fail "$SUMMARY_JSON does not match the MCP first-call summary contract"
  fi
}

value() {
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
    printf "## MCP First-Call Smoke\n\n"
    printf 'Status: `%s`\n\n' "$(value '.status')"
    printf 'Task: `%s`\n\n' "$(value '.task')"
    printf 'Route: `%s`\n\n' "$(value '.route_tools | join(" -> ")')"
    printf 'Execution plan: `%s`\n\n' "$(value '.execution_plan_actions | join(" -> ")')"
    printf 'Selected files: `%s`\n\n' "$(value '.selected_files | join("`, `")')"
    printf 'Suggested tool: `%s`\n\n' "$(value '.suggested_tool.tool')"
    printf 'Suggested tool executed: `%s`\n\n' "$(value '.suggested_tool_executed')"
    printf 'Impact status: `%s`\n\n' "$(value '.impact_status')"
    printf 'Impacted files: `%s`\n\n' "$(value '.impact_counts.impacted_files')"
    printf 'Full JSON summary: `%s`\n\n' "$SUMMARY_JSON"
    if [ -n "$RUN_URL" ]; then
      printf 'Workflow run: [open run](%s)\n\n' "$RUN_URL"
    fi
    if [ -n "$ARTIFACT_URL" ]; then
      printf 'Workflow artifact: [`%s`](%s)\n\n' "$ARTIFACT_NAME" "$ARTIFACT_URL"
    else
      printf 'Workflow artifact: `%s`\n\n' "$ARTIFACT_NAME"
    fi
  } >>"$SUMMARY_FILE"

  echo "MCP first-call step summary written to $SUMMARY_FILE"
}

main "$@"
