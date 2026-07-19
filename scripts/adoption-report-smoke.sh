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
  echo "adoption report smoke failed: $*" >&2
  exit 1
}

require_tar_entry() {
  local archive="$1"
  local entry="$2"

  tar -tzf "$archive" | grep -Fxq "$entry" ||
    fail "archive is missing $entry"
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo/src"
  echo 'export function main() { return "ok"; }' >"$TEMP_DIR/repo/src/main.ts"

  cat >"$TEMP_DIR/adoption-evidence" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

repo_root="$1"
shift
output_dir=""
task=""
token_budget=""
issue_template="0"
print_snippet="0"
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-dir)
      output_dir="$2"
      shift 2
      ;;
    --task)
      task="$2"
      shift 2
      ;;
    --token-budget)
      token_budget="$2"
      shift 2
      ;;
    --issue-template)
      issue_template="1"
      shift
      ;;
    --print-snippet)
      print_snippet="1"
      shift
      ;;
    --bin)
      shift 2
      ;;
    --no-force-index)
      shift
      ;;
    *)
      echo "unexpected argument: $1" >&2
      exit 2
      ;;
  esac
done

[ "$issue_template" = "1" ] || {
  echo "missing --issue-template" >&2
  exit 3
}
[ -n "$output_dir" ] || {
  echo "missing --output-dir" >&2
  exit 4
}

mkdir -p "$output_dir"
printf '%s\n' "$repo_root" >"$output_dir/repo-root.txt"
printf '%s\n' "$task" >"$output_dir/task.txt"
printf '%s\n' "$token_budget" >"$output_dir/token-budget.txt"

cat >"$output_dir/adoption-evidence.md" <<'MARKDOWN'
# CodeInsight Adoption Evidence
MARKDOWN
cat >"$output_dir/issue-template.md" <<'MARKDOWN'
# CodeInsight Adoption Evidence Issue

## Failure Category

adoption evidence failed [usage|prerequisite|local_cli_route|mcp_first_call|artifact_write]: ...
MARKDOWN
cat >"$output_dir/local-repo-evidence.md" <<'MARKDOWN'
# CodeInsight Local Repository Evidence
MARKDOWN
cat >"$output_dir/local-repo-evidence.json" <<'JSON'
{"status":"pass"}
JSON
cat >"$output_dir/agent-route.json" <<'JSON'
{"route":[{"tool":"index_project"}]}
JSON
cat >"$output_dir/mcp-first-call.json" <<'JSON'
{"status":"pass"}
JSON
: >"$output_dir/local-repo-evidence.out"
: >"$output_dir/local-repo-evidence.err"
: >"$output_dir/mcp-first-call.out"
: >"$output_dir/mcp-first-call.err"
: >"$output_dir/artifact-write.err"
cat >"$output_dir/summary.json" <<JSON
{
  "status": "pass",
  "repository": "$repo_root",
  "task": "$task",
  "mcp_first_call": {
    "execution_plan_reads_in_reading_plan_order": true,
    "current_reading_step_matches_reading_plan": true,
    "current_step_suggested_tool_matches_reading_plan": true,
    "continuation_after_selected_context": true
  },
  "first_read_gating": {
    "suggested_tool_after_selected_context": true,
    "continuation_after_selected_context": true,
    "impact_review_before_edits": true
  },
  "artifacts": {
    "markdown": "$output_dir/adoption-evidence.md",
    "issue_template": "$output_dir/issue-template.md",
    "local_stderr": "$output_dir/local-repo-evidence.err",
    "mcp_stderr": "$output_dir/mcp-first-call.err"
  }
}
JSON

echo "adoption evidence written to $output_dir"
if [ "$print_snippet" = "1" ]; then
  echo "# CodeInsight Adoption Evidence"
fi
EOF
  chmod +x "$TEMP_DIR/adoption-evidence"

  CODEINSIGHT_ADOPTION_EVIDENCE_SCRIPT="$TEMP_DIR/adoption-evidence" \
    "$ROOT_DIR/scripts/adoption-report.sh" \
    "$TEMP_DIR/repo" \
    --task "understand the main application entrypoint" \
    --token-budget 6000 \
    --output-dir "$TEMP_DIR/report" \
    --archive "$TEMP_DIR/codeinsight-adoption-report.tar.gz" \
    --print-snippet >"$TEMP_DIR/output.log"

  grep -Fq "adoption report written to $TEMP_DIR/report" "$TEMP_DIR/output.log" ||
    fail "missing report output directory message"
  grep -Fq "archive: $TEMP_DIR/codeinsight-adoption-report.tar.gz" "$TEMP_DIR/output.log" ||
    fail "missing archive output message"
  grep -Fq "issue_template: $TEMP_DIR/report/issue-template.md" "$TEMP_DIR/output.log" ||
    fail "missing issue template output message"
  grep -Fq '# CodeInsight Adoption Evidence' "$TEMP_DIR/output.log" ||
    fail "missing forwarded print snippet output"

  test -f "$TEMP_DIR/codeinsight-adoption-report.tar.gz" ||
    fail "archive file is missing"
  test -f "$TEMP_DIR/report/manifest.json" ||
    fail "manifest file is missing"

  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "adoption-evidence.md"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "summary.json"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "issue-template.md"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "local-repo-evidence.md"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "local-repo-evidence.json"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "agent-route.json"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "mcp-first-call.json"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "local-repo-evidence.out"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "local-repo-evidence.err"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "mcp-first-call.out"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "mcp-first-call.err"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "artifact-write.err"
  require_tar_entry "$TEMP_DIR/codeinsight-adoption-report.tar.gz" "manifest.json"

  mkdir -p "$TEMP_DIR/extracted"
  tar -xzf "$TEMP_DIR/codeinsight-adoption-report.tar.gz" -C "$TEMP_DIR/extracted"

  grep -Fq '# CodeInsight Adoption Evidence Issue' "$TEMP_DIR/extracted/issue-template.md" ||
    fail "extracted issue template is missing title"
  jq -e \
    '.status == "pass"
      and .archive == "'"$TEMP_DIR"'/codeinsight-adoption-report.tar.gz"
      and (.files | index("issue-template.md"))
      and (.files | index("summary.json"))
      and (.files | index("artifact-write.err"))' \
    "$TEMP_DIR/extracted/manifest.json" >/dev/null ||
    fail "extracted manifest JSON does not match expected contract"
  jq -e \
    '.status == "pass"
      and .mcp_first_call.execution_plan_reads_in_reading_plan_order == true
      and .mcp_first_call.current_reading_step_matches_reading_plan == true
      and .mcp_first_call.current_step_suggested_tool_matches_reading_plan == true
      and .mcp_first_call.continuation_after_selected_context == true
      and .first_read_gating.suggested_tool_after_selected_context == true
      and .first_read_gating.continuation_after_selected_context == true
      and .first_read_gating.impact_review_before_edits == true
      and .artifacts.issue_template == "'"$TEMP_DIR"'/report/issue-template.md"
      and .artifacts.local_stderr == "'"$TEMP_DIR"'/report/local-repo-evidence.err"
      and .artifacts.mcp_stderr == "'"$TEMP_DIR"'/report/mcp-first-call.err"' \
    "$TEMP_DIR/extracted/summary.json" >/dev/null ||
    fail "extracted summary JSON does not preserve artifact paths"

  echo "adoption report smoke passed"
}

main "$@"
