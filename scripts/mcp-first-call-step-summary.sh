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
      and (.seed_strategy | type == "string" and length > 0)
      and (.first_seed_source | type == "string")
      and (.first_seed_value | type == "string")
      and (.selected_seeds | type == "array")
      and (.first_context_file | type == "string" and length > 0)
      and .first_reading_file == .first_context_file
      and (.first_reading_selection_rank | type == "number")
      and .current_reading_step_matches_reading_plan == true
      and (.context_pack_read_less | type == "object")
      and (.context_pack_read_less.baseline_source_lines | type == "number")
      and (.context_pack_read_less.selected_source_lines | type == "number")
      and (.context_pack_read_less.source_lines_avoided | type == "number")
      and (.context_pack_read_less.line_reduction | type == "string" and length > 0)
      and (.context_pack_read_less.read_less_ratio | type == "string" and length > 0)
      and .baseline_source_lines == .context_pack_read_less.baseline_source_lines
      and .selected_source_lines == .context_pack_read_less.selected_source_lines
      and .source_lines_avoided == .context_pack_read_less.source_lines_avoided
      and .line_reduction == .context_pack_read_less.line_reduction
      and .read_less_ratio == .context_pack_read_less.read_less_ratio
      and .execution_plan_reads_in_reading_plan_order == true
      and .first_execution_instruction_has_focus == true
      and .first_execution_instruction_has_question == true
      and .first_execution_instruction_has_read_less == true
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
      and (.impact_counts.impacted_files | type == "number")
      and .impact_execution_suggested_tool == "impact_analysis"
      and (.impact_execution_suggested_checks | type == "number" and . >= 1)
      and .impact_execution_suggested_checks == .impact_suggested_checks
      and (.impact_first_suggested_check.kind | type == "string" and length > 0)
      and .impact_execution_instruction_has_first_check == true
      and .blocked_no_seed.route_step_status == "blocked_no_seed"
      and .blocked_no_seed.seed_strategy == "auto_no_seed"
      and .blocked_no_seed.continuation_status == "blocked_no_seed"
      and .blocked_no_seed.continuation_next_action == "provide_seed_file_or_symbol"
      and .blocked_no_seed.context_files == 0
      and .blocked_no_seed.reading_plan_steps == 0
      and .blocked_no_seed.has_current_reading_step == false
      and .blocked_no_seed.impact_status == "skipped_no_seed"
      and .blocked_no_seed.execution_plan_actions == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]
      and .blocked_no_seed.execution_plan_statuses == ["blocked_no_reading_plan", "blocked_no_current_reading_step", "manual_after_selected_context", "skipped_no_seed"]
      and .blocked_no_context.route_step_status == "blocked_no_context"
      and .blocked_no_context.continuation_status == "blocked_no_context"
      and .blocked_no_context.continuation_next_action == "provide_matching_seed_file_or_symbol"
      and .blocked_no_context.truncation_reason == "no_context_for_explicit_seed"
      and .blocked_no_context.context_files == 0
      and .blocked_no_context.reading_plan_steps == 0
      and .blocked_no_context.has_current_reading_step == false
      and .blocked_no_context.impact_status == "skipped_no_context"
      and .blocked_no_context.execution_plan_actions == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]
      and .blocked_no_context.execution_plan_statuses == ["blocked_no_reading_plan", "blocked_no_current_reading_step", "manual_after_selected_context", "skipped_no_context"]' \
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
    printf 'Seed strategy: `%s`\n\n' "$(value '.seed_strategy')"
    printf 'First seed source: `%s`\n\n' "$(value '.first_seed_source')"
    printf 'First seed value: `%s`\n\n' "$(value '.first_seed_value')"
    printf 'First context file: `%s`\n\n' "$(value '.first_context_file')"
    printf 'First reading file: `%s`\n\n' "$(value '.first_reading_file')"
    printf 'First reading selection rank: `%s`\n\n' "$(value '.first_reading_selection_rank')"
    printf 'Current reading step mirror contract: `%s`\n\n' "$(value '.current_reading_step_matches_reading_plan')"
    printf 'Blind first-read baseline: `%s` source lines\n\n' "$(value '.baseline_source_lines')"
    printf 'Routed first-read: `%s` source lines\n\n' "$(value '.selected_source_lines')"
    printf 'Source lines avoided: `%s`\n\n' "$(value '.source_lines_avoided')"
    printf 'First-read line reduction: `%s`\n\n' "$(value '.line_reduction')"
    printf 'Read less: `%s`\n\n' "$(value '.read_less_ratio')"
    printf 'First next action: `%s`\n\n' "$(value '.reading_plan[0].next_action')"
    printf 'First reading focus: `%s`\n\n' "$(value '.reading_plan[0].focus')"
    printf 'First reading question: `%s`\n\n' "$(value '.reading_plan[0].question')"
    printf 'Reading order contract: `%s`\n\n' "$(value '.execution_plan_reads_in_reading_plan_order')"
    printf 'First execution instruction focus contract: `%s`\n\n' "$(value '.first_execution_instruction_has_focus')"
    printf 'First execution instruction question contract: `%s`\n\n' "$(value '.first_execution_instruction_has_question')"
    printf 'First execution instruction read-less contract: `%s`\n\n' "$(value '.first_execution_instruction_has_read_less')"
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
    printf 'Impact execution suggested tool: `%s`\n\n' "$(value '.impact_execution_suggested_tool')"
    printf 'Impact execution suggested checks: `%s`\n\n' "$(value '.impact_execution_suggested_checks')"
    printf 'Impact first suggested check: `%s`\n\n' "$(value '.impact_first_suggested_check.command // .impact_first_suggested_check.file // .impact_first_suggested_check.kind')"
    printf 'Blocked no-seed status: `%s`\n\n' "$(value '.blocked_no_seed.continuation_status')"
    printf 'Blocked no-seed next action: `%s`\n\n' "$(value '.blocked_no_seed.continuation_next_action')"
    printf 'Blocked no-seed execution statuses: `%s`\n\n' "$(value '.blocked_no_seed.execution_plan_statuses | join(" -> ")')"
    printf 'Blocked no-context status: `%s`\n\n' "$(value '.blocked_no_context.continuation_status')"
    printf 'Blocked no-context next action: `%s`\n\n' "$(value '.blocked_no_context.continuation_next_action')"
    printf 'Blocked no-context impact status: `%s`\n\n' "$(value '.blocked_no_context.impact_status')"
    printf 'Blocked no-context execution statuses: `%s`\n\n' "$(value '.blocked_no_context.execution_plan_statuses | join(" -> ")')"
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
