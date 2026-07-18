#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${CODEINSIGHT_SELF_ADOPTION_ROOT:-$ROOT_DIR}"
TASK="${CODEINSIGHT_SELF_ADOPTION_TASK:-understand the main application entrypoint}"
TOKEN_BUDGET="${CODEINSIGHT_SELF_ADOPTION_TOKEN_BUDGET:-6000}"
OUTPUT_FILE="${CODEINSIGHT_SELF_ADOPTION_OUTPUT:-$ROOT_DIR/docs/adoption-report-codeinsight.md}"
REPORT_OUTPUT_DIR="${CODEINSIGHT_SELF_ADOPTION_REPORT_OUTPUT_DIR:-/tmp/codeinsight-self-adoption-report}"
ARCHIVE_PATH="${CODEINSIGHT_SELF_ADOPTION_ARCHIVE:-/tmp/codeinsight-self-adoption-report.tar.gz}"
REPORT_SCRIPT="${CODEINSIGHT_SELF_ADOPTION_REPORT_SCRIPT:-$ROOT_DIR/scripts/adoption-report.sh}"
REFRESHED_ON="${CODEINSIGHT_SELF_ADOPTION_REFRESHED_ON:-}"
CHECK="false"
NO_FORCE_INDEX="false"

usage() {
  cat <<'EOF'
usage: scripts/update-self-adoption-report.sh [options]

Refreshes docs/adoption-report-codeinsight.md from a live adoption-report run.

Options:
  --root PATH           Repository root. Default: this checkout.
  --task TEXT           Task passed to adoption-report.
  --token-budget N      Token budget passed to adoption-report. Default: 6000.
  --output PATH         Output Markdown path. Default: docs/adoption-report-codeinsight.md.
  --output-dir PATH     adoption-report output directory. Default: /tmp/codeinsight-self-adoption-report.
  --archive PATH        adoption-report tar.gz path. Default: /tmp/codeinsight-self-adoption-report.tar.gz.
  --report-script PATH  adoption-report-compatible script to execute.
  --refreshed-on DATE   Snapshot date written to the report. Default: today.
  --check               Verify the checked-in report is already up to date.
  --no-force-index      Reuse the existing index when available.
  -h, --help            Show this help text.

Environment:
  CODEINSIGHT_SELF_ADOPTION_ROOT
  CODEINSIGHT_SELF_ADOPTION_TASK
  CODEINSIGHT_SELF_ADOPTION_TOKEN_BUDGET
  CODEINSIGHT_SELF_ADOPTION_OUTPUT
  CODEINSIGHT_SELF_ADOPTION_REPORT_OUTPUT_DIR
  CODEINSIGHT_SELF_ADOPTION_ARCHIVE
  CODEINSIGHT_SELF_ADOPTION_REPORT_SCRIPT
  CODEINSIGHT_SELF_ADOPTION_REFRESHED_ON
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "update self adoption report failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --root)
        [ "$#" -ge 2 ] || fail "--root requires a path"
        REPO_ROOT="$2"
        shift 2
        ;;
      --task)
        [ "$#" -ge 2 ] || fail "--task requires text"
        TASK="$2"
        shift 2
        ;;
      --token-budget)
        [ "$#" -ge 2 ] || fail "--token-budget requires a number"
        TOKEN_BUDGET="$2"
        shift 2
        ;;
      --output)
        [ "$#" -ge 2 ] || fail "--output requires a path"
        OUTPUT_FILE="$2"
        shift 2
        ;;
      --output-dir)
        [ "$#" -ge 2 ] || fail "--output-dir requires a path"
        REPORT_OUTPUT_DIR="$2"
        shift 2
        ;;
      --archive)
        [ "$#" -ge 2 ] || fail "--archive requires a path"
        ARCHIVE_PATH="$2"
        shift 2
        ;;
      --report-script)
        [ "$#" -ge 2 ] || fail "--report-script requires a path"
        REPORT_SCRIPT="$2"
        shift 2
        ;;
      --refreshed-on)
        [ "$#" -ge 2 ] || fail "--refreshed-on requires a date"
        REFRESHED_ON="$2"
        shift 2
        ;;
      --check)
        CHECK="true"
        shift
        ;;
      --no-force-index)
        NO_FORCE_INDEX="true"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      -*)
        fail "unknown argument: $1"
        ;;
      *)
        fail "unexpected positional argument: $1"
        ;;
    esac
  done
}

resolve_refresh_date() {
  if [ -n "$REFRESHED_ON" ]; then
    return
  fi

  if [ "$CHECK" = "true" ] && [ -f "$OUTPUT_FILE" ]; then
    REFRESHED_ON="$(
      sed -n 's/^- Refreshed on: `\([^`]*\)`$/\1/p' "$OUTPUT_FILE" | head -n 1
    )"
  fi

  if [ -z "$REFRESHED_ON" ]; then
    REFRESHED_ON="$(date +%F)"
  fi
}

