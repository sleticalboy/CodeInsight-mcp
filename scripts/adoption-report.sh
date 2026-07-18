#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REPO_ROOT="${CODEINSIGHT_ADOPTION_ROOT:-}"
TASK="${CODEINSIGHT_ADOPTION_TASK:-understand the main application entrypoint}"
TOKEN_BUDGET="${CODEINSIGHT_ADOPTION_TOKEN_BUDGET:-6000}"
OUTPUT_DIR="${CODEINSIGHT_ADOPTION_OUTPUT_DIR:-}"
ARCHIVE_PATH="${CODEINSIGHT_ADOPTION_REPORT_ARCHIVE:-}"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
FORCE_INDEX="${CODEINSIGHT_ADOPTION_FORCE_INDEX:-1}"
PRINT_SNIPPET="${CODEINSIGHT_ADOPTION_PRINT_SNIPPET:-0}"
ADOPTION_EVIDENCE_SCRIPT="${CODEINSIGHT_ADOPTION_EVIDENCE_SCRIPT:-$ROOT_DIR/scripts/adoption-evidence.sh}"

usage() {
  cat <<'EOF'
usage: scripts/adoption-report.sh [REPO_ROOT] [options]

Builds a complete adoption evidence report and packages the copyable issue
template, aggregate summaries, raw route JSON, MCP first-call JSON, and
diagnostic logs into a tar.gz archive.

Options:
  --root PATH           Repository root. Also accepted as the first argument.
  --task TEXT           Task for local evidence and MCP first-call checks.
  --token-budget N      Token budget for context routing. Default: 6000.
  --output-dir PATH     Report output directory. Default: /tmp/codeinsight-adoption-report.
  --archive PATH        Archive path. Default: <output-dir>/codeinsight-adoption-report.tar.gz.
  --bin PATH            Use a specific codeinsight binary.
  --print-snippet       Print a copyable terminal summary after writing files.
  --no-force-index      Reuse the existing index when available.
  -h, --help            Show this help text.

Environment:
  CODEINSIGHT_ADOPTION_ROOT
  CODEINSIGHT_ADOPTION_TASK
  CODEINSIGHT_ADOPTION_TOKEN_BUDGET
  CODEINSIGHT_ADOPTION_OUTPUT_DIR
  CODEINSIGHT_ADOPTION_REPORT_ARCHIVE
  CODEINSIGHT_ADOPTION_FORCE_INDEX
  CODEINSIGHT_ADOPTION_PRINT_SNIPPET
  CODEINSIGHT_ADOPTION_EVIDENCE_SCRIPT
  CODEINSIGHT_BIN
EOF
}

fail() {
  echo "adoption report failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
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
      --output-dir)
        [ "$#" -ge 2 ] || fail "--output-dir requires a path"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --archive)
        [ "$#" -ge 2 ] || fail "--archive requires a path"
        ARCHIVE_PATH="$2"
        shift 2
        ;;
      --bin)
        [ "$#" -ge 2 ] || fail "--bin requires a path"
        CODEINSIGHT_BIN="$2"
        shift 2
        ;;
      --print-snippet)
        PRINT_SNIPPET="1"
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
        fail "unknown argument: $1"
        ;;
      *)
        if [ -n "$REPO_ROOT" ]; then
          fail "unexpected positional argument: $1"
        fi
        REPO_ROOT="$1"
        shift
        ;;
    esac
  done
}

write_manifest() {
  local target="$1"
  shift

  jq -n \
    --arg status "pass" \
    --arg repository "$REPO_ROOT" \
    --arg output_dir "$OUTPUT_DIR" \
    --arg archive "$ARCHIVE_PATH" \
    --argjson files "$(printf '%s\n' "$@" | jq -R . | jq -s .)" \
    '{
      status: $status,
      repository: $repository,
      output_dir: $output_dir,
      archive: $archive,
      files: $files
    }' >"$target"
}

main() {
  parse_args "$@"
  require_command jq
  require_command tar

  if [ -z "$REPO_ROOT" ]; then
    fail "missing repository root"
  fi
  if [ ! -d "$REPO_ROOT" ]; then
    fail "repository root does not exist: $REPO_ROOT"
  fi
  if [ ! -x "$ADOPTION_EVIDENCE_SCRIPT" ]; then
    fail "adoption evidence script is not executable: $ADOPTION_EVIDENCE_SCRIPT"
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
  OUTPUT_DIR="${OUTPUT_DIR:-/tmp/codeinsight-adoption-report}"
  ARCHIVE_PATH="${ARCHIVE_PATH:-$OUTPUT_DIR/codeinsight-adoption-report.tar.gz}"
  mkdir -p "$OUTPUT_DIR" "$(dirname "$ARCHIVE_PATH")"

  local evidence_args
  evidence_args=(
    "$REPO_ROOT"
    "--task"
    "$TASK"
    "--token-budget"
    "$TOKEN_BUDGET"
    "--output-dir"
    "$OUTPUT_DIR"
    "--issue-template"
  )
  if [ -n "$CODEINSIGHT_BIN" ]; then
    evidence_args+=("--bin" "$CODEINSIGHT_BIN")
  fi
  if [ "$PRINT_SNIPPET" = "1" ]; then
    evidence_args+=("--print-snippet")
  fi
  if [ "$FORCE_INDEX" != "1" ]; then
    evidence_args+=("--no-force-index")
  fi

  "$ADOPTION_EVIDENCE_SCRIPT" "${evidence_args[@]}"

  local required_files
  required_files=(
    "adoption-evidence.md"
    "summary.json"
    "issue-template.md"
    "local-repo-evidence.md"
    "local-repo-evidence.json"
    "agent-route.json"
    "mcp-first-call.json"
    "local-repo-evidence.out"
    "local-repo-evidence.err"
    "mcp-first-call.out"
    "mcp-first-call.err"
    "artifact-write.err"
  )

  local relative_file
  for relative_file in "${required_files[@]}"; do
    [ -f "$OUTPUT_DIR/$relative_file" ] ||
      fail "required report file is missing: $OUTPUT_DIR/$relative_file"
  done

  jq -e \
    '.status == "pass"
      and .artifacts.markdown
      and .artifacts.issue_template
      and .artifacts.local_stderr
      and .artifacts.mcp_stderr
      and .mcp_first_call.execution_plan_reads_in_reading_plan_order == true
      and .mcp_first_call.current_step_suggested_tool_matches_reading_plan == true
      and .mcp_first_call.continuation_after_selected_context == true
      and .first_read_gating.suggested_tool_after_selected_context == true
      and .first_read_gating.continuation_after_selected_context == true
      and .first_read_gating.impact_review_before_edits == true' \
    "$OUTPUT_DIR/summary.json" >/dev/null ||
    fail "summary.json does not contain the adoption report artifact contract"

  write_manifest "$OUTPUT_DIR/manifest.json" "${required_files[@]}" "manifest.json"
  required_files+=("manifest.json")

  tar -czf "$ARCHIVE_PATH" -C "$OUTPUT_DIR" "${required_files[@]}"

  echo "adoption report written to $OUTPUT_DIR"
  echo "archive: $ARCHIVE_PATH"
  echo "manifest: $OUTPUT_DIR/manifest.json"
  echo "issue_template: $OUTPUT_DIR/issue-template.md"
}

main "$@"
