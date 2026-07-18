#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${CODEINSIGHT_ADOPTION_ROOT:-}"
TASK="${CODEINSIGHT_ADOPTION_TASK:-understand the main application entrypoint}"
TOKEN_BUDGET="${CODEINSIGHT_ADOPTION_TOKEN_BUDGET:-6000}"
OUTPUT_DIR="${CODEINSIGHT_ADOPTION_OUTPUT_DIR:-}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
FORCE_INDEX="${CODEINSIGHT_ADOPTION_FORCE_INDEX:-1}"
PRINT_SNIPPET="${CODEINSIGHT_ADOPTION_PRINT_SNIPPET:-0}"
ISSUE_TEMPLATE="${CODEINSIGHT_ADOPTION_ISSUE_TEMPLATE:-0}"
LOCAL_REPO_EVIDENCE_SCRIPT="${CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT:-$ROOT_DIR/scripts/local-repo-evidence.sh}"
MCP_FIRST_CALL_SMOKE_SCRIPT="${CODEINSIGHT_MCP_FIRST_CALL_SMOKE_SCRIPT:-$ROOT_DIR/scripts/mcp-first-call-smoke.sh}"

usage() {
  cat <<'EOF'
usage: scripts/adoption-evidence.sh [REPO_ROOT] [options]

Builds a user-facing adoption evidence bundle for a local repository:
local first-read evidence, raw agent_route JSON, compact local evidence JSON,
MCP first-call JSON, and an aggregate Markdown/JSON summary.

Options:
  --root PATH           Repository root. Also accepted as the first argument.
  --task TEXT           Task for local evidence and MCP first-call checks.
  --token-budget N      Token budget for context routing. Default: 6000.
  --output-dir PATH     Evidence output directory. Default: /tmp/codeinsight-adoption-evidence.
  --bin PATH            Use a specific codeinsight binary.
  --print-snippet       Print a copyable terminal summary after writing files.
  --issue-template      Write a copyable issue-template.md into the output directory.
  --no-force-index      Reuse the existing index when available.
  -h, --help            Show this help text.

Environment:
  CODEINSIGHT_ADOPTION_ROOT
  CODEINSIGHT_ADOPTION_TASK
  CODEINSIGHT_ADOPTION_TOKEN_BUDGET
  CODEINSIGHT_ADOPTION_OUTPUT_DIR
  CODEINSIGHT_ADOPTION_FORCE_INDEX
  CODEINSIGHT_ADOPTION_PRINT_SNIPPET
  CODEINSIGHT_ADOPTION_ISSUE_TEMPLATE
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "adoption evidence failed [unexpected]: $*" >&2
  exit 1
}

fail_with() {
  local category="$1"
  shift
  echo "adoption evidence failed [${category}]: $*" >&2
  exit 1
}

fail_step() {
  local category="$1"
  local log_file="$2"
  local description="$3"

  echo "adoption evidence failed [${category}]: ${description}" >&2
  if [ -s "$log_file" ]; then
    echo "--- ${category} stderr ---" >&2
    tail -40 "$log_file" >&2
  fi
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail_with prerequisite "missing required command: $1"
  fi
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --root)
        [ "$#" -ge 2 ] || fail_with usage "--root requires a path"
        REPO_ROOT="$2"
        shift 2
        ;;
      --task)
        [ "$#" -ge 2 ] || fail_with usage "--task requires text"
        TASK="$2"
        shift 2
        ;;
      --token-budget)
        [ "$#" -ge 2 ] || fail_with usage "--token-budget requires a number"
        TOKEN_BUDGET="$2"
        shift 2
        ;;
      --output-dir)
        [ "$#" -ge 2 ] || fail_with usage "--output-dir requires a path"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --bin)
        [ "$#" -ge 2 ] || fail_with usage "--bin requires a path"
        CODEINSIGHT_BIN="$2"
        shift 2
        ;;
      --print-snippet)
        PRINT_SNIPPET="1"
        shift
        ;;
      --issue-template)
        ISSUE_TEMPLATE="1"
        shift
        ;;
      --no-force-index)
        FORCE_INDEX="0"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        fail_with usage "unknown argument: $1"
        ;;
      *)
        if [ -n "$REPO_ROOT" ]; then
          fail_with usage "unexpected positional argument: $1"
        fi
        REPO_ROOT="$1"
        shift
        ;;
    esac
  done
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

