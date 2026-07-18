#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BLOCK_FILE=""
EVIDENCE_FILE=""
EVIDENCE_JSON_FILE=""

cleanup() {
  if [ -n "$BLOCK_FILE" ]; then
    rm -f "$BLOCK_FILE"
  fi
}

usage() {
  cat >&2 <<'EOF'
usage: scripts/update-release-status.sh [options] <verify-release-summary.json> [status-doc]

Updates the generated release verification summary block in docs/status.md.

Options:
  --evidence-json-file PATH  Include archived pre-release evidence JSON fields.
  --evidence-file PATH       Include archived pre-release evidence Markdown fields.
  -h, --help                 Show this help.

Environment:
  CODEINSIGHT_STATUS_DATE=YYYY-MM-DD
EOF
  exit 2
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

status_label() {
  case "$1" in
    passed) printf 'passed' ;;
    skipped) printf 'skipped' ;;
    metadata_only) printf 'metadata-only' ;;
    *) printf '%s' "$1" ;;
  esac
}

evidence_field() {
  local key="$1"

  awk -v key="$key" '
    index($0, key ": ") == 1 {
      print substr($0, length(key) + 3)
      exit
    }
  ' "$EVIDENCE_FILE"
}

json_field() {
  local query="$1"

  jq -er "$query" "$EVIDENCE_JSON_FILE"
}

require_evidence_field() {
  local key="$1"
  local description="$2"
  local value

  value="$(evidence_field "$key")"
  if [ -z "$value" ]; then
    echo "release evidence is missing ${description}: $key" >&2
    exit 1
  fi
  printf '%s' "$value"
}

require_evidence_json_field() {
  local query="$1"
  local description="$2"
  local value

  if ! value="$(json_field "$query")"; then
    echo "release evidence JSON is missing ${description}: $query" >&2
    exit 1
  fi
  if [ -z "$value" ] || [ "$value" = "null" ]; then
    echo "release evidence JSON is missing ${description}: $query" >&2
    exit 1
  fi
  printf '%s' "$value"
}

