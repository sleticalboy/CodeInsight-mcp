#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${CODEINSIGHT_BETA_ROOT:-}"
TASK="${CODEINSIGHT_BETA_TASK:-understand the main application entrypoint}"
TOKEN_BUDGET="${CODEINSIGHT_BETA_TOKEN_BUDGET:-6000}"
OUTPUT_DIR="${CODEINSIGHT_BETA_OUTPUT_DIR:-}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
REPO_URL="${CODEINSIGHT_BETA_REPO_URL:-}"
EXPECTED_FIRST_READ="${CODEINSIGHT_BETA_EXPECTED_FIRST_READ:-}"
INSTALL_METHOD="${CODEINSIGHT_BETA_INSTALL_METHOD:-}"
MCP_CLIENT="${CODEINSIGHT_BETA_MCP_CLIENT:-}"
CODEINSIGHT_VERSION="${CODEINSIGHT_BETA_VERSION:-}"
OUTCOME="${CODEINSIGHT_BETA_OUTCOME:-needs_triage}"
PRINT_SNIPPET="${CODEINSIGHT_BETA_PRINT_SNIPPET:-1}"
FORCE_INDEX="${CODEINSIGHT_BETA_FORCE_INDEX:-1}"
PRIVATE_REPO="${CODEINSIGHT_BETA_PRIVATE_REPO:-0}"
ADOPTION_EVIDENCE_SCRIPT="${CODEINSIGHT_ADOPTION_EVIDENCE_SCRIPT:-$ROOT_DIR/scripts/adoption-evidence.sh}"
SEED_FILES=()
SEED_SYMBOLS=()