write_markdown_summary() {
  local target="$1"
  local local_summary="$2"
  local mcp_summary="$3"
  local issue_template_path="${4:-}"

  {
    echo "# CodeInsight Adoption Evidence"
    echo
    echo "- Repository: \`$REPO_ROOT\`"
    echo "- Task: \`$TASK\`"
    echo "- Token budget: \`$TOKEN_BUDGET\`"
    echo "- Status: \`pass\`"
    echo
    echo "## Key Results"
    echo
    echo "- Route: \`$(json_value "$local_summary" '.route_tools | join(" -> ")')\`"
    echo "- Selected context: \`$(json_value "$local_summary" '.metrics.selected_lines')/$(json_value "$local_summary" '.metrics.total_lines')\` source lines, \`$(json_value "$local_summary" '.metrics.line_reduction')\` reduction"
    echo "- Seed strategy: \`$(json_value "$local_summary" '.metrics.seed_strategy // "-"')\`"
    echo "- Selected seeds: \`$(json_value "$local_summary" '.metrics.selected_seed_count // 0')\`"
    echo "- First seed source: \`$(json_value "$local_summary" '.metrics.first_seed_source // "-"')\`"
    echo "- Companion entrypoint: \`$(json_value "$local_summary" '(.metrics.companion_entrypoint // "") as $value | if $value == "" then "-" else $value end')\`"
    echo "- First selected file: \`$(json_value "$local_summary" '.metrics.first_file')\`"
    echo "- First reading question: $(json_value "$local_summary" '.metrics.first_reading_question')"
    echo "- First suggested tool: \`$(json_value "$local_summary" '.metrics.first_suggested_tool')\`"
    echo "- Impact risk: \`$(json_value "$local_summary" '.metrics.risk_level')\`"
    echo "- Impacted files: \`$(json_value "$local_summary" '.metrics.impacted_files')\`"
    echo "- MCP server: \`$(json_value "$mcp_summary" '.server')\`"
    echo "- MCP first-call contract: reading_order=\`$(json_value "$mcp_summary" '.execution_plan_reads_in_reading_plan_order')\`, suggested_tool_handoff=\`$(json_value "$mcp_summary" '.current_step_suggested_tool_matches_reading_plan')\`, continuation_after_selected_context=\`$(json_value "$mcp_summary" '.continuation_after_selected_context')\`"
    echo "- First-read gating: suggested_tool_after_selected_context=\`$(json_value "$mcp_summary" '(.execution_plan_reads_in_reading_plan_order == true and .current_step_suggested_tool_matches_reading_plan == true and .suggested_tool_executed == true)')\`, continuation_after_selected_context=\`$(json_value "$mcp_summary" '.continuation_after_selected_context')\`, impact_review_before_edits=\`$(json_value "$mcp_summary" '((.execution_plan_actions | index("review_impact_before_edits")) != null and .impact_status == "complete")')\`"
    echo "- MCP suggested tool executed: \`$(json_value "$mcp_summary" '.suggested_tool_executed')\`"
    echo "- MCP impact status: \`$(json_value "$mcp_summary" '.impact_status')\`"
    echo
    echo "## Artifacts"
    echo
    echo "- Local evidence Markdown: \`$OUTPUT_DIR/local-repo-evidence.md\`"
    echo "- Local evidence summary JSON: \`$OUTPUT_DIR/local-repo-evidence.json\`"
    echo "- Raw agent_route JSON: \`$OUTPUT_DIR/agent-route.json\`"
    echo "- MCP first-call summary JSON: \`$OUTPUT_DIR/mcp-first-call.json\`"
    echo "- Aggregate summary JSON: \`$OUTPUT_DIR/summary.json\`"
    echo "- Local evidence stdout: \`$OUTPUT_DIR/local-repo-evidence.out\`"
    echo "- Local evidence stderr: \`$OUTPUT_DIR/local-repo-evidence.err\`"
    echo "- MCP first-call stdout: \`$OUTPUT_DIR/mcp-first-call.out\`"
    echo "- MCP first-call stderr: \`$OUTPUT_DIR/mcp-first-call.err\`"
    echo "- Artifact write stderr: \`$OUTPUT_DIR/artifact-write.err\`"
    if [ -n "$issue_template_path" ]; then
      echo "- Issue template: \`$issue_template_path\`"
    fi
    echo
    echo "## Adoption Policy"
    echo
    echo "1. Start broad repository tasks with \`agent_route\`."
    echo "2. Read \`context_pack.files[]\` in \`reading_plan[]\` order."
    echo "3. Use the suggested tool only after selected context is read."
    echo "4. Review \`impact_analysis\` before editing."
  } >"$target"
}