main() {
  require_command jq
  require_command ruby

  local positional=()
  local summary_file
  local status_file
  local generated_date="${CODEINSIGHT_STATUS_DATE:-$(date +%F)}"
  local evidence_head_sha=""
  local evidence_ci_run=""
  local evidence_metadata_cargo=""
  local evidence_metadata_install=""
  local evidence_metadata_changelog=""
  local evidence_benchmark_name="codeinsight-benchmark-subset"
  local evidence_benchmark_url=""
  local evidence_context_pack_quality_name="codeinsight-context-pack-quality"
  local evidence_context_pack_quality_url=""
  local evidence_agent_route_name="codeinsight-agent-route-smoke"
  local evidence_agent_route_url=""
  local evidence_agent_route_first_selection_rank=""
  local evidence_agent_route_first_selection_reason=""
  local evidence_agent_route_continuation_status=""
  local evidence_agent_route_continuation_next_action=""
  local evidence_mcp_first_call_name="codeinsight-mcp-first-call"
  local evidence_mcp_first_call_url=""
  local evidence_adoption_report_name=""
  local evidence_adoption_report_doc=""
  local evidence_adoption_report_command=""
  local evidence_adoption_report_archive=""
  local evidence_adoption_report_selected_lines=""
  local evidence_adoption_report_line_reduction=""
  local evidence_adoption_report_reading_order=""
  local evidence_adoption_report_suggested_tool_handoff=""
  local evidence_adoption_report_continuation_after_selected_context=""
  local evidence_adoption_report_suggested_tool_executed=""
  local evidence_display_file=""

  while [ "$#" -gt 0 ]; do
    case "$1" in
      -h | --help)
        usage
        ;;
      --evidence-file)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EVIDENCE_FILE="$1"
        ;;
      --evidence-json-file)
        shift
        if [ "$#" -eq 0 ]; then
          usage
        fi
        EVIDENCE_JSON_FILE="$1"
        ;;
      --)
        shift
        break
        ;;
      -*)
        usage
        ;;
      *)
        positional+=("$1")
        ;;
    esac
    shift
  done

  while [ "$#" -gt 0 ]; do
    positional+=("$1")
    shift
  done

  if [ "${#positional[@]}" -lt 1 ] || [ "${#positional[@]}" -gt 2 ]; then
    usage
  fi

  summary_file="${positional[0]}"
  status_file="${positional[1]:-$ROOT_DIR/docs/status.md}"

  if [ ! -f "$summary_file" ]; then
    echo "summary JSON not found: $summary_file" >&2
    exit 1
  fi
  if [ ! -f "$status_file" ]; then
    echo "status document not found: $status_file" >&2
    exit 1
  fi
  if [ -n "$EVIDENCE_JSON_FILE" ]; then
    if [ ! -f "$EVIDENCE_JSON_FILE" ]; then
      echo "release evidence JSON file not found: $EVIDENCE_JSON_FILE" >&2
      exit 1
    fi
    jq -e '.schema_version == 1' "$EVIDENCE_JSON_FILE" >/dev/null
    evidence_display_file="$EVIDENCE_JSON_FILE"
    evidence_head_sha="$(require_evidence_json_field '.head_sha' "head SHA")"
    evidence_ci_run="$(require_evidence_json_field '.ci.run_id' "CI run")"
    evidence_metadata_cargo="$(require_evidence_json_field '.metadata.cargo' "Cargo metadata")"
    evidence_metadata_install="$(require_evidence_json_field '.metadata.install' "install metadata")"
    evidence_metadata_changelog="$(require_evidence_json_field '.metadata.changelog' "changelog metadata")"
    evidence_benchmark_name="$(require_evidence_json_field '.artifacts.benchmark.name' "benchmark artifact name")"
    evidence_benchmark_url="$(require_evidence_json_field '.artifacts.benchmark.url' "benchmark artifact URL")"
    evidence_context_pack_quality_name="$(require_evidence_json_field '.artifacts.context_pack_quality.name' "context-pack quality artifact name")"
    evidence_context_pack_quality_url="$(require_evidence_json_field '.artifacts.context_pack_quality.url' "context-pack quality artifact URL")"
    evidence_agent_route_name="$(require_evidence_json_field '.artifacts.agent_route.name' "agent-route artifact name")"
    evidence_agent_route_url="$(require_evidence_json_field '.artifacts.agent_route.url' "agent-route artifact URL")"
    if jq -e '.artifacts.agent_route.metrics? != null' "$EVIDENCE_JSON_FILE" >/dev/null; then
      evidence_agent_route_first_selection_rank="$(require_evidence_json_field '(.artifacts.agent_route.metrics.first_selection_rank | tostring)' "agent-route first selection rank")"
      evidence_agent_route_first_selection_reason="$(require_evidence_json_field '.artifacts.agent_route.metrics.first_selection_reason' "agent-route first selection reason")"
      evidence_agent_route_continuation_status="$(require_evidence_json_field '.artifacts.agent_route.metrics.continuation_status' "agent-route continuation status")"
      evidence_agent_route_continuation_next_action="$(require_evidence_json_field '.artifacts.agent_route.metrics.continuation_next_action' "agent-route continuation next action")"
    fi
    evidence_mcp_first_call_name="$(require_evidence_json_field '.artifacts.mcp_first_call.name' "MCP first-call artifact name")"
    evidence_mcp_first_call_url="$(require_evidence_json_field '.artifacts.mcp_first_call.url' "MCP first-call artifact URL")"
    if jq -e '.artifacts.adoption_report? != null' "$EVIDENCE_JSON_FILE" >/dev/null; then
      evidence_adoption_report_name="$(require_evidence_json_field '.artifacts.adoption_report.name' "adoption report name")"
      evidence_adoption_report_doc="$(require_evidence_json_field '.artifacts.adoption_report.document' "adoption report document")"
      evidence_adoption_report_command="$(require_evidence_json_field '.artifacts.adoption_report.command' "adoption report command")"
      evidence_adoption_report_archive="$(require_evidence_json_field '.artifacts.adoption_report.archive' "adoption report archive")"
      evidence_adoption_report_selected_lines="$(require_evidence_json_field '(.artifacts.adoption_report.metrics.selected_lines | tostring) + "/" + (.artifacts.adoption_report.metrics.total_lines | tostring)' "adoption report selected lines")"
      evidence_adoption_report_line_reduction="$(require_evidence_json_field '.artifacts.adoption_report.metrics.line_reduction' "adoption report line reduction")"
      evidence_adoption_report_reading_order="$(require_evidence_json_field '(.artifacts.adoption_report.metrics.mcp_first_call_contract.reading_order | tostring)' "adoption report reading-order contract")"
      evidence_adoption_report_suggested_tool_handoff="$(require_evidence_json_field '(.artifacts.adoption_report.metrics.mcp_first_call_contract.suggested_tool_handoff | tostring)' "adoption report suggested-tool contract")"
      evidence_adoption_report_continuation_after_selected_context="$(require_evidence_json_field '(.artifacts.adoption_report.metrics.mcp_first_call_contract.continuation_after_selected_context | tostring)' "adoption report continuation contract")"
      evidence_adoption_report_suggested_tool_executed="$(require_evidence_json_field '(.artifacts.adoption_report.metrics.mcp_first_call_contract.suggested_tool_executed | tostring)' "adoption report suggested-tool execution")"
    fi
  elif [ -n "$EVIDENCE_FILE" ]; then
    if [ ! -f "$EVIDENCE_FILE" ]; then
      echo "release evidence file not found: $EVIDENCE_FILE" >&2
      exit 1
    fi
    evidence_display_file="$EVIDENCE_FILE"
    evidence_head_sha="$(require_evidence_field head_sha "head SHA")"
    evidence_ci_run="$(require_evidence_field ci_run "CI run")"
    evidence_metadata_cargo="$(require_evidence_field metadata_cargo "Cargo metadata")"
    evidence_metadata_install="$(require_evidence_field metadata_install "install metadata")"
    evidence_metadata_changelog="$(require_evidence_field metadata_changelog "changelog metadata")"
    evidence_benchmark_url="$(require_evidence_field benchmark_artifact_url "benchmark artifact URL")"
    evidence_context_pack_quality_url="$(require_evidence_field context_pack_quality_artifact_url "context-pack quality artifact URL")"
    evidence_agent_route_url="$(require_evidence_field agent_route_artifact_url "agent-route artifact URL")"
    evidence_agent_route_first_selection_rank="$(evidence_field agent_route_first_selection_rank)"
    evidence_agent_route_first_selection_reason="$(evidence_field agent_route_first_selection_reason)"
    evidence_agent_route_continuation_status="$(evidence_field agent_route_continuation_status)"
    evidence_agent_route_continuation_next_action="$(evidence_field agent_route_continuation_next_action)"
    evidence_mcp_first_call_url="$(require_evidence_field mcp_first_call_artifact_url "MCP first-call artifact URL")"
    evidence_adoption_report_name="$(evidence_field adoption_report)"
    if [ -n "$evidence_adoption_report_name" ]; then
      evidence_adoption_report_doc="$(require_evidence_field adoption_report_doc "adoption report document")"
      evidence_adoption_report_command="$(require_evidence_field adoption_report_command "adoption report command")"
      evidence_adoption_report_archive="$(require_evidence_field adoption_report_archive "adoption report archive")"
      evidence_adoption_report_selected_lines="$(require_evidence_field adoption_report_selected_lines "adoption report selected lines")"
      evidence_adoption_report_line_reduction="$(require_evidence_field adoption_report_line_reduction "adoption report line reduction")"
      evidence_adoption_report_reading_order="true"
      evidence_adoption_report_suggested_tool_handoff="true"
      evidence_adoption_report_continuation_after_selected_context="true"
      evidence_adoption_report_suggested_tool_executed="true"
    fi
  fi

  jq -e '
    .status == "passed" and
    (.tag | type == "string" and length > 0) and
    (.repo | type == "string" and length > 0) and
    (.version | type == "string" and length > 0) and
    (.gates | type == "object") and
    (.expected_assets | type == "array")
  ' "$summary_file" >/dev/null

  BLOCK_FILE="$(mktemp)"
  trap cleanup EXIT INT TERM

  {
    echo "<!-- release-verification-summary:start -->"
    echo "### Release Verification Summary"
    echo
    printf 'Generated from `scripts/verify-release.sh --json` on %s.\n' "$generated_date"
    echo
    printf -- '- Status: `%s`\n' "$(jq -r '.status' "$summary_file")"
    printf -- '- Tag: `%s`\n' "$(jq -r '.tag' "$summary_file")"
    printf -- '- Version: `%s`\n' "$(jq -r '.version' "$summary_file")"
    printf -- '- Repository: `%s`\n' "$(jq -r '.repo' "$summary_file")"
    echo "- Gates:"
    jq -r '.gates | to_entries[] | [.key, .value] | @tsv' "$summary_file" |
      while IFS="$(printf '\t')" read -r gate status; do
        printf '  - `%s`: `%s`\n' "$gate" "$(status_label "$status")"
      done
    echo "- Expected release assets:"
    jq -r '.expected_assets[]' "$summary_file" |
      while IFS= read -r asset; do
        printf '  - `%s`\n' "$asset"
      done
    printf -- '- Docker image: `%s` (%s)\n' \
      "$(jq -r '.docker.image // "-"' "$summary_file")" \
      "$(if [ "$(jq -r '.docker.skipped // false' "$summary_file")" = "true" ]; then printf 'skipped locally'; else printf 'verified'; fi)"
    printf -- '- Homebrew tap: `%s` (%s)\n' \
      "$(jq -r '.homebrew.tap // "-"' "$summary_file")" \
      "$(if [ "$(jq -r '.homebrew.skipped // false' "$summary_file")" = "true" ]; then printf 'skipped locally'; else printf 'verified'; fi)"
    printf -- '- Installed quickstart binary: `%s` (%s)\n' \
      "$(jq -r '.installed_quickstart.binary // "-"' "$summary_file")" \
      "$(if [ "$(jq -r '.installed_quickstart.skipped // false' "$summary_file")" = "true" ]; then printf 'skipped locally'; else printf 'verified'; fi)"
    printf -- '- Installed quickstart coverage: `%s`\n' \
      "$(jq -r '(.installed_quickstart.coverage // []) | join("`, `")' "$summary_file")"
    if [ -n "$evidence_display_file" ]; then
      echo "- Pre-release evidence:"
      printf '  - Evidence file: `%s`\n' "$evidence_display_file"
      printf '  - Target commit: `%s`\n' "$evidence_head_sha"
      printf '  - CI run: `%s`\n' "$evidence_ci_run"
      printf '  - Metadata: `cargo=%s`, `install=%s`, `changelog=%s`\n' \
        "$evidence_metadata_cargo" \
        "$evidence_metadata_install" \
        "$evidence_metadata_changelog"
      printf '  - Benchmark artifact: [%s](%s)\n' "$evidence_benchmark_name" "$evidence_benchmark_url"
      printf '  - Context-pack quality artifact: [%s](%s)\n' "$evidence_context_pack_quality_name" "$evidence_context_pack_quality_url"
      printf '  - Agent-route artifact: [%s](%s)\n' "$evidence_agent_route_name" "$evidence_agent_route_url"
      if [ -n "$evidence_agent_route_first_selection_rank" ]; then
        printf '  - Agent-route first selection: rank `%s`, %s\n' \
          "$evidence_agent_route_first_selection_rank" \
          "$evidence_agent_route_first_selection_reason"
        printf '  - Agent-route continuation: `%s`, next action `%s`\n' \
          "$evidence_agent_route_continuation_status" \
          "$evidence_agent_route_continuation_next_action"
      fi
      printf '  - MCP first-call artifact: [%s](%s)\n' "$evidence_mcp_first_call_name" "$evidence_mcp_first_call_url"
      if [ -n "$evidence_adoption_report_name" ]; then
        printf '  - Adoption report: [%s](%s)\n' "$evidence_adoption_report_name" "$evidence_adoption_report_doc"
        printf '  - Adoption report command: `%s`\n' "$evidence_adoption_report_command"
        printf '  - Adoption report archive: `%s`\n' "$evidence_adoption_report_archive"
        printf '  - Adoption report routed first-read: `%s` source lines, `%s` reduction\n' "$evidence_adoption_report_selected_lines" "$evidence_adoption_report_line_reduction"
        printf '  - Adoption report MCP first-call contract: `reading_order=%s`, `suggested_tool_handoff=%s`, `continuation_after_selected_context=%s`, `suggested_tool_executed=%s`\n' \
          "$evidence_adoption_report_reading_order" \
          "$evidence_adoption_report_suggested_tool_handoff" \
          "$evidence_adoption_report_continuation_after_selected_context" \
          "$evidence_adoption_report_suggested_tool_executed"
      fi
    fi
    echo "<!-- release-verification-summary:end -->"
  } >"$BLOCK_FILE"

  ruby - "$status_file" "$BLOCK_FILE" <<'RUBY'
status_path, block_path = ARGV
status = File.read(status_path)
block = File.read(block_path).strip
start_marker = "<!-- release-verification-summary:start -->"
end_marker = "<!-- release-verification-summary:end -->"

if status.include?(start_marker) && status.include?(end_marker)
  pattern = /#{Regexp.escape(start_marker)}.*?#{Regexp.escape(end_marker)}\n*/m
  updated = status.sub(pattern, "#{block}\n\n")
else
  heading = /^## Latest Verified Release\n/
  match = status.match(heading)
  abort("Latest Verified Release section not found in #{status_path}") unless match

  insert_at = status.index(/^## /, match.end(0)) || status.length
  updated = status.dup
  updated.insert(insert_at, "\n#{block}\n\n")
end

File.write(status_path, updated)
RUBY

  printf 'updated release verification summary in %s\n' "$status_file"
}

main "$@"