run_report() {
  local -a args
  args=(
    "$REPO_ROOT"
    "--task"
    "$TASK"
    "--token-budget"
    "$TOKEN_BUDGET"
    "--output-dir"
    "$REPORT_OUTPUT_DIR"
    "--archive"
    "$ARCHIVE_PATH"
    "--print-snippet"
  )
  if [ "$NO_FORCE_INDEX" = "true" ]; then
    args+=("--no-force-index")
  fi

  rm -rf "$REPORT_OUTPUT_DIR" "$ARCHIVE_PATH"
  "$REPORT_SCRIPT" "${args[@]}" >"$REPORT_OUTPUT_DIR.print-snippet.out"
}

validate_report() {
  local summary_json="$REPORT_OUTPUT_DIR/summary.json"
  local manifest_json="$REPORT_OUTPUT_DIR/manifest.json"

  [ -f "$summary_json" ] || fail "summary.json is missing: $summary_json"
  [ -f "$manifest_json" ] || fail "manifest.json is missing: $manifest_json"
  [ -f "$ARCHIVE_PATH" ] || fail "archive is missing: $ARCHIVE_PATH"

  jq -e \
    '.status == "pass"
      and .local_evidence.status == "pass"
      and .mcp_first_call.status == "pass"
      and .local_evidence.route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .mcp_first_call.execution_plan_reads_in_reading_plan_order == true
      and .mcp_first_call.current_step_suggested_tool_matches_reading_plan == true
      and .mcp_first_call.continuation_after_selected_context == true
      and .mcp_first_call.suggested_tool_executed == true
      and .first_read_gating.suggested_tool_after_selected_context == true
      and .first_read_gating.continuation_after_selected_context == true
      and .first_read_gating.impact_review_before_edits == true' \
    "$summary_json" >/dev/null ||
    fail "summary.json does not match the self adoption report contract"

  jq -e \
    '.status == "pass"
      and (.files | index("adoption-evidence.md") != null)
      and (.files | index("summary.json") != null)
      and (.files | index("mcp-first-call.json") != null)
      and (.files | index("manifest.json") != null)' \
    "$manifest_json" >/dev/null ||
    fail "manifest.json does not match the report bundle contract"
}