write_summary_json() {
  local target="$1"
  local local_summary="$2"
  local mcp_summary="$3"
  local issue_template_path="${4:-}"

  jq -n \
    --arg repository "$REPO_ROOT" \
    --arg task "$TASK" \
    --arg output_dir "$OUTPUT_DIR" \
    --arg issue_template "$issue_template_path" \
    --slurpfile local "$local_summary" \
    --slurpfile mcp "$mcp_summary" \
    '{
      status: "pass",
      repository: $repository,
      task: $task,
      output_dir: $output_dir,
      local_evidence: $local[0],
      mcp_first_call: $mcp[0],
      first_read_gating: {
        suggested_tool_after_selected_context: (
          $mcp[0].execution_plan_reads_in_reading_plan_order == true
          and $mcp[0].current_step_suggested_tool_matches_reading_plan == true
          and $mcp[0].suggested_tool_executed == true
        ),
        continuation_after_selected_context: (
          $mcp[0].continuation_after_selected_context == true
        ),
        impact_review_before_edits: (
          (($mcp[0].execution_plan_actions | index("review_impact_before_edits")) != null)
          and $mcp[0].impact_status == "complete"
        )
      },
      artifacts: {
        markdown: ($output_dir + "/adoption-evidence.md"),
        local_markdown: ($output_dir + "/local-repo-evidence.md"),
        local_summary_json: ($output_dir + "/local-repo-evidence.json"),
        raw_agent_route_json: ($output_dir + "/agent-route.json"),
        mcp_first_call_json: ($output_dir + "/mcp-first-call.json"),
        local_stdout: ($output_dir + "/local-repo-evidence.out"),
        local_stderr: ($output_dir + "/local-repo-evidence.err"),
        mcp_stdout: ($output_dir + "/mcp-first-call.out"),
        mcp_stderr: ($output_dir + "/mcp-first-call.err"),
        artifact_stderr: ($output_dir + "/artifact-write.err")
      }
    }
    | if $issue_template != "" then
        .artifacts.issue_template = $issue_template
      else
        .
      end' >"$target"

  jq -e \
    '.status == "pass"
      and .local_evidence.status == "pass"
      and .mcp_first_call.status == "pass"
      and .local_evidence.route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .mcp_first_call.route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .mcp_first_call.execution_plan_reads_in_reading_plan_order == true
      and .mcp_first_call.current_step_suggested_tool_matches_reading_plan == true
      and .mcp_first_call.continuation_after_selected_context == true
      and .mcp_first_call.suggested_tool_executed == true
      and .first_read_gating.suggested_tool_after_selected_context == true
      and .first_read_gating.continuation_after_selected_context == true
      and .first_read_gating.impact_review_before_edits == true' \
    "$target" >/dev/null ||
    fail_with artifact_write "aggregate summary JSON does not match the adoption evidence contract"
}

