#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

fail() {
  echo "agent-route step summary smoke failed: $*" >&2
  exit 1
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

require_literal() {
  local file="$1"
  local literal="$2"
  local description="$3"

  if ! grep -Fq -- "$literal" "$file"; then
    fail "$file is missing $description"
  fi
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local summary_json="$TEMP_DIR/agent-route.json"
  local summary_md="$TEMP_DIR/summary.md"

  cat >"$summary_json" <<'EOF'
{
  "status": "pass",
  "task": "understand auth entrypoint flow",
  "token_budget": 1600,
  "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
  "execution_plan_actions": ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"],
  "metrics": {
    "indexed_files": 3,
    "symbols": 5,
    "index_errors": 0,
    "entrypoints": 1,
    "selected_files": 2,
    "selected_ranges": 2,
    "reading_plan_steps": 2,
    "execution_plan_steps": 4,
    "requested_token_budget": 1600,
    "applied_token_budget": 1600,
    "seed_strategy": "auto_task_match",
    "selected_seed_count": 2,
    "first_seed_source": "task_match",
    "first_seed_value": "src/router.ts",
    "companion_entrypoint": "src/main.ts",
    "first_context_file": "src/main.ts",
    "first_reading_file": "src/main.ts",
    "first_execution_action": "read_selected_context",
    "second_execution_action": "use_current_reading_step_suggested_tool",
    "first_execution_suggested_tool": "file_outline",
    "first_next_action": "inspect_seed_file",
    "first_reading_question": "What entrypoints define the main flow?",
    "context_route_reason": "selected 2 files, 2 ranges, and 2 reading-plan steps within the token budget; read src/main.ts first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; no omitted candidate follow-up is needed before the selected context is read; continuation read_selected_context",
    "impact_route_reason": "after selected context is read, pre-edit impact check estimated 2 impacted files at medium risk, including 1 call-related files, 1 dependency-related files, 1 call paths, and 1 dependency paths",
    "impact_status": "complete",
    "impacted_files": 2,
    "suggested_checks": 3
  }
}
EOF

  GITHUB_STEP_SUMMARY="$summary_md" \
    "$ROOT_DIR/scripts/agent-route-step-summary.sh" \
      "$summary_json" \
      codeinsight-agent-route-smoke \
      https://example.com/artifact \
      https://example.com/run >/dev/null

  require_literal "$summary_md" "## Agent Route Smoke" "summary heading"
  require_literal "$summary_md" 'Route: `index_project -> project_overview -> context_pack -> impact_analysis`' "route line"
  require_literal "$summary_md" 'Execution plan: `read_selected_context -> use_current_reading_step_suggested_tool -> use_continuation_if_needed -> review_impact_before_edits`' "execution plan line"
  require_literal "$summary_md" 'Workflow artifact: [`codeinsight-agent-route-smoke`](https://example.com/artifact)' "artifact link"
  require_literal "$summary_md" '| Indexed files | `3` |' "indexed files metric"
  require_literal "$summary_md" '| Execution-plan steps | `4` |' "execution plan steps metric"
  require_literal "$summary_md" '| Seed strategy | `auto_task_match` |' "seed strategy metric"
  require_literal "$summary_md" '| Selected seeds | `2` |' "selected seeds metric"
  require_literal "$summary_md" '| First seed source | `task_match` |' "first seed source metric"
  require_literal "$summary_md" '| First seed value | `src/router.ts` |' "first seed value metric"
  require_literal "$summary_md" '| Companion entrypoint | `src/main.ts` |' "companion entrypoint metric"
  require_literal "$summary_md" '| First reading file | `src/main.ts` |' "first reading file metric"
  require_literal "$summary_md" '| First execution action | `read_selected_context` |' "first execution action metric"
  require_literal "$summary_md" '| Second execution action | `use_current_reading_step_suggested_tool` |' "second execution action metric"
  require_literal "$summary_md" '| First execution suggested tool | `file_outline` |' "first execution suggested tool metric"
  require_literal "$summary_md" '| First next action | `inspect_seed_file` |' "next action metric"
  require_literal "$summary_md" '| First reading question | `What entrypoints define the main flow?` |' "first reading question metric"
  require_literal "$summary_md" '| Context route reason | selected 2 files, 2 ranges, and 2 reading-plan steps within the token budget; read src/main.ts first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; no omitted candidate follow-up is needed before the selected context is read; continuation read_selected_context |' "context route reason metric"
  require_literal "$summary_md" '| Impact route reason | after selected context is read, pre-edit impact check estimated 2 impacted files at medium risk, including 1 call-related files, 1 dependency-related files, 1 call paths, and 1 dependency paths |' "impact route reason metric"
  require_literal "$summary_md" '| Impacted files | `2` |' "impacted files metric"

  echo "agent-route step summary smoke passed"
}

main "$@"
