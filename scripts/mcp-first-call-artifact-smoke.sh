#!/usr/bin/env bash
set -euo pipefail

ARTIFACT_NAME="codeinsight-mcp-first-call"
OUTPUT_DIR=""
OUTPUT_DIR_WAS_SET=0
REPO_ARG=()
RUN_ID=""
LATEST_SUCCESS_BRANCH=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/mcp-first-call-artifact-smoke.sh [options] <ci-run-id>
       scripts/mcp-first-call-artifact-smoke.sh [options] --latest-success BRANCH

Downloads the CI MCP first-call artifact and validates the JSON summary.

Options:
  --repo OWNER/REPO       Pass an explicit GitHub repository to gh.
  --latest-success BRANCH Download from the latest successful CI run on BRANCH.
  --dir PATH             Download into PATH instead of /tmp/codeinsight-mcp-first-call-artifact-<run-id>.
  --artifact-name NAME   Artifact name to download. Default: codeinsight-mcp-first-call.
  -h, --help             Show this help.
EOF
  exit "$status"
}

fail() {
  echo "MCP first-call artifact smoke failed: $*" >&2
  exit 1
}

resolve_latest_success_run() {
  local branch="$1"
  local run_id

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    run_id="$(
      gh run list \
        "${REPO_ARG[@]}" \
        --workflow CI \
        --branch "$branch" \
        --status success \
        --limit 1 \
        --json databaseId \
        --jq '.[0].databaseId // ""'
    )"
  else
    run_id="$(
      gh run list \
        --workflow CI \
        --branch "$branch" \
        --status success \
        --limit 1 \
        --json databaseId \
        --jq '.[0].databaseId // ""'
    )"
  fi

  if [ -z "$run_id" ]; then
    fail "no successful CI run found for branch: $branch"
  fi
  printf "%s" "$run_id"
}

