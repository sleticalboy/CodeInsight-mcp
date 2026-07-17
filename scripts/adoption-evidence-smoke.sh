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
  echo "adoption evidence smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo/src"
  echo 'export function main() { return "ok"; }' >"$TEMP_DIR/repo/src/main.ts"

  cat >"$TEMP_DIR/local-repo-evidence" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

repo_root="$1"
shift
output=""
raw_json=""
summary_json=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output)
      output="$2"
      shift 2
      ;;
    --json)
      raw_json="$2"
      shift 2
      ;;
    --summary-json)
      summary_json="$2"
      shift 2
      ;;
    --task|--token-budget|--bin)
      shift 2
      ;;
    --no-force-index)
      shift
      ;;
    *)
      shift
      ;;
  esac
done

mkdir -p "$(dirname "$output")" "$(dirname "$raw_json")" "$(dirname "$summary_json")"
cat >"$output" <<'MARKDOWN'
# CodeInsight Local Repository Evidence
MARKDOWN
cat >"$raw_json" <<'JSON'
{"route":[{"tool":"index_project"}]}
JSON
cat >"$summary_json" <<JSON
{
  "status": "pass",
  "repository": "$repo_root",
  "task": "understand the main application entrypoint",
  "token_budget": 6000,
  "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
  "execution_plan_actions": ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"],
  "metrics": {
    "indexed_files": 3,
    "symbols": 8,
    "index_errors": 0,
    "entrypoints": 1,
    "recommended_next_tools": 2,
    "total_lines": 120,
    "selected_lines": 12,
    "line_reduction": "90.0%",
    "selected_files": 1,
    "selected_ranges": 1,
    "estimated_tokens": 180,
    "reading_plan_steps": 1,
    "execution_plan_steps": 4,
    "seed_strategy": "auto_task_match",
    "selected_seed_count": 2,
    "first_seed_source": "task_match",
    "first_seed_value": "src/router.ts",
    "companion_entrypoint": "src/main.ts",
    "first_file": "src/main.ts",
    "first_reading_question": "What setup code defines the main application flow?",
    "first_next_action": "inspect_seed_file",
    "first_suggested_tool": "file_outline",
    "continuation_status": "complete",
    "risk_level": "medium",
    "impacted_files": 2,
    "suggested_checks": 2
  },
  "artifacts": {
    "markdown": "$output",
    "raw_agent_route_json": "$raw_json"
  }
}
JSON
EOF
  chmod +x "$TEMP_DIR/local-repo-evidence"

  cat >"$TEMP_DIR/mcp-first-call-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

summary_json=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --summary-json)
      summary_json="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done

