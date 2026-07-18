#!/usr/bin/env bash
set -euo pipefail

SCRIPT_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ROOT_DIR="${CODEINSIGHT_ROOT_DIR:-$SCRIPT_ROOT}"
BENCHMARK_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/benchmark-artifact-smoke.sh}"
CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/context-pack-quality-artifact-smoke.sh}"
AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/agent-route-artifact-smoke.sh}"
MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT="${CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT:-$ROOT_DIR/scripts/mcp-first-call-artifact-smoke.sh}"
RELEASE_METADATA_SUMMARY_SCRIPT="${CODEINSIGHT_RELEASE_METADATA_SUMMARY_SCRIPT:-$SCRIPT_ROOT/scripts/release-metadata-summary.sh}"
ARTIFACT_NAME="codeinsight-benchmark-subset"
CONTEXT_PACK_QUALITY_ARTIFACT_NAME="codeinsight-context-pack-quality"
AGENT_ROUTE_ARTIFACT_NAME="codeinsight-agent-route-smoke"
MCP_FIRST_CALL_ARTIFACT_NAME="codeinsight-mcp-first-call"
ADOPTION_REPORT_NAME="CodeInsight self adoption report"
ADOPTION_REPORT_DOC="docs/adoption-report-codeinsight.md"
ADOPTION_REPORT_COMMAND='scripts/adoption-report.sh . --task "understand the main application entrypoint" --token-budget 6000 --output-dir /tmp/codeinsight-self-adoption-report --archive /tmp/codeinsight-self-adoption-report.tar.gz --print-snippet'
ADOPTION_REPORT_ARCHIVE="/tmp/codeinsight-self-adoption-report.tar.gz"
ADOPTION_REPORT_SELECTED_LINES=""
ADOPTION_REPORT_TOTAL_LINES=""
ADOPTION_REPORT_LINE_REDUCTION=""
ADOPTION_REPORT_READING_ORDER=""
ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF=""
ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT=""
ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED=""
REPO_ARG=()
REPO=""
BRANCH="main"
HEAD_SHA=""
RUN_ID=""
TAG_NAME=""
JSON_OUTPUT_FILE=""

usage() {
  local status="${1:-2}"
  local stream="/dev/stderr"
  if [ "$status" -eq 0 ]; then
    stream="/dev/stdout"
  fi

  cat >"$stream" <<'EOF'
usage: scripts/release-evidence-summary.sh [options] <tag> [branch]

Build a copyable pre-release evidence summary for the target tag. The script
verifies release metadata, resolves the successful CI run for the tag target
SHA, validates the benchmark subset, context-pack quality, and agent-route
artifacts, and prints a Markdown block for release notes or handoff checklists.

Options:
  --repo OWNER/REPO       Pass an explicit GitHub repository to gh.
  --head-sha SHA          Check this commit instead of the current HEAD.
  --run-id ID             Use this CI run instead of resolving by head SHA.
                          When both are set, the run must match the SHA.
  --artifact-name NAME    Benchmark artifact name. Default: codeinsight-benchmark-subset.
  --quality-artifact-name NAME
                          Context-pack quality artifact name. Default: codeinsight-context-pack-quality.
  --agent-route-artifact-name NAME
                          Agent-route artifact name. Default: codeinsight-agent-route-smoke.
  --mcp-first-call-artifact-name NAME
                          MCP first-call artifact name. Default: codeinsight-mcp-first-call.
  --json-output PATH      Write a machine-readable evidence summary JSON.
  -h, --help              Show this help.

Environment:
  CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT=scripts/benchmark-artifact-smoke.sh
  CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT=scripts/context-pack-quality-artifact-smoke.sh
  CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT=scripts/agent-route-artifact-smoke.sh
  CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT=scripts/mcp-first-call-artifact-smoke.sh
  CODEINSIGHT_RELEASE_METADATA_SUMMARY_SCRIPT=scripts/release-metadata-summary.sh
  CODEINSIGHT_ROOT_DIR=/path/to/repo
EOF
  exit "$status"
}

fail() {
  echo "release evidence summary failed: $*" >&2
  exit 1
}

normalize_tag() {
  local tag="$1"
  case "$tag" in
    v*) printf "%s" "$tag" ;;
    *) printf "v%s" "$tag" ;;
  esac
}

resolve_repo() {
  if [ -n "$REPO" ]; then
    printf "%s" "$REPO"
    return 0
  fi

  gh repo view --json nameWithOwner --jq '.nameWithOwner'
}

resolve_run_by_head_sha() {
  local branch="$1"
  local head_sha="$2"
  local run_id

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    run_id="$(
      gh run list \
        "${REPO_ARG[@]}" \
        --workflow CI \
        --branch "$branch" \
        --status success \
        --limit 20 \
        --json databaseId,headSha \
        --jq "map(select(.headSha == \"$head_sha\"))[0].databaseId // \"\""
    )"
  else
    run_id="$(
      gh run list \
        --workflow CI \
        --branch "$branch" \
        --status success \
        --limit 20 \
        --json databaseId,headSha \
        --jq "map(select(.headSha == \"$head_sha\"))[0].databaseId // \"\""
    )"
  fi

  if [ -z "$run_id" ]; then
    fail "no successful CI run found for branch: $branch and head SHA: $head_sha"
  fi
  printf "%s" "$run_id"
}

