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
      and .task == "understand app entrypoint flow"
      and .token_budget == 1600
      and .route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .execution_plan_actions == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]
      and .first_execution_action == "read_selected_context"
      and (.selected_files | type == "array")
      and (.selected_files | length) >= 2
      and (.selected_files | index("src/main.ts"))
      and (.selected_files | index("src/auth.ts"))
      and .first_context_file == "src/main.ts"
      and .first_reading_file == .first_context_file
      and (.first_reading_selection_rank | type == "number")
      and (.reading_plan | type == "array")
      and (.reading_plan | length) >= 1
      and (.reading_plan[0].file == "src/main.ts")
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
      and (.impact_counts.paths | type == "number")' \
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
  echo "first_reading_focus: $(jq -r '.reading_plan[0].focus' "$summary_file")"
  echo "first_reading_question: $(jq -r '.reading_plan[0].question' "$summary_file")"
  echo "first_execution_instruction_has_focus: $(jq -r '.first_execution_instruction_has_focus' "$summary_file")"
  echo "current_step_instruction_has_focus: $(jq -r '.current_step_instruction_has_focus' "$summary_file")"
  echo "first_reading_selection_rank: $(jq -r '.first_reading_selection_rank' "$summary_file")"
  echo "continuation_status: $(jq -r '.continuation_status' "$summary_file")"
  echo "first_omitted_omission_reason: $(jq -r 'if .first_omitted_omission_reason == "" then "-" else .first_omitted_omission_reason end' "$summary_file")"
}

main "$@"