usage() {
  cat <<'EOF'
usage: scripts/external-beta-trial.sh [REPO_ROOT] [options]

Runs a shareable external Beta trial for one real repository. The script wraps
adoption evidence generation, then writes a GitHub issue body, redaction
checklist, and maintainer triage note.

Options:
  --root PATH                 Repository root. Also accepted as first argument.
  --task TEXT                 Exact task passed to agent_route.
  --file PATH                 Explicit seed file passed to agent_route. Repeatable.
  --symbol NAME               Explicit seed symbol passed to agent_route. Repeatable.
  --token-budget N            Token budget. Default: 6000.
  --output-dir PATH           Output directory. Default: /tmp/codeinsight-external-beta-trial.
  --bin PATH                  Use a specific codeinsight binary.
  --repo-url URL              Public repository URL, or omit for private repos.
  --expected-first-read TEXT  Expected first file, symbol, or area when known.
  --install-method TEXT       Release installer / Homebrew / Source / Docker.
  --mcp-client TEXT           Codex / Claude Code / Cursor / other.
  --version TEXT              CodeInsight version string.
  --outcome VALUE             needs_triage, route_hit, route_near_miss, route_miss,
                              workflow_friction, or overtrust_risk.
                              Default: needs_triage.
  --private-repo              Mark report as private/redacted.
  --no-print-snippet          Do not print the final issue-body path and summary.
  --no-force-index            Reuse existing index when available.
  -h, --help                  Show this help text.

Environment:
  CODEINSIGHT_BETA_ROOT
  CODEINSIGHT_BETA_TASK
  CODEINSIGHT_BETA_TOKEN_BUDGET
  CODEINSIGHT_BETA_OUTPUT_DIR
  CODEINSIGHT_BETA_REPO_URL
  CODEINSIGHT_BETA_EXPECTED_FIRST_READ
  CODEINSIGHT_BETA_INSTALL_METHOD
  CODEINSIGHT_BETA_MCP_CLIENT
  CODEINSIGHT_BETA_VERSION
  CODEINSIGHT_BETA_OUTCOME
  CODEINSIGHT_BETA_PRIVATE_REPO
  CODEINSIGHT_ADOPTION_EVIDENCE_SCRIPT
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "external beta trial failed [$1]: $2" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail prerequisite "missing required command: $1"
  fi
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --root)
        [ "$#" -ge 2 ] || fail usage "--root requires a path"
        REPO_ROOT="$2"
        shift 2
        ;;
      --task)
        [ "$#" -ge 2 ] || fail usage "--task requires text"
        TASK="$2"
        shift 2
        ;;
      --file)
        [ "$#" -ge 2 ] || fail usage "--file requires a path"
        SEED_FILES+=("$2")
        shift 2
        ;;
      --symbol)
        [ "$#" -ge 2 ] || fail usage "--symbol requires a name"
        SEED_SYMBOLS+=("$2")
        shift 2
        ;;
      --token-budget)
        [ "$#" -ge 2 ] || fail usage "--token-budget requires a number"
        TOKEN_BUDGET="$2"
        shift 2
        ;;
      --output-dir)
        [ "$#" -ge 2 ] || fail usage "--output-dir requires a path"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --bin)
        [ "$#" -ge 2 ] || fail usage "--bin requires a path"
        CODEINSIGHT_BIN="$2"
        shift 2
        ;;
      --repo-url)
        [ "$#" -ge 2 ] || fail usage "--repo-url requires a URL"
        REPO_URL="$2"
        shift 2
        ;;
      --expected-first-read)
        [ "$#" -ge 2 ] || fail usage "--expected-first-read requires text"
        EXPECTED_FIRST_READ="$2"
        shift 2
        ;;
      --install-method)
        [ "$#" -ge 2 ] || fail usage "--install-method requires text"
        INSTALL_METHOD="$2"
        shift 2
        ;;
      --mcp-client)
        [ "$#" -ge 2 ] || fail usage "--mcp-client requires text"
        MCP_CLIENT="$2"
        shift 2
        ;;
      --version)
        [ "$#" -ge 2 ] || fail usage "--version requires text"
        CODEINSIGHT_VERSION="$2"
        shift 2
        ;;
      --outcome)
        [ "$#" -ge 2 ] || fail usage "--outcome requires a value"
        OUTCOME="$2"
        shift 2
        ;;
      --private-repo)
        PRIVATE_REPO="1"
        shift
        ;;
      --no-print-snippet)
        PRINT_SNIPPET="0"
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
        fail usage "unknown argument: $1"
        ;;
      *)
        if [ -n "$REPO_ROOT" ]; then
          fail usage "unexpected positional argument: $1"
        fi
        REPO_ROOT="$1"
        shift
        ;;
    esac
  done
}

validate_args() {
  if [ -z "$REPO_ROOT" ]; then
    fail usage "missing repository root"
  fi
  if [ ! -d "$REPO_ROOT" ]; then
    fail usage "repository root does not exist: $REPO_ROOT"
  fi
  case "$TOKEN_BUDGET" in
    ''|*[!0-9]*)
      fail usage "--token-budget must be a positive integer"
      ;;
  esac
  if [ "$TOKEN_BUDGET" -le 0 ]; then
    fail usage "--token-budget must be greater than zero"
  fi
  case "$OUTCOME" in
    needs_triage|route_hit|route_near_miss|route_miss|workflow_friction|overtrust_risk)
      ;;
    *)
      fail usage "unknown outcome: $OUTCOME"
      ;;
  esac
  if [ ! -x "$ADOPTION_EVIDENCE_SCRIPT" ]; then
    fail prerequisite "adoption evidence script is not executable: $ADOPTION_EVIDENCE_SCRIPT"
  fi
}

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

