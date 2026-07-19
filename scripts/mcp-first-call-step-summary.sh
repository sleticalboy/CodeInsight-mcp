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
      and (.first_context_file | type == "string" and length > 0)
      and .first_reading_file == .first_context_file
      and (.first_reading_selection_rank | type == "number")
      and .execution_plan_reads_in_reading_plan_order == true
      and .first_execution_instruction_has_focus == true
      and .first_execution_instruction_has_question == true
      and .current_step_suggested_tool_matches_reading_plan == true
      and .current_step_instruction_has_focus == true
      and .current_step_instruction_has_question == true
      and .current_step_instruction_has_action == true
      and .continuation_after_selected_context == true
      and (.continuation_status | type == "string")
      and (.continuation_next_action | type == "string" and length > 0)
      and (.first_omitted_file | type == "string")
      and ((.first_omitted_selection_rank | type == "number") or (.first_omitted_selection_rank == null))
      and (.first_omitted_omission_reason | type == "string")
      and (.first_omitted_next_action | type == "string")
      and (.reading_plan[0].file == .first_reading_file)
      and (.reading_plan[0].selection_rank == .first_reading_selection_rank)
      and (.reading_plan[0].next_action | type == "string" and length > 0)
      and (.reading_plan[0].focus | type == "string" and length > 0)
      and (.reading_plan[0].question | type == "string" and length > 0)
      and (.reading_plan[0].selection_reason | type == "string" and length > 0)
      and (.reading_plan[0] as $step
        | ($step.reason | type == "string")
        and ($step.reason | contains($step.question))
        and ($step.reason | contains("If deeper evidence is needed, call "))
        and ($step.reason | contains($step.suggested_tool))
        and ($step.reason | contains("Selection reason:")))
      and (.suggested_tool.tool | type == "string")
      and .suggested_tool.tool == .reading_plan[0].suggested_tool
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
    printf 'First context file: `%s`\n\n' "$(value '.first_context_file')"
    printf 'First reading file: `%s`\n\n' "$(value '.first_reading_file')"
    printf 'First reading selection rank: `%s`\n\n' "$(value '.first_reading_selection_rank')"
    printf 'First next action: `%s`\n\n' "$(value '.reading_plan[0].next_action')"
    printf 'First reading focus: `%s`\n\n' "$(value '.reading_plan[0].focus')"
    printf 'First reading question: `%s`\n\n' "$(value '.reading_plan[0].question')"
    printf 'Reading order contract: `%s`\n\n' "$(value '.execution_plan_reads_in_reading_plan_order')"
    printf 'First execution instruction focus contract: `%s`\n\n' "$(value '.first_execution_instruction_has_focus')"
    printf 'First execution instruction question contract: `%s`\n\n' "$(value '.first_execution_instruction_has_question')"
    printf 'Suggested tool handoff contract: `%s`\n\n' "$(value '.current_step_suggested_tool_matches_reading_plan')"
    printf 'Current-step instruction focus contract: `%s`\n\n' "$(value '.current_step_instruction_has_focus')"
    printf 'Current-step instruction question contract: `%s`\n\n' "$(value '.current_step_instruction_has_question')"
    printf 'Current-step instruction action contract: `%s`\n\n' "$(value '.current_step_instruction_has_action')"
    printf 'Continuation timing contract: `%s`\n\n' "$(value '.continuation_after_selected_context')"
    printf 'Continuation status: `%s`\n\n' "$(value '.continuation_status')"
    printf 'Continuation next action: `%s`\n\n' "$(value '.continuation_next_action')"
    printf 'First omitted file: `%s`\n\n' "$(value 'if .first_omitted_file == "" then "-" else .first_omitted_file end')"
    printf 'First omitted selection rank: `%s`\n\n' "$(value 'if .first_omitted_selection_rank == null then "-" else .first_omitted_selection_rank end')"
    printf 'First omitted omission reason: `%s`\n\n' "$(value 'if .first_omitted_omission_reason == "" then "-" else .first_omitted_omission_reason end')"
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
