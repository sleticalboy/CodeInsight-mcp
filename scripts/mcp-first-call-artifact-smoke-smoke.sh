#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

fail() {
  echo "MCP first-call artifact smoke smoke failed: $*" >&2
  exit 1
}

write_summary_json() {
  local output_file="$1"

  cat >"$output_file" <<'EOF'
{
  "execution_plan_actions": [
    "read_selected_context",
    "use_current_reading_step_suggested_tool",
    "use_continuation_if_needed",
    "review_impact_before_edits"
  ],
  "first_execution_action": "read_selected_context",
  "first_execution_instruction_has_focus": true,
  "first_execution_instruction_has_question": true,
  "first_execution_instruction_has_read_less": true,
  "impact_counts": {
    "callees": 1,
    "callers": 1,
    "dependencies": 3,
    "errors": 0,
    "impacted_files": 2,
    "paths": 1,
    "references": 3,
    "symbols": 2
  },
  "impact_status": "complete",
  "first_context_file": "src/main.ts",
  "first_reading_file": "src/main.ts",
  "first_reading_selection_rank": 1,
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
  "current_reading_step_matches_reading_plan": true,
  "reading_plan": [
    {
      "file": "src/main.ts",
      "selection_rank": 1,
      "next_action": "inspect_seed_file",
      "focus": "Start with seed file context and primary symbols.",
      "question": "What entrypoints define the main flow?",
      "reason": "Read this step to answer: What entrypoints define the main flow? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file",
      "selection_reason": "Selected for high relevance via seed_file",
      "suggested_tool": "file_outline"
    }
  ],
  "root": "/tmp/repo",
  "route_tools": [
    "index_project",
    "project_overview",
    "context_pack",
    "impact_analysis"
  ],
  "execution_plan_reads_in_reading_plan_order": true,
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
  "selected_files": [
    "src/main.ts",
    "src/auth.ts"
  ],
  "server": "codeinsight",
  "status": "pass",
  "suggested_tool": {
    "arguments": {
      "path": "/tmp/repo/src/main.ts"
    },
    "tool": "file_outline"
  },
  "suggested_tool_executed": true,
  "blocked_no_seed": {
    "route_step_status": "blocked_no_seed",
    "seed_strategy": "auto_no_seed",
    "continuation_status": "blocked_no_seed",
    "continuation_next_action": "provide_seed_file_or_symbol",
    "context_files": 0,
    "reading_plan_steps": 0,
    "has_current_reading_step": false,
    "impact_status": "skipped_no_seed",
    "execution_plan_actions": [
      "read_selected_context",
      "use_current_reading_step_suggested_tool",
      "use_continuation_if_needed",
      "review_impact_before_edits"
    ],
    "execution_plan_statuses": [
      "blocked_no_reading_plan",
      "blocked_no_current_reading_step",
      "manual_after_selected_context",
      "skipped_no_seed"
    ]
  },
  "blocked_no_context": {
    "route_step_status": "blocked_no_context",
    "continuation_status": "blocked_no_context",
    "continuation_next_action": "provide_matching_seed_file_or_symbol",
    "truncation_reason": "no_context_for_explicit_seed",
    "context_files": 0,
    "reading_plan_steps": 0,
    "has_current_reading_step": false,
    "impact_status": "skipped_no_context",
    "execution_plan_actions": [
      "read_selected_context",
      "use_current_reading_step_suggested_tool",
      "use_continuation_if_needed",
      "review_impact_before_edits"
    ],
    "execution_plan_statuses": [
      "blocked_no_reading_plan",
      "blocked_no_current_reading_step",
      "manual_after_selected_context",
      "skipped_no_context"
    ]
  },
  "task": "understand app entrypoint flow",
  "token_budget": 1600
}
EOF
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/bin"
  cat >"$TEMP_DIR/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_LOG:?}"
printf 'gh %s\n' "$*" >>"$log"

if [ "$1" = "run" ] && [ "$2" = "list" ]; then
  printf '123456\n'
  exit 0
fi

if [ "$1" = "run" ] && [ "$2" = "download" ]; then
  test "$3" = "123456"
  case " $* " in
    *" --name codeinsight-mcp-first-call "*) ;;
    *) exit 12 ;;
  esac
  output_dir=""
  while [ "$#" -gt 0 ]; do
    if [ "$1" = "--dir" ]; then
      shift
      output_dir="$1"
      break
    fi
    shift
  done
  test -n "$output_dir"
  mkdir -p "$output_dir"
  cp "${CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_FIXTURE:?}" "$output_dir/mcp-first-call.json"
  exit 0
fi