print_snippet() {
  local summary_json="$1"

  cat <<EOF
# CodeInsight Adoption Evidence

- Status: \`$(json_value "$summary_json" '.status')\`
- Route: \`$(json_value "$summary_json" '.local_evidence.route_tools | join(" -> ")')\`
- Selected context: \`$(json_value "$summary_json" '.local_evidence.metrics.selected_lines')/$(json_value "$summary_json" '.local_evidence.metrics.total_lines')\` source lines, \`$(json_value "$summary_json" '.local_evidence.metrics.line_reduction')\` reduction
- Seed strategy: \`$(json_value "$summary_json" '.local_evidence.metrics.seed_strategy // "-"')\`
- Selected seeds: \`$(json_value "$summary_json" '.local_evidence.metrics.selected_seed_count // 0')\`
- First seed source: \`$(json_value "$summary_json" '.local_evidence.metrics.first_seed_source // "-"')\`
- Companion entrypoint: \`$(json_value "$summary_json" '(.local_evidence.metrics.companion_entrypoint // "") as $value | if $value == "" then "-" else $value end')\`
- First selected file: \`$(json_value "$summary_json" '.local_evidence.metrics.first_file')\`
- First reading question: $(json_value "$summary_json" '.local_evidence.metrics.first_reading_question')
- MCP server: \`$(json_value "$summary_json" '.mcp_first_call.server')\`
- MCP first-call contract: reading_order=\`$(json_value "$summary_json" '.mcp_first_call.execution_plan_reads_in_reading_plan_order')\`, suggested_tool_handoff=\`$(json_value "$summary_json" '.mcp_first_call.current_step_suggested_tool_matches_reading_plan')\`, continuation_after_selected_context=\`$(json_value "$summary_json" '.mcp_first_call.continuation_after_selected_context')\`
- First-read gating: suggested_tool_after_selected_context=\`$(json_value "$summary_json" '.first_read_gating.suggested_tool_after_selected_context')\`, continuation_after_selected_context=\`$(json_value "$summary_json" '.first_read_gating.continuation_after_selected_context')\`, impact_review_before_edits=\`$(json_value "$summary_json" '.first_read_gating.impact_review_before_edits')\`
- MCP suggested tool executed: \`$(json_value "$summary_json" '.mcp_first_call.suggested_tool_executed')\`
- MCP impact status: \`$(json_value "$summary_json" '.mcp_first_call.impact_status')\`
EOF
}

write_issue_template() {
  local target="$1"
  local summary_json="$2"

  {
    echo "# CodeInsight Adoption Evidence Issue"
    echo
    echo "## Summary"
    echo
    echo '```text'
    print_snippet "$summary_json"
    echo '```'
    echo
    echo "## Failure Category"
    echo
    echo "If the command failed, paste the exact category line here:"
    echo
    echo '```text'
    echo "adoption evidence failed [usage|prerequisite|local_cli_route|mcp_first_call|artifact_write]: ..."
    echo '```'
    echo
    echo "## Command"
    echo
    echo '```bash'
    echo "scripts/adoption-evidence.sh \"$REPO_ROOT\" --output-dir \"$OUTPUT_DIR\" --print-snippet --issue-template"
    echo '```'
    echo
    echo "## Artifacts"
    echo
    echo "- Adoption evidence: \`$(json_value "$summary_json" '.artifacts.markdown')\`"
    echo "- Aggregate summary JSON: \`$summary_json\`"
    echo "- Raw agent_route JSON: \`$(json_value "$summary_json" '.artifacts.raw_agent_route_json')\`"
    echo "- Local evidence stdout: \`$(json_value "$summary_json" '.artifacts.local_stdout')\`"
    echo "- Local evidence stderr: \`$(json_value "$summary_json" '.artifacts.local_stderr')\`"
    echo "- MCP first-call stdout: \`$(json_value "$summary_json" '.artifacts.mcp_stdout')\`"
    echo "- MCP first-call stderr: \`$(json_value "$summary_json" '.artifacts.mcp_stderr')\`"
    echo "- Artifact write stderr: \`$(json_value "$summary_json" '.artifacts.artifact_stderr')\`"
    echo
    echo "## Environment"
    echo
    echo "- OS:"
    echo "- Shell:"
    echo "- CodeInsight version:"
    echo "- MCP client:"
    echo "- Repository language/framework:"
    echo "- Repository size:"
    echo
    echo "## Notes"
    echo
    echo "Describe what you expected the first-read route to do and what looked wrong."
  } >"$target"
}

