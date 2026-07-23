#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

fail() {
  echo "mcp first-call step summary smoke failed: $*" >&2
  exit 1
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
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
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local summary_json="$TEMP_DIR/mcp-first-call.json"
  local summary_md="$TEMP_DIR/summary.md"

  cat >"$summary_json" <<'EOF'
{
  "status": "pass",
  "server": "codeinsight",
  "root": "/tmp/repo",
  "task": "inspect src/auth.ts before editing login behavior",
  "token_budget": 1600,
  "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
  "selected_files": ["src/auth.ts", "src/audit.ts"],
  "seed_strategy": "auto_task_path",
  "selected_seeds": [
    {
      "kind": "file",
      "role": "source",
      "source": "task_path",
      "value": "src/auth.ts"
    }
  ],
  "first_seed_source": "task_path",
  "first_seed_value": "src/auth.ts",
  "first_context_file": "src/auth.ts",
  "first_reading_file": "src/auth.ts",
  "first_reading_selection_rank": 1,
  "route_quality": {
    "level": "high",
    "score": 96,
    "evidence_count": 4,
    "evidence_sources": ["seed file", "call graph"],
    "warnings": [],
    "recommended_action": "read_selected_context"
  },
  "route_quality_level": "high",
  "route_quality_score": 96,
  "route_quality_evidence_count": 4,
  "route_quality_recommended_action": "read_selected_context",
  "routing_decision": {
    "seed_strategy": "auto_task_path",
    "route_quality": {
      "level": "high",
      "score": 96,
      "evidence_count": 4,
      "evidence_sources": ["seed file", "call graph"],
      "warnings": [],
      "recommended_action": "read_selected_context"
    },
    "first_seed_source": "task_path",
    "first_seed_value": "src/auth.ts",
    "first_file": "src/auth.ts",
    "first_selection_rank": 1,
    "first_suggested_tool": "file_outline",
    "line_reduction": "90.0%",
    "read_less_ratio": "10.0x",
    "continuation_status": "complete",
    "continuation_next_action": "read_selected_context",
    "impact_status": "complete"
  },
  "current_reading_step_matches_reading_plan": true,
  "context_pack_read_less": {
    "baseline_source_lines": 120,
    "selected_source_lines": 12,
    "source_lines_avoided": 108,
    "line_reduction": "90.0%",
    "read_less_ratio": "10.0x"
  },
  "baseline_source_lines": 120,
  "selected_source_lines": 12,
  "source_lines_avoided": 108,
  "line_reduction": "90.0%",
  "read_less_ratio": "10.0x",
  "reading_plan": [
    {
      "file": "src/auth.ts",
      "selection_rank": 1,
      "next_action": "inspect_seed_file",
      "focus": "Start with seed file authentication and session boundaries.",
      "question": "Where are authentication decisions, credentials, or session boundaries handled here?",
      "reason": "Read this step to answer: Where are authentication decisions, credentials, or session boundaries handled here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file",
      "selection_reason": "Selected for high relevance via seed_file",
      "suggested_tool": "file_outline"
    }
  ],
  "execution_plan_actions": ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"],
  "execution_plan_reads_in_reading_plan_order": true,
  "first_execution_action": "read_selected_context",
  "first_execution_instruction_has_focus": true,
  "first_execution_instruction_has_question": true,
  "first_execution_instruction_has_read_less": true,
  "current_step_suggested_tool_matches_reading_plan": true,
  "current_step_instruction_has_focus": true,
  "current_step_instruction_has_question": true,
  "current_step_instruction_has_action": true,
  "continuation_after_selected_context": true,
  "continuation_status": "complete",
  "continuation_next_action": "read_selected_context",
  "first_omitted_file": "",
  "first_omitted_selection_rank": null,
  "first_omitted_omission_reason": "",
  "first_omitted_next_action": "",
  "suggested_tool": {
    "tool": "file_outline",
    "arguments": {
      "path": "/tmp/repo/src/auth.ts"
    }
  },
  "suggested_tool_executed": true,
  "impact_status": "complete",
  "impact_counts": {
    "impacted_files": 2,
    "paths": 1
  },
  "impact_execution_suggested_tool": "impact_analysis",
  "impact_suggested_checks": 3,
  "impact_execution_suggested_checks": 3,
  "impact_first_suggested_check": {
    "kind": "review",
    "command": "cargo test --locked"
  },
  "impact_execution_instruction_has_first_check": true,
  "blocked_no_seed": {
    "route_step_status": "blocked_no_seed",
    "seed_strategy": "auto_no_seed",
    "continuation_status": "blocked_no_seed",
    "continuation_next_action": "provide_seed_file_or_symbol",
    "context_files": 0,
    "reading_plan_steps": 0,
    "has_current_reading_step": false,
    "route_quality": {
      "level": "blocked",
      "score": 0,
      "evidence_count": 0,
      "evidence_sources": [],
      "warnings": ["No reading plan was produced; context status is blocked_no_seed."],
      "recommended_action": "provide_seed_file_or_symbol"
    },
    "impact_status": "skipped_no_seed",
    "execution_plan_actions": ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"],
    "execution_plan_statuses": ["blocked_no_reading_plan", "blocked_no_current_reading_step", "manual_after_selected_context", "skipped_no_seed"]
  },
  "blocked_no_context": {
    "route_step_status": "blocked_no_context",
    "continuation_status": "blocked_no_context",
    "continuation_next_action": "provide_matching_seed_file_or_symbol",
    "truncation_reason": "no_context_for_explicit_seed",
    "context_files": 0,
    "reading_plan_steps": 0,
    "has_current_reading_step": false,
    "route_quality": {
      "level": "blocked",
      "score": 0,
      "evidence_count": 0,
      "evidence_sources": [],
      "warnings": ["No reading plan was produced; context status is blocked_no_context."],
      "recommended_action": "provide_matching_seed_file_or_symbol"
    },
    "impact_status": "skipped_no_context",
    "execution_plan_actions": ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"],
    "execution_plan_statuses": ["blocked_no_reading_plan", "blocked_no_current_reading_step", "manual_after_selected_context", "skipped_no_context"]
  },
  "blocked_unindexed_task_path": {
    "route_step_status": "blocked_unindexed_task_path",
    "seed_strategy": "auto_task_path_unindexed",
    "first_seed_source": "task_path_unindexed",
    "first_seed_value": "src/main.ts",
    "continuation_status": "blocked_unindexed_task_path",
    "continuation_next_action": "index_or_update_scope_for_task_path",
    "truncation_reason": "unindexed_task_path",
    "context_files": 0,
    "reading_plan_steps": 0,
    "has_current_reading_step": false,
    "route_quality": {
      "level": "blocked",
      "score": 0,
      "evidence_count": 0,
      "evidence_sources": [],
      "warnings": ["No reading plan was produced; context status is blocked_unindexed_task_path."],
      "recommended_action": "index_or_update_scope_for_task_path"
    },
    "impact_status": "skipped_unindexed_task_path",
    "execution_plan_actions": ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"],
    "execution_plan_statuses": ["blocked_no_reading_plan", "blocked_no_current_reading_step", "manual_after_selected_context", "skipped_unindexed_task_path"],
    "continuation_message_has_scope_hint": true,
    "impact_instruction_has_skipped_reason": true
  }
}
EOF

  GITHUB_STEP_SUMMARY="$summary_md" \
    "$ROOT_DIR/scripts/mcp-first-call-step-summary.sh" \
      "$summary_json" \
      codeinsight-mcp-first-call \
      https://example.com/artifact \
      https://example.com/run >/dev/null

  require_literal "$summary_md" "## MCP First-Call Smoke" "summary heading"
  require_literal "$summary_md" 'Status: `pass`' "status"
  require_literal "$summary_md" 'Task: `inspect src/auth.ts before editing login behavior`' "task"
  require_literal "$summary_md" 'Route: `index_project -> project_overview -> context_pack -> impact_analysis`' "route"
  require_literal "$summary_md" 'Execution plan: `read_selected_context -> use_current_reading_step_suggested_tool -> use_continuation_if_needed -> review_impact_before_edits`' "execution plan"
  require_literal "$summary_md" 'Selected files: `src/auth.ts`, `src/audit.ts`' "selected files"
  require_literal "$summary_md" 'Seed strategy: `auto_task_path`' "seed strategy"
  require_literal "$summary_md" 'First seed source: `task_path`' "first seed source"
  require_literal "$summary_md" 'First seed value: `src/auth.ts`' "first seed value"
  require_literal "$summary_md" 'First context file: `src/auth.ts`' "first context file"
  require_literal "$summary_md" 'First reading file: `src/auth.ts`' "first reading file"
  require_literal "$summary_md" 'First reading selection rank: `1`' "first reading selection rank"
  require_literal "$summary_md" 'Route quality: `high` (`96/100`, `4` evidence signals)' "route quality"
  require_literal "$summary_md" 'Route quality recommended action: `read_selected_context`' "route quality action"
  require_literal "$summary_md" 'Route quality evidence sources: `seed file`, `call graph`' "route quality evidence sources"
  require_literal "$summary_md" 'Route quality warnings: `-`' "route quality warnings"
  require_literal "$summary_md" 'Routing decision first seed: `task_path:src/auth.ts`' "routing decision first seed"
  require_literal "$summary_md" 'Routing decision first file: `src/auth.ts`' "routing decision first file"
  require_literal "$summary_md" 'Routing decision quality: `high` (`96/100`)' "routing decision quality"
  require_literal "$summary_md" 'Routing decision suggested tool: `file_outline`' "routing decision suggested tool"
  require_literal "$summary_md" 'Routing decision continuation: `complete -> read_selected_context`' "routing decision continuation"
  require_literal "$summary_md" 'Routing decision impact status: `complete`' "routing decision impact status"
  require_literal "$summary_md" 'Current reading step mirror contract: `true`' "current reading step mirror contract"
  require_literal "$summary_md" 'Blind first-read baseline: `120` source lines' "blind first-read baseline"
  require_literal "$summary_md" 'Routed first-read: `12` source lines' "routed first-read"
  require_literal "$summary_md" 'Source lines avoided: `108`' "source lines avoided"
  require_literal "$summary_md" 'First-read line reduction: `90.0%`' "first-read line reduction"
  require_literal "$summary_md" 'Read less: `10.0x`' "read-less ratio"
  require_literal "$summary_md" 'First next action: `inspect_seed_file`' "first next action"
  require_literal "$summary_md" 'First reading focus: `Start with seed file authentication and session boundaries.`' "first reading focus"
  require_literal "$summary_md" 'First reading question: `Where are authentication decisions, credentials, or session boundaries handled here?`' "first reading question"
  require_literal "$summary_md" 'Reading order contract: `true`' "reading order contract"
  require_literal "$summary_md" 'First execution instruction focus contract: `true`' "first execution instruction focus contract"
  require_literal "$summary_md" 'First execution instruction question contract: `true`' "first execution instruction question contract"
  require_literal "$summary_md" 'First execution instruction read-less contract: `true`' "first execution instruction read-less contract"
  require_literal "$summary_md" 'Suggested tool handoff contract: `true`' "suggested tool handoff contract"
  require_literal "$summary_md" 'Current-step instruction focus contract: `true`' "current-step instruction focus contract"
  require_literal "$summary_md" 'Current-step instruction question contract: `true`' "current-step instruction question contract"
  require_literal "$summary_md" 'Current-step instruction action contract: `true`' "current-step instruction action contract"
  require_literal "$summary_md" 'Continuation timing contract: `true`' "continuation timing contract"
  require_literal "$summary_md" 'Continuation status: `complete`' "continuation status"
  require_literal "$summary_md" 'Continuation next action: `read_selected_context`' "continuation next action"
  require_literal "$summary_md" 'First omitted file: `-`' "first omitted file"
  require_literal "$summary_md" 'First omitted selection rank: `-`' "first omitted selection rank"
  require_literal "$summary_md" 'First omitted omission reason: `-`' "first omitted omission reason"
  require_literal "$summary_md" 'Suggested tool: `file_outline`' "suggested tool"
  require_literal "$summary_md" 'Suggested tool executed: `true`' "suggested tool execution"
  require_literal "$summary_md" 'Impact status: `complete`' "impact status"
  require_literal "$summary_md" 'Impacted files: `2`' "impacted files"
  require_literal "$summary_md" 'Impact execution suggested tool: `impact_analysis`' "impact execution suggested tool"
  require_literal "$summary_md" 'Impact execution suggested checks: `3`' "impact execution suggested checks"
  require_literal "$summary_md" 'Impact first suggested check: `cargo test --locked`' "impact first suggested check"
  require_literal "$summary_md" 'Blocked no-seed status: `blocked_no_seed`' "blocked no-seed status"
  require_literal "$summary_md" 'Blocked no-seed next action: `provide_seed_file_or_symbol`' "blocked no-seed next action"
  require_literal "$summary_md" 'Blocked no-seed route quality: `blocked` (`0/100`) -> `provide_seed_file_or_symbol`' "blocked no-seed route quality"
  require_literal "$summary_md" 'Blocked no-seed execution statuses: `blocked_no_reading_plan -> blocked_no_current_reading_step -> manual_after_selected_context -> skipped_no_seed`' "blocked no-seed execution statuses"
  require_literal "$summary_md" 'Blocked no-context status: `blocked_no_context`' "blocked no-context status"
  require_literal "$summary_md" 'Blocked no-context next action: `provide_matching_seed_file_or_symbol`' "blocked no-context next action"
  require_literal "$summary_md" 'Blocked no-context route quality: `blocked` (`0/100`) -> `provide_matching_seed_file_or_symbol`' "blocked no-context route quality"
  require_literal "$summary_md" 'Blocked no-context impact status: `skipped_no_context`' "blocked no-context impact status"
  require_literal "$summary_md" 'Blocked no-context execution statuses: `blocked_no_reading_plan -> blocked_no_current_reading_step -> manual_after_selected_context -> skipped_no_context`' "blocked no-context execution statuses"
  require_literal "$summary_md" 'Blocked unindexed path status: `blocked_unindexed_task_path`' "blocked unindexed path status"
  require_literal "$summary_md" 'Blocked unindexed path seed strategy: `auto_task_path_unindexed`' "blocked unindexed path seed strategy"
  require_literal "$summary_md" 'Blocked unindexed path first seed: `task_path_unindexed:src/main.ts`' "blocked unindexed path first seed"
  require_literal "$summary_md" 'Blocked unindexed path next action: `index_or_update_scope_for_task_path`' "blocked unindexed path next action"
  require_literal "$summary_md" 'Blocked unindexed path route quality: `blocked` (`0/100`) -> `index_or_update_scope_for_task_path`' "blocked unindexed path route quality"
  require_literal "$summary_md" 'Blocked unindexed path impact status: `skipped_unindexed_task_path`' "blocked unindexed path impact status"
  require_literal "$summary_md" 'Blocked unindexed path scope hint: `true`' "blocked unindexed path scope hint"
  require_literal "$summary_md" 'Workflow artifact: [`codeinsight-mcp-first-call`](https://example.com/artifact)' "artifact link"

  echo "mcp first-call step summary smoke passed"
}

main "$@"
