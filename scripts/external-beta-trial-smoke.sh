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
  echo "external beta trial smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo/src"
  echo 'export function routes() { return "ok"; }' >"$TEMP_DIR/repo/src/router.ts"

  cat >"$TEMP_DIR/adoption-evidence" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

repo_root="$1"
shift
output_dir=""
task="understand the main application entrypoint"
saw_file=0
saw_symbol=0
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
    --file)
      [ "${2:-}" = "src/router.ts" ] || {
        echo "unexpected seed file: ${2:-}" >&2
        exit 1
      }
      saw_file=1
      shift 2
      ;;
    --symbol)
      [ "${2:-}" = "routes" ] || {
        echo "unexpected seed symbol: ${2:-}" >&2
        exit 1
      }
      saw_symbol=1
      shift 2
      ;;
    --token-budget|--bin)
      shift 2
      ;;
    --print-snippet|--issue-template|--no-force-index)
      shift
      ;;
    *)
      shift
      ;;
  esac
done
if [ "${EXPECT_SEEDS:-0}" = "1" ]; then
  [ "$saw_file" -eq 1 ] || {
    echo "missing seed file" >&2
    exit 1
  }
  [ "$saw_symbol" -eq 1 ] || {
    echo "missing seed symbol" >&2
    exit 1
  }
fi