write_issue_body() {
  local target="$1"
  local summary_json="$2"
  local private_label

  if [ "$PRIVATE_REPO" = "1" ]; then
    private_label="yes, paths may be redacted"
  else
    private_label="no"
  fi

  {
    echo "# External Beta Trial"
    echo
    echo "## Environment"
    echo
    echo "- CodeInsight version: ${CODEINSIGHT_VERSION:-unknown}"
    echo "- Install method: ${INSTALL_METHOD:-unknown}"
    echo "- MCP client: ${MCP_CLIENT:-unknown}"
    echo "- Repository URL: ${REPO_URL:-private or not provided}"
    echo "- Private/redacted repository: $private_label"
    echo "- Repository root used locally: \`$REPO_ROOT\`"
    echo
    echo "## Task"
    echo
    echo '```text'
    echo "$TASK"
    echo '```'
    echo
    echo "## Expected First Read"
    echo
    echo "${EXPECTED_FIRST_READ:-Unknown. Please triage from the generated route and notes.}"
    echo
    echo "## CodeInsight Result"
    echo
    echo "- Outcome: \`$OUTCOME\`"
    echo "- First selected file: \`$(json_value "$summary_json" '.local_evidence.metrics.first_file')\`"
    echo "- First reading focus: $(json_value "$summary_json" '.local_evidence.metrics.first_reading_focus')"
    echo "- First reading question: $(json_value "$summary_json" '.local_evidence.metrics.first_reading_question')"
    echo "- First suggested tool: \`$(json_value "$summary_json" '.local_evidence.metrics.first_suggested_tool')\`"
    echo "- Selected context: \`$(json_value "$summary_json" '.local_evidence.metrics.selected_lines')/$(json_value "$summary_json" '.local_evidence.metrics.total_lines')\` source lines"
    echo "- Source lines avoided: \`$(json_value "$summary_json" '.local_evidence.metrics.source_lines_avoided // 0')\`"
    echo "- Line reduction: \`$(json_value "$summary_json" '.local_evidence.metrics.line_reduction')\`"
    echo "- Read-less ratio: \`$(json_value "$summary_json" '.local_evidence.metrics.read_less_ratio // "n/a"')\`"
    echo "- Impact risk: \`$(json_value "$summary_json" '.local_evidence.metrics.risk_level')\`"
    echo "- Suggested checks: \`$(json_value "$summary_json" '.local_evidence.metrics.suggested_checks')\`"
    echo "- MCP route quality: \`$(json_value "$summary_json" '.mcp_first_call.route_quality.level')\` (\`$(json_value "$summary_json" '.mcp_first_call.route_quality.score')/100\`, \`$(json_value "$summary_json" '.mcp_first_call.route_quality.evidence_count')\` evidence signals), next=\`$(json_value "$summary_json" '.mcp_first_call.route_quality.recommended_action')\`"
    echo "- MCP suggested tool executed: \`$(json_value "$summary_json" '.mcp_first_call.suggested_tool_executed')\`"
    echo "- First-read gating: suggested_tool_after_selected_context=\`$(json_value "$summary_json" '.first_read_gating.suggested_tool_after_selected_context')\`, continuation_after_selected_context=\`$(json_value "$summary_json" '.first_read_gating.continuation_after_selected_context')\`, impact_review_before_edits=\`$(json_value "$summary_json" '.first_read_gating.impact_review_before_edits')\`"
    echo
    echo "## Reproduction"
    echo
    echo '```bash'
    echo "scripts/external-beta-trial.sh \"$REPO_ROOT\" \\"
    echo "  --task \"$TASK\" \\"
    if [ "${#SEED_FILES[@]}" -gt 0 ]; then
      for seed_file in "${SEED_FILES[@]}"; do
        echo "  --file \"$seed_file\" \\"
      done
    fi
    if [ "${#SEED_SYMBOLS[@]}" -gt 0 ]; then
      for seed_symbol in "${SEED_SYMBOLS[@]}"; do
        echo "  --symbol \"$seed_symbol\" \\"
      done
    fi
    echo "  --token-budget $TOKEN_BUDGET \\"
    echo "  --output-dir \"$OUTPUT_DIR\""
    echo '```'
    echo
    echo "## Artifacts"
    echo
    echo "- Beta issue body: \`$OUTPUT_DIR/issue-body.md\`"
    echo "- Beta summary JSON: \`$OUTPUT_DIR/beta-summary.json\`"
    echo "- Redaction checklist: \`$OUTPUT_DIR/redaction-checklist.md\`"
    echo "- Maintainer triage note: \`$OUTPUT_DIR/maintainer-triage.md\`"
    echo "- Adoption evidence: \`$(json_value "$summary_json" '.artifacts.markdown')\`"
    echo "- Aggregate summary JSON: \`$summary_json\`"
    echo "- Raw agent_route JSON: \`$(json_value "$summary_json" '.artifacts.raw_agent_route_json')\`"
    echo "- MCP first-call JSON: \`$(json_value "$summary_json" '.artifacts.mcp_first_call_json')\`"
    echo "- Diagnostic stderr logs: \`$(json_value "$summary_json" '.artifacts.local_stderr')\`, \`$(json_value "$summary_json" '.artifacts.mcp_stderr')\`, \`$(json_value "$summary_json" '.artifacts.artifact_stderr')\`"
    echo
    echo "## Notes"
    echo
    echo "- Did the first selected file help the agent avoid broad reading?"
    echo "- Did the agent read selected files before using the suggested follow-up tool?"
    echo "- Did any output look more certain than best-effort route evidence?"
  } >"$target"
}