exit 11
EOF
  chmod +x "$TEMP_DIR/bin/gh"

  write_summary_json "$TEMP_DIR/mcp-first-call.json"

  CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_FIXTURE="$TEMP_DIR/mcp-first-call.json" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/mcp-first-call-artifact-smoke.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --dir "$TEMP_DIR/download" \
      123456 >"$TEMP_DIR/output.log"

  grep -Fq 'MCP first-call artifact smoke passed' "$TEMP_DIR/output.log" ||
    fail "missing artifact smoke success output"
  grep -Fq "summary: $TEMP_DIR/download/mcp-first-call.json" "$TEMP_DIR/output.log" ||
    fail "missing summary path output"
  grep -Fq 'first_reading_focus: Start with seed file context and primary symbols.' "$TEMP_DIR/output.log" ||
    fail "missing first reading focus output"
  grep -Fq 'first_reading_question: What entrypoints define the main flow?' "$TEMP_DIR/output.log" ||
    fail "missing first reading question output"
  grep -Fq 'current_reading_step_matches_reading_plan: true' "$TEMP_DIR/output.log" ||
    fail "missing current reading step mirror output"
  grep -Fq 'first_execution_instruction_has_focus: true' "$TEMP_DIR/output.log" ||
    fail "missing first execution focus contract output"
  grep -Fq 'first_execution_instruction_has_read_less: true' "$TEMP_DIR/output.log" ||
    fail "missing first execution read-less contract output"
  grep -Fq 'current_step_instruction_has_focus: true' "$TEMP_DIR/output.log" ||
    fail "missing current-step focus contract output"
  grep -Fq 'first_reading_selection_rank: 1' "$TEMP_DIR/output.log" ||
    fail "missing first reading selection rank output"
  grep -Fq 'source_lines_avoided: 108' "$TEMP_DIR/output.log" ||
    fail "missing source lines avoided output"
  grep -Fq 'read_less_ratio: 10.0x' "$TEMP_DIR/output.log" ||
    fail "missing read-less ratio output"
  grep -Fq 'continuation_status: complete' "$TEMP_DIR/output.log" ||
    fail "missing continuation status output"
  grep -Fq 'first_omitted_omission_reason: -' "$TEMP_DIR/output.log" ||
    fail "missing omitted omission reason output"
  grep -Fq 'blocked_no_seed_status: blocked_no_seed' "$TEMP_DIR/output.log" ||
    fail "missing blocked no-seed status output"
  grep -Fq 'blocked_no_seed_next_action: provide_seed_file_or_symbol' "$TEMP_DIR/output.log" ||
    fail "missing blocked no-seed next action output"
  grep -Fq 'blocked_no_seed_impact_status: skipped_no_seed' "$TEMP_DIR/output.log" ||
    fail "missing blocked no-seed impact status output"
  grep -Fq 'blocked_no_context_status: blocked_no_context' "$TEMP_DIR/output.log" ||
    fail "missing blocked no-context status output"
  grep -Fq 'blocked_no_context_next_action: provide_matching_seed_file_or_symbol' "$TEMP_DIR/output.log" ||
    fail "missing blocked no-context next action output"
  grep -Fq 'blocked_no_context_impact_status: skipped_no_context' "$TEMP_DIR/output.log" ||
    fail "missing blocked no-context impact status output"
  grep -Fq 'gh run download 123456 --repo sleticalboy/CodeInsight-mcp --name codeinsight-mcp-first-call --dir '"$TEMP_DIR/download" "$TEMP_DIR/calls.log" ||
    fail "missing fixed-run artifact download"

  CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_LOG="$TEMP_DIR/latest-calls.log" \
    CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_FIXTURE="$TEMP_DIR/mcp-first-call.json" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/mcp-first-call-artifact-smoke.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --latest-success main >"$TEMP_DIR/latest-output.log"

  grep -Fq 'using latest successful CI run on main: 123456' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest successful run output"
  grep -Fq 'gh run list --repo sleticalboy/CodeInsight-mcp --workflow CI --branch main --status success --limit 1 --json databaseId --jq .[0].databaseId // ""' "$TEMP_DIR/latest-calls.log" ||
    fail "missing latest successful run lookup"
  grep -Fq 'gh run download 123456 --repo sleticalboy/CodeInsight-mcp --name codeinsight-mcp-first-call --dir' "$TEMP_DIR/latest-calls.log" ||
    fail "missing latest successful artifact download"
  grep -Fq 'first_reading_question: What entrypoints define the main flow?' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest first reading question output"
  grep -Fq 'first_reading_focus: Start with seed file context and primary symbols.' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest first reading focus output"
  grep -Fq 'current_reading_step_matches_reading_plan: true' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest current reading step mirror output"
  grep -Fq 'first_execution_instruction_has_focus: true' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest first execution focus contract output"
  grep -Fq 'first_execution_instruction_has_read_less: true' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest first execution read-less contract output"
  grep -Fq 'current_step_instruction_has_focus: true' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest current-step focus contract output"
  grep -Fq 'first_reading_selection_rank: 1' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest first reading selection rank output"
  grep -Fq 'source_lines_avoided: 108' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest source lines avoided output"
  grep -Fq 'read_less_ratio: 10.0x' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest read-less ratio output"
  grep -Fq 'continuation_status: complete' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest continuation status output"
  grep -Fq 'blocked_no_seed_status: blocked_no_seed' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest blocked no-seed status output"
  grep -Fq 'blocked_no_seed_next_action: provide_seed_file_or_symbol' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest blocked no-seed next action output"
  grep -Fq 'blocked_no_seed_impact_status: skipped_no_seed' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest blocked no-seed impact status output"
  grep -Fq 'blocked_no_context_status: blocked_no_context' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest blocked no-context status output"
  grep -Fq 'blocked_no_context_next_action: provide_matching_seed_file_or_symbol' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest blocked no-context next action output"
  grep -Fq 'blocked_no_context_impact_status: skipped_no_context' "$TEMP_DIR/latest-output.log" ||
    fail "missing latest blocked no-context impact status output"

  echo "MCP first-call artifact smoke smoke passed"
}

main "$@"