write_generated_doc() {
  local target="$1"
  local summary_json="$REPORT_OUTPUT_DIR/summary.json"
  local manifest_json="$REPORT_OUTPUT_DIR/manifest.json"

  ruby -rjson - "$summary_json" "$manifest_json" "$target" "$REFRESHED_ON" "$ARCHIVE_PATH" "$REPORT_OUTPUT_DIR" "$TASK" "$TOKEN_BUDGET" <<'RUBY'
summary = JSON.parse(File.read(ARGV.fetch(0)))
manifest = JSON.parse(File.read(ARGV.fetch(1)))
target = ARGV.fetch(2)
refreshed_on = ARGV.fetch(3)
archive_path = ARGV.fetch(4)
report_output_dir = ARGV.fetch(5)
task = ARGV.fetch(6)
token_budget = ARGV.fetch(7)

local = summary.fetch("local_evidence")
mcp = summary.fetch("mcp_first_call")
metrics = local.fetch("metrics")
route = local.fetch("route_tools").join(" -> ")
companion = metrics.fetch("companion_entrypoint", "")
companion = "-" if companion.empty?
suggested_args = mcp.fetch("suggested_tool").fetch("arguments")
suggested_path = suggested_args.fetch("path")
files = manifest.fetch("files")

def line(text = "")
  "#{text}\n"
end

content = +""
content << line("# CodeInsight Self Adoption Report")
content << line
content << line("This is a reproducible adoption report snapshot for CodeInsight itself. It uses")
content << line("the complete `scripts/adoption-report.sh` path, not only the shorter")
content << line("blind-read comparison flow, so it verifies the uploadable `tar.gz` report")
content << line("shape, issue template, aggregate summaries, raw JSON, and diagnostic logs.")
content << line
content << line("This is adoption evidence, not a controlled performance benchmark. The goal is")
content << line("to prove that the report bundle preserves the same first-read route and MCP")
content << line("first-call contract that a client or issue triage flow needs.")
content << line
content << line("## Snapshot")
content << line
content << line("- Repository: `CodeInsight-mcp`")
content << line("- Root: `#{summary.fetch("repository")}`")
content << line("- Task: `#{task}`")
content << line("- Token budget: `#{token_budget}`")
content << line("- Route: `#{route}`")
content << line("- Generated with: `scripts/adoption-report.sh`")
content << line("- Refreshed on: `#{refreshed_on}`")
content << line("- Source summary: `#{report_output_dir}/summary.json`")
content << line("- Source manifest: `#{report_output_dir}/manifest.json`")
content << line
content << line("## Result")
content << line
content << line("| Metric | Value |")
content << line("| --- | ---: |")
content << line("| Indexed files | `#{metrics.fetch("indexed_files")}` |")
content << line("| Symbols | `#{metrics.fetch("symbols")}` |")
content << line("| Index errors | `#{metrics.fetch("index_errors")}` |")
content << line("| Entrypoints | `#{metrics.fetch("entrypoints")}` |")
content << line("| Blind first-read baseline | `#{metrics.fetch("total_lines")}` source lines |")
content << line("| CodeInsight routed first-read | `#{metrics.fetch("selected_lines")}` source lines |")
content << line("| First-read reduction | `#{metrics.fetch("line_reduction")}` |")
content << line("| Selected files | `#{metrics.fetch("selected_files")}` |")
content << line("| Selected ranges | `#{metrics.fetch("selected_ranges")}` |")
content << line("| Estimated tokens | `#{metrics.fetch("estimated_tokens")}` |")
content << line("| Reading plan steps | `#{metrics.fetch("reading_plan_steps")}` |")
content << line("| Impacted files | `#{metrics.fetch("impacted_files")}` |")
content << line
content << line("## First-Read Route")
content << line
content << line("| Field | Value |")
content << line("| --- | --- |")
content << line("| Seed strategy | `#{metrics.fetch("seed_strategy")}` |")
content << line("| First seed source | `#{metrics.fetch("first_seed_source")}` |")
content << line("| First seed value | `#{metrics.fetch("first_seed_value")}` |")
content << line("| Companion entrypoint | `#{companion}` |")
content << line("| First selected file | `#{metrics.fetch("first_file")}` |")
content << line("| First next action | `#{metrics.fetch("first_next_action")}` |")
content << line("| First suggested tool | `#{metrics.fetch("first_suggested_tool")}` |")
content << line("| Impact risk | `#{metrics.fetch("risk_level")}` |")
content << line
content << line("First reading question:")
content << line
content << line("```text")
content << line(metrics.fetch("first_reading_question"))
content << line("```")
content << line
content << line("## MCP First-Call Contract")
content << line
content << line("| Contract | Value |")
content << line("| --- | --- |")
content << line("| Reading order starts with selected context | `#{mcp.fetch("execution_plan_reads_in_reading_plan_order")}` |")
content << line("| Current-step suggested tool matches the reading plan | `#{mcp.fetch("current_step_suggested_tool_matches_reading_plan")}` |")
content << line("| Continuation is checked after selected context | `#{mcp.fetch("continuation_after_selected_context")}` |")
content << line("| Suggested tool executed through MCP `tools/call` | `#{mcp.fetch("suggested_tool_executed")}` |")
content << line("| MCP impact status | `#{mcp.fetch("impact_status")}` |")
content << line
content << line("The first MCP selected file and first reading-plan file were both")
content << line("`#{mcp.fetch("first_context_file")}`, and the executable suggested tool was")
content << line("`#{mcp.fetch("suggested_tool").fetch("tool")}` with an absolute `#{suggested_path}` path.")
content << line
content << line("## Report Bundle")
content << line
content << line("The generated archive was:")
content << line
content << line("```text")
content << line(archive_path)
content << line("```")
content << line
content << line("The archive manifest contained:")
content << line
files.each do |file|
  content << line("- `#{file}`")
end
content << line
content << line("The generated manifest reported `status: #{manifest.fetch("status")}` and listed the same #{files.length} files")
content << line("that are packaged in the archive.")
content << line
content << line("## Generated Snippet")
content << line
content << line("The `--print-snippet` output from the refreshed report was:")
content << line
content << line("```text")
content << line("# CodeInsight Adoption Evidence")
content << line
content << line("- Status: `#{summary.fetch("status")}`")
content << line("- Route: `#{route}`")
content << line("- Selected context: `#{metrics.fetch("selected_lines")}/#{metrics.fetch("total_lines")}` source lines, `#{metrics.fetch("line_reduction")}` reduction")
content << line("- Seed strategy: `#{metrics.fetch("seed_strategy")}`")
content << line("- Selected seeds: `#{metrics.fetch("selected_seed_count")}`")
content << line("- First seed source: `#{metrics.fetch("first_seed_source")}`")
content << line("- Companion entrypoint: `#{companion}`")
content << line("- First selected file: `#{metrics.fetch("first_file")}`")
content << line("- First reading question: #{metrics.fetch("first_reading_question")}")
content << line("- MCP server: `#{mcp.fetch("server")}`")
content << line("- MCP first-call contract: reading_order=`#{mcp.fetch("execution_plan_reads_in_reading_plan_order")}`, suggested_tool_handoff=`#{mcp.fetch("current_step_suggested_tool_matches_reading_plan")}`, continuation_after_selected_context=`#{mcp.fetch("continuation_after_selected_context")}`")
content << line("- First-read gating: suggested_tool_after_selected_context=`#{summary.fetch("first_read_gating").fetch("suggested_tool_after_selected_context")}`, continuation_after_selected_context=`#{summary.fetch("first_read_gating").fetch("continuation_after_selected_context")}`, impact_review_before_edits=`#{summary.fetch("first_read_gating").fetch("impact_review_before_edits")}`")
content << line("- MCP suggested tool executed: `#{mcp.fetch("suggested_tool_executed")}`")
content << line("- MCP impact status: `#{mcp.fetch("impact_status")}`")
content << line("```")
content << line
content << line("## Reproduce")
content << line
content << line("Refresh this checked-in snapshot:")
content << line
content << line("```bash")
content << line("scripts/update-self-adoption-report.sh")
content << line("```")
content << line
content << line("Verify the checked-in snapshot is current:")
content << line
content << line("```bash")
content << line("scripts/update-self-adoption-report.sh --check")
content << line("```")
content << line
content << line("Run from a CodeInsight checkout:")
content << line
content << line("```bash")
content << line("rm -rf /tmp/codeinsight-self-adoption-report /tmp/codeinsight-self-adoption-report.tar.gz")
content << line("scripts/adoption-report.sh . \\")
content << line("  --task \"understand the main application entrypoint\" \\")
content << line("  --token-budget 6000 \\")
content << line("  --output-dir /tmp/codeinsight-self-adoption-report \\")
content << line("  --archive /tmp/codeinsight-self-adoption-report.tar.gz \\")
content << line("  --print-snippet")
content << line("```")
content << line
content << line("Expected summary lines:")
content << line
content << line("```text")
content << line("- Selected context: `#{metrics.fetch("selected_lines")}/#{metrics.fetch("total_lines")}` source lines, `#{metrics.fetch("line_reduction")}` reduction")
content << line("- MCP first-call contract: reading_order=`#{mcp.fetch("execution_plan_reads_in_reading_plan_order")}`, suggested_tool_handoff=`#{mcp.fetch("current_step_suggested_tool_matches_reading_plan")}`, continuation_after_selected_context=`#{mcp.fetch("continuation_after_selected_context")}`")
content << line("- First-read gating: suggested_tool_after_selected_context=`#{summary.fetch("first_read_gating").fetch("suggested_tool_after_selected_context")}`, continuation_after_selected_context=`#{summary.fetch("first_read_gating").fetch("continuation_after_selected_context")}`, impact_review_before_edits=`#{summary.fetch("first_read_gating").fetch("impact_review_before_edits")}`")
content << line("```")

File.write(target, content)
RUBY
}