write_redaction_checklist() {
  local target="$1"

  {
    echo "# External Beta Redaction Checklist"
    echo
    echo "Before uploading a trial report for a private repository:"
    echo
    echo "- Replace private absolute paths with repository-relative paths."
    echo "- Remove secrets, tokens, customer names, internal hostnames, and private URLs."
    echo "- Keep the exact task text unless it contains confidential names."
    echo '- Keep `summary.json` metrics when possible; redact file paths only when needed.'
    echo '- If raw `agent-route.json` cannot be shared, attach `issue-body.md` and describe the expected area.'
    echo "- Keep stderr logs when they only contain tool errors; redact local user names if needed."
  } >"$target"
}

write_maintainer_triage() {
  local target="$1"
  local summary_json="$2"

  {
    echo "# Maintainer Triage Note"
    echo
    echo "- Intake label: \`adoption-feedback\`"
    echo "- Initial outcome: \`$OUTCOME\`"
    echo "- Add label: \`needs-triage\` unless the outcome has already been verified."
    echo "- If outcome is \`needs_triage\`, reclassify as \`route-hit\`, \`route-near-miss\`, \`route-miss\`, \`workflow-friction\`, or \`overtrust-risk\`."
    echo "- First selected file: \`$(json_value "$summary_json" '.local_evidence.metrics.first_file')\`"
    echo "- Read-less ratio: \`$(json_value "$summary_json" '.local_evidence.metrics.read_less_ratio // "n/a"')\`"
    echo "- MCP route quality: \`$(json_value "$summary_json" '.mcp_first_call.route_quality.level')\` (\`$(json_value "$summary_json" '.mcp_first_call.route_quality.score')/100\`), next=\`$(json_value "$summary_json" '.mcp_first_call.route_quality.recommended_action')\`"
    echo "- MCP first-call status: \`$(json_value "$summary_json" '.mcp_first_call.status')\`"
    echo "- Priority rule: fix workflow friction before low route quality, route misses, overtrust wording, and near misses."
  } >"$target"
}