main() {
  parse_args "$@"
  require_command jq

  if [ -z "$REPO_ROOT" ]; then
    fail_with usage "missing repository root"
  fi
  if [ ! -d "$REPO_ROOT" ]; then
    fail_with usage "repository root does not exist: $REPO_ROOT"
  fi
  case "$TOKEN_BUDGET" in
    ''|*[!0-9]*)
      fail_with usage "--token-budget must be a positive integer"
      ;;
  esac
  if [ "$TOKEN_BUDGET" -le 0 ]; then
    fail_with usage "--token-budget must be greater than zero"
  fi
  if [ ! -x "$LOCAL_REPO_EVIDENCE_SCRIPT" ]; then
    fail_with prerequisite "local repo evidence script is not executable: $LOCAL_REPO_EVIDENCE_SCRIPT"
  fi
  if [ ! -x "$MCP_FIRST_CALL_SMOKE_SCRIPT" ]; then
    fail_with prerequisite "MCP first-call smoke script is not executable: $MCP_FIRST_CALL_SMOKE_SCRIPT"
  fi

  REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
  OUTPUT_DIR="${OUTPUT_DIR:-/tmp/codeinsight-adoption-evidence}"
  mkdir -p "$OUTPUT_DIR" ||
    fail_with artifact_write "could not create output directory: $OUTPUT_DIR"

  local local_args mcp_env local_log mcp_log artifact_log
  local_log="$OUTPUT_DIR/local-repo-evidence.err"
  mcp_log="$OUTPUT_DIR/mcp-first-call.err"
  artifact_log="$OUTPUT_DIR/artifact-write.err"
  local_args=(
    "$REPO_ROOT"
    "--task"
    "$TASK"
    "--token-budget"
    "$TOKEN_BUDGET"
    "--output"
    "$OUTPUT_DIR/local-repo-evidence.md"
    "--json"
    "$OUTPUT_DIR/agent-route.json"
    "--summary-json"
    "$OUTPUT_DIR/local-repo-evidence.json"
  )
  if [ -n "$CODEINSIGHT_BIN" ]; then
    local_args+=("--bin" "$CODEINSIGHT_BIN")
  fi
  if [ "$FORCE_INDEX" != "1" ]; then
    local_args+=("--no-force-index")
  fi

  if ! "$LOCAL_REPO_EVIDENCE_SCRIPT" "${local_args[@]}" >"$OUTPUT_DIR/local-repo-evidence.out" 2>"$local_log"; then
    fail_step local_cli_route "$local_log" "local first-read evidence generation failed"
  fi

  mcp_env=(
    "CODEINSIGHT_FIRST_CALL_ROOT=$REPO_ROOT"
    "CODEINSIGHT_FIRST_CALL_TASK=$TASK"
    "CODEINSIGHT_FIRST_CALL_TOKEN_BUDGET=$TOKEN_BUDGET"
  )
  if [ -n "$CODEINSIGHT_BIN" ]; then
    mcp_env+=("CODEINSIGHT_BIN=$CODEINSIGHT_BIN")
  fi

  env "${mcp_env[@]}" \
    "$MCP_FIRST_CALL_SMOKE_SCRIPT" \
    --summary-json "$OUTPUT_DIR/mcp-first-call.json" >"$OUTPUT_DIR/mcp-first-call.out" 2>"$mcp_log" ||
    fail_step mcp_first_call "$mcp_log" "MCP first-call verification failed"

  local issue_template_path
  issue_template_path=""
  if [ "$ISSUE_TEMPLATE" = "1" ]; then
    issue_template_path="$OUTPUT_DIR/issue-template.md"
  fi

  if ! {
    write_summary_json "$OUTPUT_DIR/summary.json" \
      "$OUTPUT_DIR/local-repo-evidence.json" \
      "$OUTPUT_DIR/mcp-first-call.json" \
      "$issue_template_path"
    write_markdown_summary "$OUTPUT_DIR/adoption-evidence.md" \
      "$OUTPUT_DIR/local-repo-evidence.json" \
      "$OUTPUT_DIR/mcp-first-call.json" \
      "$issue_template_path"
    if [ -n "$issue_template_path" ]; then
      write_issue_template "$issue_template_path" "$OUTPUT_DIR/summary.json"
    fi
  } 2>"$artifact_log"; then
    fail_step artifact_write "$artifact_log" "aggregate adoption artifacts could not be written"
  fi

  echo "adoption evidence written to $OUTPUT_DIR"
  echo "markdown: $OUTPUT_DIR/adoption-evidence.md"
  echo "summary_json: $OUTPUT_DIR/summary.json"
  if [ -n "$issue_template_path" ]; then
    echo "issue_template: $issue_template_path"
  fi
  if [ "$PRINT_SNIPPET" = "1" ]; then
    echo
    print_snippet "$OUTPUT_DIR/summary.json"
  fi
}

main "$@"
