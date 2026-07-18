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
  "task": "understand app entrypoint flow",
  "token_budget": 1600,
  "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
  "selected_files": ["src/main.ts", "src/auth.ts"],
  "first_context_file": "src/main.ts",
  "first_reading_file": "src/main.ts",
  "first_reading_selection_rank": 1,
  "reading_plan": [
    {
      "file": "src/main.ts",
      "selection_rank": 1,
      "next_action": "inspect_seed_file",
      "question": "What entrypoints define the main flow?",
      "reason": "Read this step to answer: What entrypoints define the main flow? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file",
      "selection_reason": "Selected for high relevance via seed_file",
      "suggested_tool": "file_outline"
    }
  ],
  "execution_plan_actions": ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"],
  "execution_plan_reads_in_reading_plan_order": true,
  "first_execution_action": "read_selected_context",
  "current_step_suggested_tool_matches_reading_plan": true,
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
      "path": "/tmp/repo/src/main.ts"
    }
  },
  "suggested_tool_executed": true,
  "impact_status": "complete",
  "impact_counts": {
    "impacted_files": 2,
    "paths": 1
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
  require_literal "$summary_md" 'Task: `understand app entrypoint flow`' "task"
  require_literal "$summary_md" 'Route: `index_project -> project_overview -> context_pack -> impact_analysis`' "route"
  require_literal "$summary_md" 'Execution plan: `read_selected_context -> use_current_reading_step_suggested_tool -> use_continuation_if_needed -> review_impact_before_edits`' "execution plan"
  require_literal "$summary_md" 'Selected files: `src/main.ts`, `src/auth.ts`' "selected files"
  require_literal "$summary_md" 'First context file: `src/main.ts`' "first context file"
  require_literal "$summary_md" 'First reading file: `src/main.ts`' "first reading file"
  require_literal "$summary_md" 'First reading selection rank: `1`' "first reading selection rank"
  require_literal "$summary_md" 'First next action: `inspect_seed_file`' "first next action"
  require_literal "$summary_md" 'First reading question: `What entrypoints define the main flow?`' "first reading question"
  require_literal "$summary_md" 'Reading order contract: `true`' "reading order contract"
  require_literal "$summary_md" 'Suggested tool handoff contract: `true`' "suggested tool handoff contract"
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
  require_literal "$summary_md" 'Workflow artifact: [`codeinsight-mcp-first-call`](https://example.com/artifact)' "artifact link"

  echo "mcp first-call step summary smoke passed"
}

main "$@"