mkdir -p "$(dirname "$summary_json")"
cat >"$summary_json" <<'JSON'
{
  "status": "pass",
  "server": "codeinsight",
  "root": "/tmp/repo",
  "task": "understand the main application entrypoint",
  "token_budget": 6000,
  "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
  "selected_files": ["src/main.ts"],
  "reading_plan": [
    {
      "file": "src/main.ts",
      "question": "What setup code defines the main application flow?",
      "reason": "Read this step first.",
      "selection_reason": "Selected for high relevance",
      "suggested_tool": "file_outline"
    }
  ],
  "execution_plan_actions": ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"],
  "first_execution_action": "read_selected_context",
  "suggested_tool": {
    "tool": "file_outline",
    "arguments": {"path": "src/main.ts"}
  },
  "suggested_tool_executed": true,
  "impact_status": "complete",
  "impact_counts": {
    "impacted_files": 2
  }
}
JSON
EOF
  chmod +x "$TEMP_DIR/mcp-first-call-smoke"

  CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT="$TEMP_DIR/local-repo-evidence" \
  CODEINSIGHT_MCP_FIRST_CALL_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-smoke" \
    "$ROOT_DIR/scripts/adoption-evidence.sh" \
    "$TEMP_DIR/repo" \
    --output-dir "$TEMP_DIR/evidence" \
    --issue-template >"$TEMP_DIR/output.log"

  grep -Fq "adoption evidence written to $TEMP_DIR/evidence" "$TEMP_DIR/output.log" ||
    fail "missing output directory message"
  grep -Fq "issue_template: $TEMP_DIR/evidence/issue-template.md" "$TEMP_DIR/output.log" ||
    fail "missing issue template output path"
  if grep -Fq '# CodeInsight Adoption Evidence' "$TEMP_DIR/output.log"; then
    fail "default run should not print the copyable snippet"
  fi
  grep -Fq '# CodeInsight Adoption Evidence' "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing adoption evidence title"
  grep -Fq -- '- Selected context: `12/120` source lines, `90.0%` reduction' "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing selected context line"
  grep -Fq -- '- Seed strategy: `auto_task_match`' "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing seed strategy line"
  grep -Fq -- '- Selected seeds: `2`' "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing selected seeds line"
  grep -Fq -- '- First seed source: `task_match`' "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing first seed source line"
  grep -Fq -- '- Companion entrypoint: `src/main.ts`' "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing companion entrypoint line"
  grep -Fq -- '- MCP suggested tool executed: `true`' "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing MCP suggested tool execution line"
  grep -Fq -- "- Local evidence stdout: \`$TEMP_DIR/evidence/local-repo-evidence.out\`" "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing local evidence stdout artifact"
  grep -Fq -- "- MCP first-call stderr: \`$TEMP_DIR/evidence/mcp-first-call.err\`" "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing MCP stderr artifact"
  grep -Fq -- "- Issue template: \`$TEMP_DIR/evidence/issue-template.md\`" "$TEMP_DIR/evidence/adoption-evidence.md" ||
    fail "missing issue template artifact"

  jq -e \
    '.status == "pass"
      and .local_evidence.status == "pass"
      and .mcp_first_call.status == "pass"
      and .local_evidence.metrics.line_reduction == "90.0%"
      and .local_evidence.metrics.seed_strategy == "auto_task_match"
      and .local_evidence.metrics.selected_seed_count == 2
      and .local_evidence.metrics.first_seed_source == "task_match"
      and .local_evidence.metrics.companion_entrypoint == "src/main.ts"
      and .mcp_first_call.suggested_tool_executed == true
      and .artifacts.markdown == "'"$TEMP_DIR"'/evidence/adoption-evidence.md"
      and .artifacts.mcp_first_call_json == "'"$TEMP_DIR"'/evidence/mcp-first-call.json"
      and .artifacts.local_stdout == "'"$TEMP_DIR"'/evidence/local-repo-evidence.out"
      and .artifacts.local_stderr == "'"$TEMP_DIR"'/evidence/local-repo-evidence.err"
      and .artifacts.mcp_stdout == "'"$TEMP_DIR"'/evidence/mcp-first-call.out"
      and .artifacts.mcp_stderr == "'"$TEMP_DIR"'/evidence/mcp-first-call.err"
      and .artifacts.artifact_stderr == "'"$TEMP_DIR"'/evidence/artifact-write.err"
      and .artifacts.issue_template == "'"$TEMP_DIR"'/evidence/issue-template.md"' \
    "$TEMP_DIR/evidence/summary.json" >/dev/null ||
    fail "aggregate summary JSON does not match expected contract"
  test -f "$TEMP_DIR/evidence/local-repo-evidence.out" ||
    fail "local evidence stdout file is missing"
  test -f "$TEMP_DIR/evidence/mcp-first-call.err" ||
    fail "MCP first-call stderr file is missing"
  test -f "$TEMP_DIR/evidence/artifact-write.err" ||
    fail "artifact write stderr file is missing"
  test -f "$TEMP_DIR/evidence/issue-template.md" ||
    fail "issue template file is missing"
  grep -Fq '# CodeInsight Adoption Evidence Issue' "$TEMP_DIR/evidence/issue-template.md" ||
    fail "missing issue template title"
  grep -Fq '## Failure Category' "$TEMP_DIR/evidence/issue-template.md" ||
    fail "missing issue template failure category section"
  grep -Fq 'adoption evidence failed [usage|prerequisite|local_cli_route|mcp_first_call|artifact_write]: ...' "$TEMP_DIR/evidence/issue-template.md" ||
    fail "missing issue template failure category placeholder"
  grep -Fq '## Artifacts' "$TEMP_DIR/evidence/issue-template.md" ||
    fail "missing issue template artifacts section"
  grep -Fq -- "- MCP first-call stderr: \`$TEMP_DIR/evidence/mcp-first-call.err\`" "$TEMP_DIR/evidence/issue-template.md" ||
    fail "missing issue template MCP stderr artifact"
  grep -Fq '## Environment' "$TEMP_DIR/evidence/issue-template.md" ||
    fail "missing issue template environment section"
  grep -Fq -- '- CodeInsight version:' "$TEMP_DIR/evidence/issue-template.md" ||
    fail "missing issue template CodeInsight version placeholder"

  CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT="$TEMP_DIR/local-repo-evidence" \
  CODEINSIGHT_MCP_FIRST_CALL_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-smoke" \
    "$ROOT_DIR/scripts/adoption-evidence.sh" \
    "$TEMP_DIR/repo" \
    --output-dir "$TEMP_DIR/evidence-with-snippet" \
    --print-snippet >"$TEMP_DIR/snippet.log"

  grep -Fq '# CodeInsight Adoption Evidence' "$TEMP_DIR/snippet.log" ||
    fail "missing printed snippet title"
  grep -Fq -- '- Selected context: `12/120` source lines, `90.0%` reduction' "$TEMP_DIR/snippet.log" ||
    fail "missing printed selected context line"
  grep -Fq -- '- Companion entrypoint: `src/main.ts`' "$TEMP_DIR/snippet.log" ||
    fail "missing printed companion entrypoint line"
  grep -Fq -- '- MCP suggested tool executed: `true`' "$TEMP_DIR/snippet.log" ||
    fail "missing printed MCP suggested tool execution line"

  cat >"$TEMP_DIR/local-repo-evidence-fail" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "simulated local route failure" >&2