validate_run() {
  local run_id="$1"
  local expected_head_sha="$2"
  local run_json

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    run_json="$(gh run view "$run_id" "${REPO_ARG[@]}" --json conclusion,databaseId,headSha,status,url)"
  else
    run_json="$(gh run view "$run_id" --json conclusion,databaseId,headSha,status,url)"
  fi

  RUN_JSON="$run_json" ruby -rjson - "$expected_head_sha" <<'RUBY'
expected_head_sha = ARGV.fetch(0)
run = JSON.parse(ENV.fetch("RUN_JSON"))

def fail!(message)
  warn("release evidence summary failed: #{message}")
  exit(1)
end

fail!("CI run is not completed: #{run["status"]}") unless run["status"] == "completed"
fail!("CI run did not succeed: #{run["conclusion"]}") unless run["conclusion"] == "success"
fail!("CI run head SHA #{run["headSha"]} does not match #{expected_head_sha}") unless run["headSha"] == expected_head_sha

puts "ci_run: #{run["databaseId"]}"
puts "ci_url: #{run["url"]}"
RUBY
}

resolve_artifact_url() {
  local repo="$1"
  local run_id="$2"
  local artifact_name="$3"
  local artifact_id

  artifact_id="$(
    gh api "repos/$repo/actions/runs/$run_id/artifacts" \
      --jq ".artifacts[] | select(.name == \"$artifact_name\") | .id" \
      | head -n 1
  )"

  if [ -z "$artifact_id" ]; then
    fail "artifact not found on CI run $run_id: $artifact_name"
  fi

  printf "https://github.com/%s/actions/runs/%s/artifacts/%s" "$repo" "$run_id" "$artifact_id"
}