validate_summary_json() {
  local summary_file="$1"

  if ! jq -e \
    '.status == "pass"
      and .server == "codeinsight"
      and .task == "inspect src/auth.ts before editing login behavior"
      and .token_budget == 1600
      and .route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .execution_plan_actions == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]
      and .first_execution_action == "read_selected_context"
      and (.selected_files | type == "array")
      and (.selected_files | length) >= 1
      and (.selected_files | index("src/auth.ts"))
      and .seed_strategy == "auto_task_path"
      and .first_seed_source == "task_path"
      and .first_seed_value == "src/auth.ts"
      and (.selected_seeds | type == "array")
      and (.selected_seeds | length) >= 1
      and .selected_seeds[0].source == "task_path"
      and .selected_seeds[0].value == "src/auth.ts"
      and .first_context_file == "src/auth.ts"
      and .first_reading_file == .first_context_file
      and (.first_reading_selection_rank | type == "number")
      and (.route_quality | type == "object")
      and .route_quality.level == .route_quality_level
      and .route_quality.score == .route_quality_score
      and .route_quality.evidence_count == .route_quality_evidence_count
      and .route_quality.recommended_action == .route_quality_recommended_action
      and .route_quality.level == "high"
      and .route_quality.score >= 80
      and .route_quality.evidence_count >= 1
      and (.route_quality.evidence_sources | type == "array")
      and (.route_quality.warnings | type == "array")
      and (.route_quality.recommended_action | type == "string" and length > 0)
      and (.routing_decision | type == "object")
      and .routing_decision.route_quality == .route_quality
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
      and .current_reading_step_matches_reading_plan == true
      and (.reading_plan | type == "array")
      and (.reading_plan | length) >= 1
      and (.reading_plan[0].file == "src/auth.ts")
      and (.reading_plan[0].selection_rank == .first_reading_selection_rank)
      and (.reading_plan[0].next_action == "inspect_seed_file")
      and (.reading_plan[0].focus | type == "string" and length > 0)
      and (.reading_plan[0].question | type == "string" and length > 0)
      and (.reading_plan[0].selection_reason | type == "string" and length > 0)
      and (.reading_plan[0].suggested_tool | type == "string" and length > 0)
      and (.reading_plan[0] as $step
        | ($step.reason | type == "string")
        and ($step.reason | contains($step.question))
        and ($step.reason | contains("If deeper evidence is needed, call "))
        and ($step.reason | contains($step.suggested_tool))
        and ($step.reason | contains("Selection reason:")))
      and .execution_plan_reads_in_reading_plan_order == true
      and .first_execution_instruction_has_focus == true
      and .first_execution_instruction_has_read_less == true
      and .current_step_instruction_has_focus == true
      and .current_step_suggested_tool_matches_reading_plan == true
      and .continuation_after_selected_context == true
      and (.continuation_status | type == "string")
      and (.continuation_next_action | type == "string" and length > 0)
      and (.first_omitted_file | type == "string")
      and ((.first_omitted_selection_rank | type == "number") or (.first_omitted_selection_rank == null))
      and (.first_omitted_omission_reason | type == "string")
      and (.first_omitted_next_action | type == "string")
      and (.suggested_tool.tool | type == "string" and length > 0)
      and .suggested_tool.tool == .reading_plan[0].suggested_tool
      and (.suggested_tool.arguments | type == "object")
      and .suggested_tool_executed == true
      and .impact_status == "complete"
      and (.impact_counts | type == "object")
      and (.impact_counts.impacted_files | type == "number")
      and .impact_counts.impacted_files >= 1
      and (.impact_counts.paths | type == "number")
      and .blocked_no_seed.route_step_status == "blocked_no_seed"
      and .blocked_no_seed.seed_strategy == "auto_no_seed"
      and .blocked_no_seed.continuation_status == "blocked_no_seed"
      and .blocked_no_seed.continuation_next_action == "provide_seed_file_or_symbol"
      and .blocked_no_seed.context_files == 0
      and .blocked_no_seed.reading_plan_steps == 0
      and .blocked_no_seed.has_current_reading_step == false
      and .blocked_no_seed.route_quality.level == "blocked"
      and .blocked_no_seed.route_quality.score == 0
      and .blocked_no_seed.route_quality.evidence_count == 0
      and .blocked_no_seed.route_quality.recommended_action == "provide_seed_file_or_symbol"
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
      and .blocked_no_context.route_quality.level == "blocked"
      and .blocked_no_context.route_quality.score == 0
      and .blocked_no_context.route_quality.evidence_count == 0
      and .blocked_no_context.route_quality.recommended_action == "provide_matching_seed_file_or_symbol"
      and .blocked_no_context.impact_status == "skipped_no_context"
      and .blocked_no_context.execution_plan_actions == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]
      and .blocked_no_context.execution_plan_statuses == ["blocked_no_reading_plan", "blocked_no_current_reading_step", "manual_after_selected_context", "skipped_no_context"]
      and .blocked_unindexed_task_path.route_step_status == "blocked_unindexed_task_path"
      and .blocked_unindexed_task_path.seed_strategy == "auto_task_path_unindexed"
      and .blocked_unindexed_task_path.first_seed_source == "task_path_unindexed"
      and .blocked_unindexed_task_path.first_seed_value == "src/main.ts"
      and .blocked_unindexed_task_path.continuation_status == "blocked_unindexed_task_path"
      and .blocked_unindexed_task_path.continuation_next_action == "index_or_update_scope_for_task_path"
      and .blocked_unindexed_task_path.truncation_reason == "unindexed_task_path"
      and .blocked_unindexed_task_path.context_files == 0
      and .blocked_unindexed_task_path.reading_plan_steps == 0
      and .blocked_unindexed_task_path.has_current_reading_step == false
      and .blocked_unindexed_task_path.route_quality.level == "blocked"
      and .blocked_unindexed_task_path.route_quality.score == 0
      and .blocked_unindexed_task_path.route_quality.evidence_count == 0
      and .blocked_unindexed_task_path.route_quality.recommended_action == "index_or_update_scope_for_task_path"
      and .blocked_unindexed_task_path.impact_status == "skipped_unindexed_task_path"
      and .blocked_unindexed_task_path.execution_plan_actions == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]
      and .blocked_unindexed_task_path.execution_plan_statuses == ["blocked_no_reading_plan", "blocked_no_current_reading_step", "manual_after_selected_context", "skipped_unindexed_task_path"]
      and .blocked_unindexed_task_path.continuation_message_has_scope_hint == true
      and .blocked_unindexed_task_path.impact_instruction_has_skipped_reason == true' \
    "$summary_file" >/dev/null; then
    fail "$summary_file does not match expected MCP first-call summary"
  fi
}