exit 42
EOF
  chmod +x "$TEMP_DIR/local-repo-evidence-fail"

  if CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT="$TEMP_DIR/local-repo-evidence-fail" \
    CODEINSIGHT_MCP_FIRST_CALL_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-smoke" \
    "$ROOT_DIR/scripts/adoption-evidence.sh" \
      "$TEMP_DIR/repo" \
      --output-dir "$TEMP_DIR/evidence-local-fail" >"$TEMP_DIR/local-fail.out" 2>"$TEMP_DIR/local-fail.err"; then
    fail "local route failure should fail adoption evidence"
  fi
  grep -Fq 'adoption evidence failed [local_cli_route]: local first-read evidence generation failed' "$TEMP_DIR/local-fail.err" ||
    fail "missing local route failure category"
  grep -Fq 'simulated local route failure' "$TEMP_DIR/local-fail.err" ||
    fail "missing local route failure stderr"

  cat >"$TEMP_DIR/mcp-first-call-smoke-fail" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
echo "simulated MCP first-call failure" >&2
exit 43
EOF
  chmod +x "$TEMP_DIR/mcp-first-call-smoke-fail"

  if CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT="$TEMP_DIR/local-repo-evidence" \
    CODEINSIGHT_MCP_FIRST_CALL_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-smoke-fail" \
    "$ROOT_DIR/scripts/adoption-evidence.sh" \
      "$TEMP_DIR/repo" \
      --output-dir "$TEMP_DIR/evidence-mcp-fail" >"$TEMP_DIR/mcp-fail.out" 2>"$TEMP_DIR/mcp-fail.err"; then
    fail "MCP first-call failure should fail adoption evidence"
  fi
  grep -Fq 'adoption evidence failed [mcp_first_call]: MCP first-call verification failed' "$TEMP_DIR/mcp-fail.err" ||
    fail "missing MCP first-call failure category"
  grep -Fq 'simulated MCP first-call failure' "$TEMP_DIR/mcp-fail.err" ||
    fail "missing MCP first-call failure stderr"

  echo "adoption evidence smoke passed"
}

main "$@"