write_beta_summary_json() {
  local target="$1"
  local summary_json="$2"

  jq -n \
    --arg status "pass" \
    --arg repository "$REPO_ROOT" \
    --arg repository_url "$REPO_URL" \
    --arg task "$TASK" \
    --arg output_dir "$OUTPUT_DIR" \
    --arg expected_first_read "$EXPECTED_FIRST_READ" \
    --arg install_method "$INSTALL_METHOD" \
    --arg mcp_client "$MCP_CLIENT" \
    --arg codeinsight_version "$CODEINSIGHT_VERSION" \
    --arg outcome "$OUTCOME" \
    --argjson private_repo "$PRIVATE_REPO" \
    --slurpfile adoption "$summary_json" \
    '{
      status: $status,
      stage: "external_beta_trial",
      repository: $repository,
      repository_url: $repository_url,
      task: $task,
      expected_first_read: $expected_first_read,
      install_method: $install_method,
      mcp_client: $mcp_client,
      codeinsight_version: $codeinsight_version,
      outcome: $outcome,
      private_repo: ($private_repo == 1),
      adoption_summary: $adoption[0],
      artifacts: {
        issue_body: ($output_dir + "/issue-body.md"),
        beta_summary_json: ($output_dir + "/beta-summary.json"),
        redaction_checklist: ($output_dir + "/redaction-checklist.md"),
        maintainer_triage: ($output_dir + "/maintainer-triage.md"),
        adoption_evidence: ($output_dir + "/adoption-evidence.md"),
        adoption_summary_json: ($output_dir + "/summary.json")
      }
    }' >"$target"
}

main() {
  parse_args "$@"
  require_command jq
  validate_args

  REPO_ROOT="$(cd "$REPO_ROOT" && pwd)"
  OUTPUT_DIR="${OUTPUT_DIR:-/tmp/codeinsight-external-beta-trial}"
  mkdir -p "$OUTPUT_DIR" ||
    fail artifact_write "could not create output directory: $OUTPUT_DIR"

  adoption_args=(
    "$REPO_ROOT"
    "--task"
    "$TASK"
    "--token-budget"
    "$TOKEN_BUDGET"
    "--output-dir"
    "$OUTPUT_DIR"
    "--issue-template"
  )
  if [ "$PRINT_SNIPPET" = "1" ]; then
    adoption_args+=("--print-snippet")
  fi
  if [ "$FORCE_INDEX" != "1" ]; then
    adoption_args+=("--no-force-index")
  fi
  if [ "${#SEED_FILES[@]}" -gt 0 ]; then
    for seed_file in "${SEED_FILES[@]}"; do
      adoption_args+=("--file" "$seed_file")
    done
  fi
  if [ "${#SEED_SYMBOLS[@]}" -gt 0 ]; then
    for seed_symbol in "${SEED_SYMBOLS[@]}"; do
      adoption_args+=("--symbol" "$seed_symbol")
    done
  fi
  if [ -n "$CODEINSIGHT_BIN" ]; then
    adoption_args+=("--bin" "$CODEINSIGHT_BIN")
  fi

  "$ADOPTION_EVIDENCE_SCRIPT" "${adoption_args[@]}"

  write_issue_body "$OUTPUT_DIR/issue-body.md" "$OUTPUT_DIR/summary.json"
  write_redaction_checklist "$OUTPUT_DIR/redaction-checklist.md"
  write_maintainer_triage "$OUTPUT_DIR/maintainer-triage.md" "$OUTPUT_DIR/summary.json"
  write_beta_summary_json "$OUTPUT_DIR/beta-summary.json" "$OUTPUT_DIR/summary.json"

  jq -e \
    '.status == "pass"
      and .stage == "external_beta_trial"
      and .outcome != ""
      and .adoption_summary.status == "pass"
      and .artifacts.issue_body != ""
      and .artifacts.redaction_checklist != ""
      and .artifacts.maintainer_triage != ""' \
    "$OUTPUT_DIR/beta-summary.json" >/dev/null ||
    fail artifact_write "beta summary JSON does not match the external trial contract"

  if [ "$PRINT_SNIPPET" = "1" ]; then
    echo
    echo "external beta trial written to $OUTPUT_DIR"
    echo "issue_body: $OUTPUT_DIR/issue-body.md"
    echo "beta_summary_json: $OUTPUT_DIR/beta-summary.json"
    echo "redaction_checklist: $OUTPUT_DIR/redaction-checklist.md"
    echo "maintainer_triage: $OUTPUT_DIR/maintainer-triage.md"
  fi
}

main "$@"