main() {
  local summary_count
  local summary_file

  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h | --help)
        usage 0
        ;;
      --repo)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        REPO_ARG=(--repo "$1")
        ;;
      --latest-success)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        LATEST_SUCCESS_BRANCH="$1"
        ;;
      --dir)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        OUTPUT_DIR="$1"
        OUTPUT_DIR_WAS_SET=1
        ;;
      --artifact-name)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        ARTIFACT_NAME="$1"
        ;;
      --)
        shift
        break
        ;;
      -*)
        usage
        ;;
      *)
        if [ -n "$RUN_ID" ]; then
          usage
        fi
        RUN_ID="$1"
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    if [ -n "$RUN_ID" ]; then
      usage
    fi
    RUN_ID="$1"
    shift
  done

  if ! command -v gh >/dev/null 2>&1; then
    fail "missing required command: gh"
  fi
  if ! command -v jq >/dev/null 2>&1; then
    fail "missing required command: jq"
  fi
  if [ -n "$LATEST_SUCCESS_BRANCH" ] && [ -n "$RUN_ID" ]; then
    usage
  fi
  if [ -z "$RUN_ID" ]; then
    if [ -n "$LATEST_SUCCESS_BRANCH" ]; then
      RUN_ID="$(resolve_latest_success_run "$LATEST_SUCCESS_BRANCH")"
      echo "using latest successful CI run on $LATEST_SUCCESS_BRANCH: $RUN_ID"
    else
      usage
    fi
  fi
  if [ -z "$OUTPUT_DIR" ]; then
    OUTPUT_DIR="${TMPDIR:-/tmp}/codeinsight-mcp-first-call-artifact-$RUN_ID"
  fi

  if [ -e "$OUTPUT_DIR" ]; then
    if [ "$OUTPUT_DIR_WAS_SET" -eq 1 ]; then
      if [ -n "$(find "$OUTPUT_DIR" -mindepth 1 -print -quit)" ]; then
        fail "output directory already exists and is not empty: $OUTPUT_DIR"
      fi
    else
      case "$OUTPUT_DIR" in
        "${TMPDIR:-/tmp}"/codeinsight-mcp-first-call-artifact-*) rm -rf "$OUTPUT_DIR" ;;
        *) fail "refusing to replace unexpected output directory: $OUTPUT_DIR" ;;
      esac
    fi
  fi
  mkdir -p "$OUTPUT_DIR"

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    gh run download "$RUN_ID" \
      "${REPO_ARG[@]}" \
      --name "$ARTIFACT_NAME" \
      --dir "$OUTPUT_DIR"
  else
    gh run download "$RUN_ID" \
      --name "$ARTIFACT_NAME" \
      --dir "$OUTPUT_DIR"
  fi

  summary_count="$(find "$OUTPUT_DIR" -type f -name '*.json' | wc -l | tr -d ' ')"
  if [ "$summary_count" -ne 1 ]; then
    find "$OUTPUT_DIR" -type f -print >&2
    fail "expected exactly one JSON summary in $OUTPUT_DIR, found $summary_count"
  fi
  summary_file="$(find "$OUTPUT_DIR" -type f -name '*.json' | head -n 1)"

  validate_summary_json "$summary_file"

  echo "MCP first-call artifact smoke passed"
  echo "summary: $summary_file"
  echo "seed_strategy: $(jq -r '.seed_strategy' "$summary_file")"
  echo "first_seed_source: $(jq -r '.first_seed_source' "$summary_file")"
  echo "first_seed_value: $(jq -r '.first_seed_value' "$summary_file")"
  echo "first_reading_focus: $(jq -r '.reading_plan[0].focus' "$summary_file")"
  echo "first_reading_question: $(jq -r '.reading_plan[0].question' "$summary_file")"
  echo "route_quality: $(jq -r '.route_quality.level + " " + (.route_quality.score | tostring) + "/100 evidence=" + (.route_quality.evidence_count | tostring)' "$summary_file")"
  echo "route_quality_recommended_action: $(jq -r '.route_quality.recommended_action' "$summary_file")"
  echo "current_reading_step_matches_reading_plan: $(jq -r '.current_reading_step_matches_reading_plan' "$summary_file")"
  echo "first_execution_instruction_has_focus: $(jq -r '.first_execution_instruction_has_focus' "$summary_file")"
  echo "first_execution_instruction_has_read_less: $(jq -r '.first_execution_instruction_has_read_less' "$summary_file")"
  echo "current_step_instruction_has_focus: $(jq -r '.current_step_instruction_has_focus' "$summary_file")"
  echo "first_reading_selection_rank: $(jq -r '.first_reading_selection_rank' "$summary_file")"
  echo "source_lines_avoided: $(jq -r '.source_lines_avoided' "$summary_file")"
  echo "read_less_ratio: $(jq -r '.read_less_ratio' "$summary_file")"
  echo "continuation_status: $(jq -r '.continuation_status' "$summary_file")"
  echo "first_omitted_omission_reason: $(jq -r 'if .first_omitted_omission_reason == "" then "-" else .first_omitted_omission_reason end' "$summary_file")"
  echo "blocked_no_seed_status: $(jq -r '.blocked_no_seed.continuation_status' "$summary_file")"
  echo "blocked_no_seed_next_action: $(jq -r '.blocked_no_seed.continuation_next_action' "$summary_file")"
  echo "blocked_no_seed_route_quality: $(jq -r '.blocked_no_seed.route_quality.level + " " + (.blocked_no_seed.route_quality.score | tostring) + "/100 -> " + .blocked_no_seed.route_quality.recommended_action' "$summary_file")"
  echo "blocked_no_seed_impact_status: $(jq -r '.blocked_no_seed.impact_status' "$summary_file")"
  echo "blocked_no_context_status: $(jq -r '.blocked_no_context.continuation_status' "$summary_file")"
  echo "blocked_no_context_next_action: $(jq -r '.blocked_no_context.continuation_next_action' "$summary_file")"
  echo "blocked_no_context_route_quality: $(jq -r '.blocked_no_context.route_quality.level + " " + (.blocked_no_context.route_quality.score | tostring) + "/100 -> " + .blocked_no_context.route_quality.recommended_action' "$summary_file")"
  echo "blocked_no_context_impact_status: $(jq -r '.blocked_no_context.impact_status' "$summary_file")"
  echo "blocked_unindexed_task_path_status: $(jq -r '.blocked_unindexed_task_path.continuation_status' "$summary_file")"
  echo "blocked_unindexed_task_path_seed_strategy: $(jq -r '.blocked_unindexed_task_path.seed_strategy' "$summary_file")"
  echo "blocked_unindexed_task_path_first_seed_source: $(jq -r '.blocked_unindexed_task_path.first_seed_source' "$summary_file")"
  echo "blocked_unindexed_task_path_first_seed_value: $(jq -r '.blocked_unindexed_task_path.first_seed_value' "$summary_file")"
  echo "blocked_unindexed_task_path_next_action: $(jq -r '.blocked_unindexed_task_path.continuation_next_action' "$summary_file")"
  echo "blocked_unindexed_task_path_route_quality: $(jq -r '.blocked_unindexed_task_path.route_quality.level + " " + (.blocked_unindexed_task_path.route_quality.score | tostring) + "/100 -> " + .blocked_unindexed_task_path.route_quality.recommended_action' "$summary_file")"
  echo "blocked_unindexed_task_path_impact_status: $(jq -r '.blocked_unindexed_task_path.impact_status' "$summary_file")"
  echo "blocked_unindexed_task_path_scope_hint: $(jq -r '.blocked_unindexed_task_path.continuation_message_has_scope_hint' "$summary_file")"
}

main "$@"
