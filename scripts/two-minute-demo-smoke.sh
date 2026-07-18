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
  echo "two-minute demo smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  cat >"$TEMP_DIR/codeinsight" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [ "${1:-}" != "agent-route" ]; then
  echo "unexpected command: $*" >&2
  exit 1
fi

cat <<'JSON'
{
  "root": "/tmp/demo",
  "task": "understand agent context routing",
  "token_budget": 6000,
  "route": [
    {"order": 1, "tool": "index_project", "status": "complete", "reason": "indexed"},
    {"order": 2, "tool": "project_overview", "status": "complete", "reason": "overview"},
    {"order": 3, "tool": "context_pack", "status": "complete", "reason": "selected 1 files, 11 ranges, and 1 reading-plan steps within the token budget; read src/main.rs first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; no omitted candidate follow-up is needed before the selected context is read; continuation read_selected_context"},
    {"order": 4, "tool": "impact_analysis", "status": "complete", "reason": "after selected context is read, pre-edit impact check estimated 11 impacted files at high risk, including 3 call-related files, 2 dependency-related files, 4 call paths, and 5 dependency paths"}
  ],
  "execution_plan": [
    {
      "order": 1,
      "action": "read_selected_context",
      "status": "ready",
      "instruction": "Read context_pack.files[] in reading_plan[] order, starting with src/main.rs (candidate rank 1).",
      "files": ["src/main.rs"]
    },
    {
      "order": 2,
      "action": "use_current_reading_step_suggested_tool",
      "status": "available_after_current_file",
      "instruction": "After reading src/main.rs, call file_outline only if deeper evidence is needed.",
      "files": ["src/main.rs"],
      "suggested_tool": {"tool": "file_outline"}
    },
    {
      "order": 3,
      "action": "use_continuation_if_needed",
      "status": "complete",
      "instruction": "Use continuation_summary only after selected context has been read."
    },
    {
      "order": 4,
      "action": "review_impact_before_edits",
      "status": "complete",
      "instruction": "Before editing, review impact_analysis."
    }
  ],
  "impact_seed_files": ["src/main.rs"],
  "impact_seed_symbols": [],
  "impact_status": "complete",
  "index_report": {
    "root": "/tmp/demo",
    "schema_version": 1,
    "index_version": "0.0.0",
    "indexed_files": 23,
    "changed_files": 23,
    "unchanged_files": 0,
    "deleted_files": 0,
    "skipped_files": 0,
    "symbols": 918,
    "changed_symbols": 918,
    "errors": [],
    "duration_ms": 300
  },
  "overview": {
    "root": "/tmp/demo",
    "indexed_files": 23,
    "total_lines": 27681,
    "symbols": 918,
    "dependencies": 0,
    "call_edges": 0,
    "summary": "demo",
    "languages": [],
    "top_directories": [],
    "main_directories": [],
    "symbol_kinds": [],
    "dependency_summary": {},
    "call_summary": {},
    "entrypoints": [{"file": "src/main.rs"}],
    "recommended_next_tools": [{}, {}, {}, {}],
    "index_status": {}
  },
  "context_pack": {
    "task": "understand agent context routing",
    "summary": "demo",
    "seed_strategy": "auto_entrypoint",
    "selected_seeds": [],
    "reading_plan": [
      {
        "order": 1,
        "file": "src/main.rs",
        "next_action": "inspect_seed_file",
        "question": "What entrypoints or setup code define the main flow here?",
        "reason": "Read this step to answer: What entrypoints or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/main.rs",
        "selection_rank": 1,
        "selection_reason": "Selected for high relevance via seed_file: Seed file header and imports for task: src/main.rs",
        "suggested_tool": {"tool": "file_outline"}
      }
    ],
    "semantic_status": {},
    "budget": {
      "requested_token_budget": 6000,
      "applied_token_budget": 6000,
      "estimated_tokens": 4372,
      "candidate_files": 10,
      "selected_files": 10,
      "omitted_files": 0,
      "candidate_ranges": 11,
      "selected_ranges": 11,
      "omitted_ranges": 0,
      "truncated": false,
      "truncation_reason": "complete"
    },
    "continuation_summary": {"status": "complete", "next_action": "read_selected_context"},
    "omitted_candidates": [],
    "files": [
      {
        "file": "src/main.rs",
        "ranges": [
          {"start_line": 1, "end_line": 439}
        ]
      }
    ],
    "symbols": [],
    "references": [],
    "estimated_tokens": 4372,
    "truncated": false
  },
  "impact_analysis": {
    "risk_level": "high",
    "impact_counts": {
      "impacted_files": 11,
      "paths": 0
    },
    "suggested_checks": [{}, {}, {}]
  }
}
JSON
EOF
  chmod +x "$TEMP_DIR/codeinsight"

  CODEINSIGHT_BIN="$TEMP_DIR/codeinsight" \
  CODEINSIGHT_DEMO_SAVE_JSON="$TEMP_DIR/agent-route.json" \
    "$ROOT_DIR/scripts/two-minute-demo.sh" >"$TEMP_DIR/output.log"

  grep -Fq 'Problem: AI agents waste the first read' "$TEMP_DIR/output.log" ||
    fail "missing problem statement"
  grep -Fq 'Promise: route the agent through agent_route before edits.' "$TEMP_DIR/output.log" ||
    fail "missing product promise"
  grep -Fq 'CodeInsight agent_route demo' "$TEMP_DIR/output.log" ||
    fail "missing agent_route live heading"
  grep -Fq 'agent_route ran index_project, project_overview, context_pack, and impact_analysis in one call.' "$TEMP_DIR/output.log" ||
    fail "missing agent_route talk track"
  grep -Fq 'project_overview found 1 entrypoints and 4 recommended next tools.' "$TEMP_DIR/output.log" ||
    fail "missing overview talk track"
  grep -Fq 'context_pack selected 1 files and 11 ranges, then produced 1 reading-plan steps.' "$TEMP_DIR/output.log" ||
    fail "missing context_pack talk track"
  grep -Fq 'execution_plan_steps: 4' "$TEMP_DIR/output.log" ||
    fail "missing execution plan steps metric"
  grep -Fq 'first_execution_action: read_selected_context' "$TEMP_DIR/output.log" ||
    fail "missing first execution action metric"
  grep -Fq 'second_execution_action: use_current_reading_step_suggested_tool' "$TEMP_DIR/output.log" ||
    fail "missing second execution action metric"
  grep -Fq 'first_execution_suggested_tool: file_outline' "$TEMP_DIR/output.log" ||
    fail "missing first execution suggested tool metric"
  grep -Fq 'first_reading_question: What entrypoints or setup code define the main flow here?' "$TEMP_DIR/output.log" ||
    fail "missing first reading question metric"
  grep -Fq 'first_selection_rank: 1' "$TEMP_DIR/output.log" ||
    fail "missing first selection rank metric"
  grep -Fq 'first_reading_file: src/main.rs' "$TEMP_DIR/output.log" ||
    fail "missing first reading file metric"
  grep -Fq 'reading_order_contract: true' "$TEMP_DIR/output.log" ||
    fail "missing reading order contract metric"
  grep -Fq 'suggested_tool_handoff_contract: true' "$TEMP_DIR/output.log" ||
    fail "missing suggested tool handoff contract metric"
  grep -Fq 'continuation_timing_contract: true' "$TEMP_DIR/output.log" ||
    fail "missing continuation timing contract metric"
  grep -Fq 'continuation_next_action: read_selected_context' "$TEMP_DIR/output.log" ||
    fail "missing continuation next action metric"
  grep -Fq 'first_omitted_candidate: none' "$TEMP_DIR/output.log" ||
    fail "missing no omitted candidate metric"
  grep -Fq 'route_reason: selected 1 files, 11 ranges, and 1 reading-plan steps within the token budget; read src/main.rs first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; no omitted candidate follow-up is needed before the selected context is read; continuation read_selected_context' "$TEMP_DIR/output.log" ||
    fail "missing context route reason"
  grep -Fq 'reading_plan_reason: Read this step to answer: What entrypoints or setup code define the main flow here? If deeper evidence is needed, call file_outline. Selection reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/main.rs' "$TEMP_DIR/output.log" ||
    fail "missing reading plan reason"
  grep -Fq 'selection_reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/main.rs' "$TEMP_DIR/output.log" ||
    fail "missing selection reason"
  grep -Fq 'selected context reduced source reading by' "$TEMP_DIR/output.log" ||
    fail "missing line reduction talk track"
  grep -Fq '[Evidence summary]' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary"
  grep -Fq 'agent_route selected 439/27681 source lines (98.4% reduction) across 1 files.' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary line reduction"
  grep -Fq 'First reading question: What entrypoints or setup code define the main flow here?' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary reading question"
  grep -Fq 'The first selected file is src/main.rs; reading_plan starts at src/main.rs as candidate rank 1.' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary first reading file"
  grep -Fq 'Execution contract: reading_order=true, suggested_tool_handoff=true, continuation_after_selected_context=true.' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary execution contract"
  grep -Fq 'Selection evidence: Selected for high relevance via seed_file: Seed file header and imports for task: src/main.rs' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary selection evidence"
  grep -Fq 'Continuation: status=complete, next_action=read_selected_context.' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary continuation"
  grep -Fq 'Next follow-up candidate: none before selected context is read.' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary omitted candidate"
  grep -Fq 'Read src/main.rs before offering file_outline.' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary suggested tool timing"
  grep -Fq 'Before edits, impact_analysis reports high risk across 11 impacted files.' "$TEMP_DIR/output.log" ||
    fail "missing evidence summary impact check"
  grep -Fq 'execution_plan starts with read_selected_context, then use_current_reading_step_suggested_tool; this keeps suggested tools behind selected-context reading.' "$TEMP_DIR/output.log" ||
    fail "missing execution plan talk track"
  grep -Fq 'The first execution-plan suggested tool is file_outline; offer it only after the selected file has been read.' "$TEMP_DIR/output.log" ||
    fail "missing execution suggested tool talk track"
  grep -Fq 'The first reading-plan question is: What entrypoints or setup code define the main flow here?' "$TEMP_DIR/output.log" ||
    fail "missing reading question talk track"
  grep -Fq 'The first reading-plan action is inspect_seed_file; Read this step to answer: What entrypoints or setup code define the main flow here?' "$TEMP_DIR/output.log" ||
    fail "missing reading reason talk track"
  grep -Fq 'Reading order contract is true; execution_plan[0].files follows reading_plan[] order.' "$TEMP_DIR/output.log" ||
    fail "missing reading order contract talk track"
  grep -Fq 'Suggested-tool handoff contract is true; execution_plan[1] points to the current reading step.' "$TEMP_DIR/output.log" ||
    fail "missing suggested tool handoff contract talk track"
  grep -Fq 'Continuation timing contract is true; continuation is only considered after selected context is read.' "$TEMP_DIR/output.log" ||
    fail "missing continuation timing contract talk track"
  grep -Fq 'Selection evidence: candidate rank 1; Selected for high relevance via seed_file: Seed file header and imports for task: src/main.rs' "$TEMP_DIR/output.log" ||
    fail "missing selection evidence talk track"
  grep -Fq 'Continuation status is complete; next_action=read_selected_context, so no omitted candidate follow-up is needed before selected context is read.' "$TEMP_DIR/output.log" ||
    fail "missing continuation next action talk track"
  grep -Fq 'pre-edit impact check estimated 11 impacted files at high risk' "$TEMP_DIR/output.log" ||
    fail "missing impact route reason"
  grep -Fq 'impact_analysis reports high risk across 11 impacted files with 3 suggested checks' "$TEMP_DIR/output.log" ||
    fail "missing impact_analysis talk track"
  grep -Fq 'Call agent_route with root, task, and token_budget for the default first read.' "$TEMP_DIR/output.log" ||
    fail "missing agent policy"
  grep -Fq "raw_agent_route_json: $TEMP_DIR/agent-route.json" "$TEMP_DIR/output.log" ||
    fail "missing saved JSON path"

  if [ ! -s "$TEMP_DIR/agent-route.json" ]; then
    fail "saved JSON file is missing"
  fi
  jq -e '.route[0].tool == "index_project" and .context_pack.reading_plan[0].file == "src/main.rs"' \
    "$TEMP_DIR/agent-route.json" >/dev/null ||
    fail "saved JSON file does not contain the raw agent_route payload"

  echo "two-minute demo smoke passed"
}

main "$@"