mkdir -p "$output_dir"
cat >"$output_dir/adoption-evidence.md" <<'MARKDOWN'
# CodeInsight Adoption Evidence
MARKDOWN
cat >"$output_dir/agent-route.json" <<'JSON'
{"route":[{"tool":"index_project"}]}
JSON
cat >"$output_dir/mcp-first-call.json" <<'JSON'
{
  "status": "pass",
  "suggested_tool_executed": true
}
JSON
cat >"$output_dir/summary.json" <<JSON
{
  "status": "pass",
  "repository": "$repo_root",
  "task": "$task",
  "local_evidence": {
    "status": "pass",
    "metrics": {
      "first_file": "src/router.ts",
      "first_reading_focus": "Start with route registration boundaries.",
      "first_reading_question": "Which code registers and dispatches routes?",
      "first_suggested_tool": "file_outline",
      "selected_lines": 24,
      "total_lines": 240,
      "source_lines_avoided": 216,
      "line_reduction": "90.0%",
      "read_less_ratio": "10.0x",
      "risk_level": "medium",
      "suggested_checks": 2
    }
  },
  "mcp_first_call": {
    "status": "pass",
    "route_quality": {
      "level": "high",
      "score": 96,
      "evidence_count": 4,
      "recommended_action": "read_selected_context"
    },
    "suggested_tool_executed": true
  },
  "first_read_gating": {
    "suggested_tool_after_selected_context": true,
    "continuation_after_selected_context": true,
    "impact_review_before_edits": true
  },
  "artifacts": {
    "markdown": "$output_dir/adoption-evidence.md",
    "raw_agent_route_json": "$output_dir/agent-route.json",
    "mcp_first_call_json": "$output_dir/mcp-first-call.json",
    "local_stderr": "$output_dir/local-repo-evidence.err",
    "mcp_stderr": "$output_dir/mcp-first-call.err",
    "artifact_stderr": "$output_dir/artifact-write.err"
  }
}
JSON
touch "$output_dir/local-repo-evidence.err" "$output_dir/mcp-first-call.err" "$output_dir/artifact-write.err"
echo "adoption evidence written to $output_dir"
EOF
  chmod +x "$TEMP_DIR/adoption-evidence"

  EXPECT_SEEDS=1 \
  CODEINSIGHT_ADOPTION_EVIDENCE_SCRIPT="$TEMP_DIR/adoption-evidence" \
    "$ROOT_DIR/scripts/external-beta-trial.sh" \
      "$TEMP_DIR/repo" \
      --task "understand route registration behavior" \
      --file src/router.ts \
      --symbol routes \
      --repo-url "https://example.com/demo/repo" \
      --expected-first-read "src/router.ts or route registration package" \
      --install-method "Source" \
      --mcp-client "Codex" \
      --version "codeinsight 0.1.0 smoke" \
      --outcome needs_triage \
      --output-dir "$TEMP_DIR/beta" \
      --no-print-snippet >"$TEMP_DIR/output.log"

  if grep -Fq "external beta trial written to" "$TEMP_DIR/output.log"; then
    fail "no-print-snippet should suppress final helper snippet"
  fi

  test -f "$TEMP_DIR/beta/issue-body.md" ||
    fail "issue body is missing"
  test -f "$TEMP_DIR/beta/redaction-checklist.md" ||
    fail "redaction checklist is missing"
  test -f "$TEMP_DIR/beta/maintainer-triage.md" ||
    fail "maintainer triage note is missing"
  test -f "$TEMP_DIR/beta/beta-summary.json" ||
    fail "beta summary JSON is missing"

  grep -Fq '# External Beta Trial' "$TEMP_DIR/beta/issue-body.md" ||
    fail "issue body is missing title"
  grep -Fq -- '- Outcome: `needs_triage`' "$TEMP_DIR/beta/issue-body.md" ||
    fail "issue body is missing needs_triage outcome"
  grep -Fq -- '- First selected file: `src/router.ts`' "$TEMP_DIR/beta/issue-body.md" ||
    fail "issue body is missing first selected file"
  grep -Fq -- '- MCP route quality: `high` (`96/100`, `4` evidence signals), next=`read_selected_context`' "$TEMP_DIR/beta/issue-body.md" ||
    fail "issue body is missing MCP route quality"
  grep -Fq -- '  --file "src/router.ts" \' "$TEMP_DIR/beta/issue-body.md" ||
    fail "issue body reproduction command is missing seed file"
  grep -Fq -- '  --symbol "routes" \' "$TEMP_DIR/beta/issue-body.md" ||
    fail "issue body reproduction command is missing seed symbol"
  grep -Fq 'Private/redacted repository: no' "$TEMP_DIR/beta/issue-body.md" ||
    fail "issue body should mark public trial"
  grep -Fq 'Replace private absolute paths with repository-relative paths.' "$TEMP_DIR/beta/redaction-checklist.md" ||
    fail "redaction checklist is missing path guidance"
  grep -Fq 'reclassify as `route-hit`, `route-near-miss`, `route-miss`, `workflow-friction`, or `overtrust-risk`' "$TEMP_DIR/beta/maintainer-triage.md" ||
    fail "maintainer triage note is missing reclassification guidance"
  grep -Fq -- '- MCP route quality: `high` (`96/100`), next=`read_selected_context`' "$TEMP_DIR/beta/maintainer-triage.md" ||
    fail "maintainer triage note is missing MCP route quality"

  jq -e \
    '.status == "pass"
      and .stage == "external_beta_trial"
      and .repository_url == "https://example.com/demo/repo"
      and .task == "understand route registration behavior"
      and .expected_first_read == "src/router.ts or route registration package"
      and .install_method == "Source"
      and .mcp_client == "Codex"
      and .codeinsight_version == "codeinsight 0.1.0 smoke"
      and .outcome == "needs_triage"
      and .private_repo == false
      and .adoption_summary.status == "pass"
      and .adoption_summary.mcp_first_call.route_quality.level == "high"
      and .adoption_summary.mcp_first_call.route_quality.score == 96
      and .adoption_summary.mcp_first_call.route_quality.evidence_count == 4
      and .adoption_summary.mcp_first_call.route_quality.recommended_action == "read_selected_context"
      and .artifacts.issue_body == "'"$TEMP_DIR"'/beta/issue-body.md"
      and .artifacts.redaction_checklist == "'"$TEMP_DIR"'/beta/redaction-checklist.md"
      and .artifacts.maintainer_triage == "'"$TEMP_DIR"'/beta/maintainer-triage.md"' \
    "$TEMP_DIR/beta/beta-summary.json" >/dev/null ||
    fail "beta summary JSON does not match expected contract"

  CODEINSIGHT_ADOPTION_EVIDENCE_SCRIPT="$TEMP_DIR/adoption-evidence" \
    "$ROOT_DIR/scripts/external-beta-trial.sh" \
      "$TEMP_DIR/repo" \
      --task "understand private route behavior" \
      --private-repo \
      --outcome route_miss \
      --output-dir "$TEMP_DIR/private-beta" >"$TEMP_DIR/private-output.log"

  grep -Fq "external beta trial written to $TEMP_DIR/private-beta" "$TEMP_DIR/private-output.log" ||
    fail "default run should print output directory"
  grep -Fq 'Private/redacted repository: yes, paths may be redacted' "$TEMP_DIR/private-beta/issue-body.md" ||
    fail "private issue body should mark redaction"
  jq -e '.private_repo == true and .outcome == "route_miss"' \
    "$TEMP_DIR/private-beta/beta-summary.json" >/dev/null ||
    fail "private beta summary should record private repo and route miss"

  if "$ROOT_DIR/scripts/external-beta-trial.sh" "$TEMP_DIR/repo" --outcome bad_value >"$TEMP_DIR/bad.out" 2>"$TEMP_DIR/bad.err"; then
    fail "invalid outcome should fail"
  fi
  grep -Fq 'external beta trial failed [usage]: unknown outcome: bad_value' "$TEMP_DIR/bad.err" ||
    fail "invalid outcome error is missing category"

  echo "external beta trial smoke passed"
}

main "$@"