validate_benchmark_artifact() {
  local run_id="$1"
  local output

  if [ ! -x "$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "benchmark artifact smoke script is not executable: $BENCHMARK_ARTIFACT_SMOKE_SCRIPT"
  fi

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    output="$("$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" --artifact-name "$ARTIFACT_NAME" "$run_id")"
  else
    output="$("$BENCHMARK_ARTIFACT_SMOKE_SCRIPT" --artifact-name "$ARTIFACT_NAME" "$run_id")"
  fi

  printf "%s\n" "$output" | awk -F': ' '/^report: / { print "report: " $2 } /^summary: / { print "summary: " $2 }'
}

benchmark_metric() {
  local summary_file="$1"
  local query="$2"
  local description="$3"
  local value

  value="$(jq -r "$query" "$summary_file")"
  if [ -z "$value" ] || [ "$value" = "null" ]; then
    fail "benchmark summary is missing $description: $summary_file"
  fi
  printf "%s" "$value"
}

agent_route_metric() {
  local summary_file="$1"
  local query="$2"
  local description="$3"
  local value

  value="$(jq -r "$query" "$summary_file")"
  if [ -z "$value" ] || [ "$value" = "null" ]; then
    fail "agent-route summary is missing $description: $summary_file"
  fi
  printf "%s" "$value"
}

validate_context_pack_quality_artifact() {
  local run_id="$1"
  local output

  if [ ! -x "$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "context-pack quality artifact smoke script is not executable: $CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT"
  fi

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    output="$("$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" --artifact-name "$CONTEXT_PACK_QUALITY_ARTIFACT_NAME" "$run_id")"
  else
    output="$("$CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT" --artifact-name "$CONTEXT_PACK_QUALITY_ARTIFACT_NAME" "$run_id")"
  fi

  printf "%s\n" "$output" | awk -F': ' '/^summary: / { print $2; exit }'
}

validate_agent_route_artifact() {
  local run_id="$1"
  local output

  if [ ! -x "$AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "agent-route artifact smoke script is not executable: $AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT"
  fi

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    output="$("$AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" --artifact-name "$AGENT_ROUTE_ARTIFACT_NAME" "$run_id")"
  else
    output="$("$AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT" --artifact-name "$AGENT_ROUTE_ARTIFACT_NAME" "$run_id")"
  fi

  printf "%s\n" "$output" | awk -F': ' '/^summary: / { print $2; exit }'
}

validate_mcp_first_call_artifact() {
  local run_id="$1"
  local output

  if [ ! -x "$MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT" ]; then
    fail "MCP first-call artifact smoke script is not executable: $MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT"
  fi

  if [ "${#REPO_ARG[@]}" -gt 0 ]; then
    output="$("$MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT" "${REPO_ARG[@]}" --artifact-name "$MCP_FIRST_CALL_ARTIFACT_NAME" "$run_id")"
  else
    output="$("$MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT" --artifact-name "$MCP_FIRST_CALL_ARTIFACT_NAME" "$run_id")"
  fi

  printf "%s\n" "$output" | awk -F': ' '/^summary: / { print $2; exit }'
}

load_adoption_report_doc() {
  local report_doc="$ROOT_DIR/$ADOPTION_REPORT_DOC"
  local selected_lines
  local total_lines
  local line_reduction
  local reading_order
  local suggested_tool_handoff
  local continuation_after_selected_context
  local suggested_tool_executed

  if [ ! -f "$report_doc" ]; then
    fail "adoption report document is missing: $ADOPTION_REPORT_DOC"
  fi

  selected_lines="$(adoption_report_table_value "$report_doc" "CodeInsight routed first-read" | sed 's/ source lines$//')"
  total_lines="$(adoption_report_table_value "$report_doc" "Blind first-read baseline" | sed 's/ source lines$//')"
  line_reduction="$(adoption_report_table_value "$report_doc" "First-read reduction")"
  reading_order="$(adoption_report_table_value "$report_doc" "Reading order starts with selected context")"
  suggested_tool_handoff="$(adoption_report_table_value "$report_doc" "Current-step suggested tool matches the reading plan")"
  continuation_after_selected_context="$(adoption_report_table_value "$report_doc" "Continuation is checked after selected context")"
  suggested_tool_executed="$(adoption_report_table_value "$report_doc" 'Suggested tool executed through MCP `tools/call`')"

  case "$selected_lines" in
    ''|*[!0-9]*)
      fail "adoption report document has non-numeric routed first-read lines"
      ;;
  esac
  case "$total_lines" in
    ''|*[!0-9]*)
      fail "adoption report document has non-numeric blind first-read baseline"
      ;;
  esac

  ADOPTION_REPORT_SELECTED_LINES="$selected_lines"
  ADOPTION_REPORT_TOTAL_LINES="$total_lines"
  ADOPTION_REPORT_LINE_REDUCTION="$line_reduction"
  ADOPTION_REPORT_READING_ORDER="$reading_order"
  ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF="$suggested_tool_handoff"
  ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT="$continuation_after_selected_context"
  ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED="$suggested_tool_executed"

  [ -n "$ADOPTION_REPORT_LINE_REDUCTION" ] ||
    fail "adoption report document is missing line reduction"
  [ "$ADOPTION_REPORT_READING_ORDER" = "true" ] ||
    fail "adoption report MCP reading-order contract did not pass"
  [ "$ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF" = "true" ] ||
    fail "adoption report MCP suggested-tool handoff contract did not pass"
  [ "$ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT" = "true" ] ||
    fail "adoption report MCP continuation contract did not pass"
  [ "$ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED" = "true" ] ||
    fail "adoption report MCP suggested-tool execution contract did not pass"
}

adoption_report_table_value() {
  local report_doc="$1"
  local label="$2"
  local value

  value="$(
    awk -F'|' -v label="$label" '
      {
        row_label = $2
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", row_label)
      }
      row_label == label {
        value = $3
        gsub(/^[[:space:]]+|[[:space:]]+$/, "", value)
        gsub(/`/, "", value)
        print value
        found = 1
        exit
      }
      END {
        if (!found) {
          exit 1
        }
      }
    ' "$report_doc"
  )" ||
    fail "adoption report document is missing $label"

  printf "%s" "$value"
}

write_json_summary() {
  local output_file="$1"
  local metadata_summary="$2"
  local repo_name="$3"
  local ci_url="$4"
  local benchmark_artifact_url="$5"
  local benchmark_report_file="$6"
  local benchmark_summary_file="$7"
  local benchmark_context_pack_first="${8}"
  local benchmark_routing_total="${9}"
  local benchmark_line_reduction="${10}"
  local benchmark_guardrail_failures="${11}"
  local benchmark_truncated_packs="${12}"
  local context_pack_quality_artifact_url="${13}"
  local context_pack_quality_summary_file="${14}"
  local agent_route_artifact_url="${15}"
  local agent_route_summary_file="${16}"
  local agent_route_first_selection_rank="${17}"
  local agent_route_first_selection_reason="${18}"
  local agent_route_continuation_status="${19}"
  local agent_route_continuation_next_action="${20}"
  local mcp_first_call_artifact_url="${21}"
  local mcp_first_call_summary_file="${22}"

  mkdir -p "$(dirname "$output_file")"
  TAG_NAME="$TAG_NAME" \
    BRANCH="$BRANCH" \
    HEAD_SHA="$HEAD_SHA" \
    REPO_NAME="$repo_name" \
    RUN_ID="$RUN_ID" \
    CI_URL="$ci_url" \
    ARTIFACT_NAME="$ARTIFACT_NAME" \
    BENCHMARK_ARTIFACT_URL="$benchmark_artifact_url" \
    BENCHMARK_REPORT_FILE="$benchmark_report_file" \
    BENCHMARK_SUMMARY_FILE="$benchmark_summary_file" \
    BENCHMARK_CONTEXT_PACK_FIRST="$benchmark_context_pack_first" \
    BENCHMARK_ROUTING_TOTAL="$benchmark_routing_total" \
    BENCHMARK_LINE_REDUCTION="$benchmark_line_reduction" \
    BENCHMARK_GUARDRAIL_FAILURES="$benchmark_guardrail_failures" \
    BENCHMARK_TRUNCATED_PACKS="$benchmark_truncated_packs" \
    CONTEXT_PACK_QUALITY_ARTIFACT_NAME="$CONTEXT_PACK_QUALITY_ARTIFACT_NAME" \
    CONTEXT_PACK_QUALITY_ARTIFACT_URL="$context_pack_quality_artifact_url" \
    CONTEXT_PACK_QUALITY_SUMMARY_FILE="$context_pack_quality_summary_file" \
    AGENT_ROUTE_ARTIFACT_NAME="$AGENT_ROUTE_ARTIFACT_NAME" \
    AGENT_ROUTE_ARTIFACT_URL="$agent_route_artifact_url" \
    AGENT_ROUTE_SUMMARY_FILE="$agent_route_summary_file" \
    AGENT_ROUTE_FIRST_SELECTION_RANK="$agent_route_first_selection_rank" \
    AGENT_ROUTE_FIRST_SELECTION_REASON="$agent_route_first_selection_reason" \
    AGENT_ROUTE_CONTINUATION_STATUS="$agent_route_continuation_status" \
    AGENT_ROUTE_CONTINUATION_NEXT_ACTION="$agent_route_continuation_next_action" \
    MCP_FIRST_CALL_ARTIFACT_NAME="$MCP_FIRST_CALL_ARTIFACT_NAME" \
    MCP_FIRST_CALL_ARTIFACT_URL="$mcp_first_call_artifact_url" \
    MCP_FIRST_CALL_SUMMARY_FILE="$mcp_first_call_summary_file" \
    ADOPTION_REPORT_NAME="$ADOPTION_REPORT_NAME" \
    ADOPTION_REPORT_DOC="$ADOPTION_REPORT_DOC" \
    ADOPTION_REPORT_COMMAND="$ADOPTION_REPORT_COMMAND" \
    ADOPTION_REPORT_ARCHIVE="$ADOPTION_REPORT_ARCHIVE" \
    ADOPTION_REPORT_SELECTED_LINES="$ADOPTION_REPORT_SELECTED_LINES" \
    ADOPTION_REPORT_TOTAL_LINES="$ADOPTION_REPORT_TOTAL_LINES" \
    ADOPTION_REPORT_LINE_REDUCTION="$ADOPTION_REPORT_LINE_REDUCTION" \
    ADOPTION_REPORT_READING_ORDER="$ADOPTION_REPORT_READING_ORDER" \
    ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF="$ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF" \
    ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT="$ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT" \
    ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED="$ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED" \
    METADATA_SUMMARY="$metadata_summary" \
    ruby -rjson - "$output_file" <<'RUBY'
output_file = ARGV.fetch(0)
metadata = ENV.fetch("METADATA_SUMMARY").lines(chomp: true).map do |line|
  key, value = line.split(": ", 2)
  next unless key && value
  [key, value]
end.compact.to_h

release_notes = [
  "## #{ENV.fetch("TAG_NAME")} release evidence",
  "",
  "- Target commit: `#{ENV.fetch("HEAD_SHA")}`",
  "- CI: [run #{ENV.fetch("RUN_ID")}](#{ENV.fetch("CI_URL")})",
  "- Benchmark artifact: [#{ENV.fetch("ARTIFACT_NAME")}](#{ENV.fetch("BENCHMARK_ARTIFACT_URL")})",
  "- Benchmark report: `#{ENV.fetch("BENCHMARK_REPORT_FILE")}`",
  "- Benchmark summary: `#{ENV.fetch("BENCHMARK_SUMMARY_FILE")}`",
  "- Benchmark routing: `context_pack` first for #{ENV.fetch("BENCHMARK_CONTEXT_PACK_FIRST")}/#{ENV.fetch("BENCHMARK_ROUTING_TOTAL")} repositories",
  "- Benchmark line reduction: `#{ENV.fetch("BENCHMARK_LINE_REDUCTION")}`",
  "- Benchmark guardrail failures: `#{ENV.fetch("BENCHMARK_GUARDRAIL_FAILURES")}`",
  "- Benchmark truncated context packs: `#{ENV.fetch("BENCHMARK_TRUNCATED_PACKS")}`",
  "- Context-pack quality artifact: [#{ENV.fetch("CONTEXT_PACK_QUALITY_ARTIFACT_NAME")}](#{ENV.fetch("CONTEXT_PACK_QUALITY_ARTIFACT_URL")})",
  "- Context-pack quality summary: `#{ENV.fetch("CONTEXT_PACK_QUALITY_SUMMARY_FILE")}`",
  "- Agent-route artifact: [#{ENV.fetch("AGENT_ROUTE_ARTIFACT_NAME")}](#{ENV.fetch("AGENT_ROUTE_ARTIFACT_URL")})",
  "- Agent-route summary: `#{ENV.fetch("AGENT_ROUTE_SUMMARY_FILE")}`",
  "- Agent-route first selection: rank `#{ENV.fetch("AGENT_ROUTE_FIRST_SELECTION_RANK")}`, #{ENV.fetch("AGENT_ROUTE_FIRST_SELECTION_REASON")}",
  "- Agent-route continuation: `#{ENV.fetch("AGENT_ROUTE_CONTINUATION_STATUS")}`, next action `#{ENV.fetch("AGENT_ROUTE_CONTINUATION_NEXT_ACTION")}`",
  "- MCP first-call artifact: [#{ENV.fetch("MCP_FIRST_CALL_ARTIFACT_NAME")}](#{ENV.fetch("MCP_FIRST_CALL_ARTIFACT_URL")})",
  "- MCP first-call summary: `#{ENV.fetch("MCP_FIRST_CALL_SUMMARY_FILE")}`",
  "- Adoption report: [#{ENV.fetch("ADOPTION_REPORT_NAME")}](#{ENV.fetch("ADOPTION_REPORT_DOC")})",
  "- Adoption report command: `#{ENV.fetch("ADOPTION_REPORT_COMMAND")}`",
  "- Adoption report archive: `#{ENV.fetch("ADOPTION_REPORT_ARCHIVE")}`",
  "- Adoption report routed first-read: `#{ENV.fetch("ADOPTION_REPORT_SELECTED_LINES")}/#{ENV.fetch("ADOPTION_REPORT_TOTAL_LINES")}` source lines, `#{ENV.fetch("ADOPTION_REPORT_LINE_REDUCTION")}` reduction",
  "- Adoption report MCP first-call contract: `reading_order=#{ENV.fetch("ADOPTION_REPORT_READING_ORDER")}`, `suggested_tool_handoff=#{ENV.fetch("ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF")}`, `continuation_after_selected_context=#{ENV.fetch("ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT")}`, `suggested_tool_executed=#{ENV.fetch("ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED")}`",
  *metadata.map { |key, value| "- #{key}: #{value}" }
].join("\n")

summary = {
  "schema_version" => 1,
  "tag" => ENV.fetch("TAG_NAME"),
  "branch" => ENV.fetch("BRANCH"),
  "head_sha" => ENV.fetch("HEAD_SHA"),
  "repo" => ENV.fetch("REPO_NAME"),
  "metadata" => {
    "cargo" => metadata.fetch("metadata_cargo"),
    "install" => metadata.fetch("metadata_install"),
    "changelog" => metadata.fetch("metadata_changelog")
  },
  "ci" => {
    "run_id" => ENV.fetch("RUN_ID"),
    "url" => ENV.fetch("CI_URL")
  },
  "artifacts" => {
    "benchmark" => {
      "name" => ENV.fetch("ARTIFACT_NAME"),
      "url" => ENV.fetch("BENCHMARK_ARTIFACT_URL"),
      "report" => ENV.fetch("BENCHMARK_REPORT_FILE"),
      "summary" => ENV.fetch("BENCHMARK_SUMMARY_FILE"),
      "metrics" => {
        "context_pack_first" => ENV.fetch("BENCHMARK_CONTEXT_PACK_FIRST").to_i,
        "routing_total" => ENV.fetch("BENCHMARK_ROUTING_TOTAL").to_i,
        "line_reduction" => ENV.fetch("BENCHMARK_LINE_REDUCTION"),
        "guardrail_failures" => ENV.fetch("BENCHMARK_GUARDRAIL_FAILURES").to_i,
        "truncated_packs" => ENV.fetch("BENCHMARK_TRUNCATED_PACKS").to_i
      }
    },
    "context_pack_quality" => {
      "name" => ENV.fetch("CONTEXT_PACK_QUALITY_ARTIFACT_NAME"),
      "url" => ENV.fetch("CONTEXT_PACK_QUALITY_ARTIFACT_URL"),
      "summary" => ENV.fetch("CONTEXT_PACK_QUALITY_SUMMARY_FILE")
    },
    "agent_route" => {
      "name" => ENV.fetch("AGENT_ROUTE_ARTIFACT_NAME"),
      "url" => ENV.fetch("AGENT_ROUTE_ARTIFACT_URL"),
      "summary" => ENV.fetch("AGENT_ROUTE_SUMMARY_FILE"),
      "metrics" => {
        "first_selection_rank" => ENV.fetch("AGENT_ROUTE_FIRST_SELECTION_RANK").to_i,
        "first_selection_reason" => ENV.fetch("AGENT_ROUTE_FIRST_SELECTION_REASON"),
        "continuation_status" => ENV.fetch("AGENT_ROUTE_CONTINUATION_STATUS"),
        "continuation_next_action" => ENV.fetch("AGENT_ROUTE_CONTINUATION_NEXT_ACTION")
      }
    },
    "mcp_first_call" => {
      "name" => ENV.fetch("MCP_FIRST_CALL_ARTIFACT_NAME"),
      "url" => ENV.fetch("MCP_FIRST_CALL_ARTIFACT_URL"),
      "summary" => ENV.fetch("MCP_FIRST_CALL_SUMMARY_FILE")
    },
    "adoption_report" => {
      "name" => ENV.fetch("ADOPTION_REPORT_NAME"),
      "document" => ENV.fetch("ADOPTION_REPORT_DOC"),
      "command" => ENV.fetch("ADOPTION_REPORT_COMMAND"),
      "archive" => ENV.fetch("ADOPTION_REPORT_ARCHIVE"),
      "metrics" => {
        "selected_lines" => ENV.fetch("ADOPTION_REPORT_SELECTED_LINES").to_i,
        "total_lines" => ENV.fetch("ADOPTION_REPORT_TOTAL_LINES").to_i,
        "line_reduction" => ENV.fetch("ADOPTION_REPORT_LINE_REDUCTION"),
        "mcp_first_call_contract" => {
          "reading_order" => ENV.fetch("ADOPTION_REPORT_READING_ORDER") == "true",
          "suggested_tool_handoff" => ENV.fetch("ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF") == "true",
          "continuation_after_selected_context" => ENV.fetch("ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT") == "true",
          "suggested_tool_executed" => ENV.fetch("ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED") == "true"
        }
      }
    }
  },
  "release_notes_block" => release_notes
}

File.write(output_file, "#{JSON.pretty_generate(summary)}\n")
RUBY
}

main() {
  local metadata_summary
  local run_summary
  local repo_name
  local ci_url
  local benchmark_artifact_url
  local benchmark_artifact_validation
  local benchmark_report_file
  local benchmark_summary_file
  local benchmark_context_pack_first
  local benchmark_routing_total
  local benchmark_line_reduction
  local benchmark_guardrail_failures
  local benchmark_truncated_packs
  local context_pack_quality_artifact_url
  local context_pack_quality_summary_file
  local agent_route_artifact_url
  local agent_route_summary_file
  local agent_route_first_selection_rank
  local agent_route_first_selection_reason
  local agent_route_continuation_status
  local agent_route_continuation_next_action
  local mcp_first_call_artifact_url
  local mcp_first_call_summary_file

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
        REPO="$1"
        REPO_ARG=(--repo "$1")
        ;;
      --head-sha)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        HEAD_SHA="$1"
        ;;
      --run-id)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        RUN_ID="$1"
        ;;
      --artifact-name)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        ARTIFACT_NAME="$1"
        ;;
      --quality-artifact-name)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        CONTEXT_PACK_QUALITY_ARTIFACT_NAME="$1"
        ;;
      --agent-route-artifact-name)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        AGENT_ROUTE_ARTIFACT_NAME="$1"
        ;;
      --mcp-first-call-artifact-name)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        MCP_FIRST_CALL_ARTIFACT_NAME="$1"
        ;;
      --json-output)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        JSON_OUTPUT_FILE="$1"
        ;;
      --)
        shift
        break
        ;;
      -*)
        usage
        ;;
      *)
        if [ -z "$TAG_NAME" ]; then
          TAG_NAME="$(normalize_tag "$1")"
        elif [ "$BRANCH" = "main" ]; then
          BRANCH="$1"
        else
          usage
        fi
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    if [ -z "$TAG_NAME" ]; then
      TAG_NAME="$(normalize_tag "$1")"
    elif [ "$BRANCH" = "main" ]; then
      BRANCH="$1"
    else
      usage
    fi
    shift
  done

  if [ -z "$TAG_NAME" ]; then
    usage
  fi
  if [[ ! "$TAG_NAME" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    fail "tag must look like vX.Y.Z or X.Y.Z: $TAG_NAME"
  fi
  if ! command -v gh >/dev/null 2>&1; then
    fail "missing required command: gh"
  fi
  if ! command -v jq >/dev/null 2>&1; then
    fail "missing required command: jq"
  fi
  if [ ! -x "$RELEASE_METADATA_SUMMARY_SCRIPT" ]; then
    fail "release metadata summary script is not executable: $RELEASE_METADATA_SUMMARY_SCRIPT"
  fi

  if [ -z "$HEAD_SHA" ]; then
    HEAD_SHA="$(git -C "$ROOT_DIR" rev-parse HEAD)"
  fi

  metadata_summary="$(
    CODEINSIGHT_ROOT_DIR="$ROOT_DIR" \
      CODEINSIGHT_RELEASE_METADATA_CONTEXT="release evidence summary" \
      "$RELEASE_METADATA_SUMMARY_SCRIPT" "$TAG_NAME"
  )"
  if [ -z "$RUN_ID" ]; then
    RUN_ID="$(resolve_run_by_head_sha "$BRANCH" "$HEAD_SHA")"
  fi
  repo_name="$(resolve_repo)"
  run_summary="$(validate_run "$RUN_ID" "$HEAD_SHA")"
  ci_url="$(printf "%s\n" "$run_summary" | awk -F': ' '/^ci_url: / { print $2; exit }')"
  benchmark_artifact_url="$(resolve_artifact_url "$repo_name" "$RUN_ID" "$ARTIFACT_NAME")"
  context_pack_quality_artifact_url="$(resolve_artifact_url "$repo_name" "$RUN_ID" "$CONTEXT_PACK_QUALITY_ARTIFACT_NAME")"
  agent_route_artifact_url="$(resolve_artifact_url "$repo_name" "$RUN_ID" "$AGENT_ROUTE_ARTIFACT_NAME")"
  mcp_first_call_artifact_url="$(resolve_artifact_url "$repo_name" "$RUN_ID" "$MCP_FIRST_CALL_ARTIFACT_NAME")"
  benchmark_artifact_validation="$(validate_benchmark_artifact "$RUN_ID")"
  benchmark_report_file="$(printf "%s\n" "$benchmark_artifact_validation" | awk -F': ' '/^report: / { print $2; exit }')"
  benchmark_summary_file="$(printf "%s\n" "$benchmark_artifact_validation" | awk -F': ' '/^summary: / { print $2; exit }')"
  if [ -z "$benchmark_report_file" ]; then
    fail "benchmark artifact smoke did not report a Markdown report path"
  fi
  if [ -z "$benchmark_summary_file" ]; then
    fail "benchmark artifact smoke did not report a JSON summary path"
  fi
  benchmark_context_pack_first="$(benchmark_metric "$benchmark_summary_file" '(.routing.context_pack_first // 0) | tostring' "context_pack routing count")"
  benchmark_routing_total="$(benchmark_metric "$benchmark_summary_file" '(.routing.total // 0) | tostring' "routing total")"
  benchmark_line_reduction="$(benchmark_metric "$benchmark_summary_file" '.context.line_reduction // empty' "line reduction")"
  benchmark_guardrail_failures="$(benchmark_metric "$benchmark_summary_file" '(.failures.total // 0) | tostring' "guardrail failures")"
  benchmark_truncated_packs="$(benchmark_metric "$benchmark_summary_file" '(.context.truncated_packs // 0) | tostring' "truncated context packs")"
  context_pack_quality_summary_file="$(validate_context_pack_quality_artifact "$RUN_ID")"
  agent_route_summary_file="$(validate_agent_route_artifact "$RUN_ID")"
  agent_route_first_selection_rank="$(agent_route_metric "$agent_route_summary_file" '(.metrics.first_selection_rank // 0) | tostring' "first selection rank")"
  agent_route_first_selection_reason="$(agent_route_metric "$agent_route_summary_file" '.metrics.first_selection_reason // empty' "first selection reason")"
  agent_route_continuation_status="$(agent_route_metric "$agent_route_summary_file" '.metrics.continuation_status // empty' "continuation status")"
  agent_route_continuation_next_action="$(agent_route_metric "$agent_route_summary_file" '.metrics.continuation_next_action // empty' "continuation next action")"
  mcp_first_call_summary_file="$(validate_mcp_first_call_artifact "$RUN_ID")"
  load_adoption_report_doc

  echo "release evidence summary"
  echo "tag: $TAG_NAME"
  echo "branch: $BRANCH"
  echo "head_sha: $HEAD_SHA"
  printf "%s\n" "$metadata_summary"
  printf "%s\n" "$run_summary"
  echo "benchmark_artifact: $ARTIFACT_NAME"
  echo "benchmark_artifact_url: $benchmark_artifact_url"
  echo "benchmark_report: $benchmark_report_file"
  echo "benchmark_summary: $benchmark_summary_file"
  echo "benchmark_context_pack_first: $benchmark_context_pack_first/$benchmark_routing_total"
  echo "benchmark_line_reduction: $benchmark_line_reduction"
  echo "benchmark_guardrail_failures: $benchmark_guardrail_failures"
  echo "benchmark_truncated_packs: $benchmark_truncated_packs"
  echo "context_pack_quality_artifact: $CONTEXT_PACK_QUALITY_ARTIFACT_NAME"
  echo "context_pack_quality_artifact_url: $context_pack_quality_artifact_url"
  echo "context_pack_quality_summary: $context_pack_quality_summary_file"
  echo "agent_route_artifact: $AGENT_ROUTE_ARTIFACT_NAME"
  echo "agent_route_artifact_url: $agent_route_artifact_url"
  echo "agent_route_summary: $agent_route_summary_file"
  echo "agent_route_first_selection_rank: $agent_route_first_selection_rank"
  echo "agent_route_first_selection_reason: $agent_route_first_selection_reason"
  echo "agent_route_continuation_status: $agent_route_continuation_status"
  echo "agent_route_continuation_next_action: $agent_route_continuation_next_action"
  echo "mcp_first_call_artifact: $MCP_FIRST_CALL_ARTIFACT_NAME"
  echo "mcp_first_call_artifact_url: $mcp_first_call_artifact_url"
  echo "mcp_first_call_summary: $mcp_first_call_summary_file"
  echo "adoption_report: $ADOPTION_REPORT_NAME"
  echo "adoption_report_doc: $ADOPTION_REPORT_DOC"
  echo "adoption_report_archive: $ADOPTION_REPORT_ARCHIVE"
  echo "adoption_report_command: $ADOPTION_REPORT_COMMAND"
  echo "adoption_report_selected_lines: $ADOPTION_REPORT_SELECTED_LINES/$ADOPTION_REPORT_TOTAL_LINES"
  echo "adoption_report_line_reduction: $ADOPTION_REPORT_LINE_REDUCTION"
  echo "adoption_report_contract_reading_order: $ADOPTION_REPORT_READING_ORDER"
  echo "adoption_report_contract_suggested_tool_handoff: $ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF"
  echo "adoption_report_contract_continuation_after_selected_context: $ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT"
  echo "adoption_report_contract_suggested_tool_executed: $ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED"
  echo
  echo "release_notes_block:"
  echo "## $TAG_NAME release evidence"
  echo
  echo "- Target commit: \`$HEAD_SHA\`"
  echo "- CI: [run $RUN_ID]($ci_url)"
  echo "- Benchmark artifact: [$ARTIFACT_NAME]($benchmark_artifact_url)"
  echo "- Benchmark report: \`$benchmark_report_file\`"
  echo "- Benchmark summary: \`$benchmark_summary_file\`"
  echo "- Benchmark routing: \`context_pack\` first for $benchmark_context_pack_first/$benchmark_routing_total repositories"
  echo "- Benchmark line reduction: \`$benchmark_line_reduction\`"
  echo "- Benchmark guardrail failures: \`$benchmark_guardrail_failures\`"
  echo "- Benchmark truncated context packs: \`$benchmark_truncated_packs\`"
  echo "- Context-pack quality artifact: [$CONTEXT_PACK_QUALITY_ARTIFACT_NAME]($context_pack_quality_artifact_url)"
  echo "- Context-pack quality summary: \`$context_pack_quality_summary_file\`"
  echo "- Agent-route artifact: [$AGENT_ROUTE_ARTIFACT_NAME]($agent_route_artifact_url)"
  echo "- Agent-route summary: \`$agent_route_summary_file\`"
  echo "- Agent-route first selection: rank \`$agent_route_first_selection_rank\`, $agent_route_first_selection_reason"
  echo "- Agent-route continuation: \`$agent_route_continuation_status\`, next action \`$agent_route_continuation_next_action\`"
  echo "- MCP first-call artifact: [$MCP_FIRST_CALL_ARTIFACT_NAME]($mcp_first_call_artifact_url)"
  echo "- MCP first-call summary: \`$mcp_first_call_summary_file\`"
  echo "- Adoption report: [$ADOPTION_REPORT_NAME]($ADOPTION_REPORT_DOC)"
  echo "- Adoption report command: \`$ADOPTION_REPORT_COMMAND\`"
  echo "- Adoption report archive: \`$ADOPTION_REPORT_ARCHIVE\`"
  echo "- Adoption report routed first-read: \`$ADOPTION_REPORT_SELECTED_LINES/$ADOPTION_REPORT_TOTAL_LINES\` source lines, \`$ADOPTION_REPORT_LINE_REDUCTION\` reduction"
  echo "- Adoption report MCP first-call contract: \`reading_order=$ADOPTION_REPORT_READING_ORDER\`, \`suggested_tool_handoff=$ADOPTION_REPORT_SUGGESTED_TOOL_HANDOFF\`, \`continuation_after_selected_context=$ADOPTION_REPORT_CONTINUATION_AFTER_SELECTED_CONTEXT\`, \`suggested_tool_executed=$ADOPTION_REPORT_SUGGESTED_TOOL_EXECUTED\`"
  printf "%s\n" "$metadata_summary" | sed 's/^/- /'

  if [ -n "$JSON_OUTPUT_FILE" ]; then
    write_json_summary \
      "$JSON_OUTPUT_FILE" \
      "$metadata_summary" \
      "$repo_name" \
      "$ci_url" \
      "$benchmark_artifact_url" \
      "$benchmark_report_file" \
      "$benchmark_summary_file" \
      "$benchmark_context_pack_first" \
      "$benchmark_routing_total" \
      "$benchmark_line_reduction" \
      "$benchmark_guardrail_failures" \
      "$benchmark_truncated_packs" \
      "$context_pack_quality_artifact_url" \
      "$context_pack_quality_summary_file" \
      "$agent_route_artifact_url" \
      "$agent_route_summary_file" \
      "$agent_route_first_selection_rank" \
      "$agent_route_first_selection_reason" \
      "$agent_route_continuation_status" \
      "$agent_route_continuation_next_action" \
      "$mcp_first_call_artifact_url" \
      "$mcp_first_call_summary_file"
  fi
}

main "$@"