main() {
  parse_args "$@"
  require_command jq
  require_command git
  require_command ruby

  if [ ! -d "$REPO_ROOT" ]; then
    fail "repository root does not exist: $REPO_ROOT"
  fi
  if [ ! -x "$REPORT_SCRIPT" ]; then
    fail "report script is not executable: $REPORT_SCRIPT"
  fi
  case "$TOKEN_BUDGET" in
    ''|*[!0-9]*)
      fail "--token-budget must be a positive integer"
      ;;
  esac
  if [ "$TOKEN_BUDGET" -le 0 ]; then
    fail "--token-budget must be greater than zero"
  fi

  REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
  resolve_refresh_date
  mkdir -p "$(dirname "$REPORT_OUTPUT_DIR")"
  run_report
  validate_report

  local generated
  generated="$(mktemp)"
  write_generated_doc "$generated"

  if [ "$CHECK" = "true" ]; then
    if ! cmp -s "$generated" "$OUTPUT_FILE"; then
      echo "self adoption report is out of date: $OUTPUT_FILE" >&2
      git diff --no-index -- "$OUTPUT_FILE" "$generated" >&2 || true
      rm -f "$generated"
      exit 1
    fi
    rm -f "$generated"
    echo "self adoption report is up to date"
    return
  fi

  mkdir -p "$(dirname "$OUTPUT_FILE")"
  mv "$generated" "$OUTPUT_FILE"
  echo "updated self adoption report: $OUTPUT_FILE"
  echo "summary: $REPORT_OUTPUT_DIR/summary.json"
  echo "manifest: $REPORT_OUTPUT_DIR/manifest.json"
  echo "archive: $ARCHIVE_PATH"
}

main "$@"
