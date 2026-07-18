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
  echo "local repo evidence smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo/src"
  cat >"$TEMP_DIR/repo/src/main.ts" <<'EOF'
export function main() {
  return "ok";
}
EOF

  cat >"$TEMP_DIR/codeinsight" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

if [ "${1:-}" != "agent-route" ]; then
  echo "unexpected command: $*" >&2
  exit 1
fi

cat <<'JSON'
{
  "root": "/tmp/local-evidence",
  "task": "understand the main application entrypoint",
  "token_budget": 6000,
  "route": [
    {"order": 1, "tool": "index_project", "status": "complete", "reason": "indexed"},
    {"order": 2, "tool": "project_overview", "status": "complete", "reason": "overview"},
    {"order": 3, "tool": "context_pack", "status": "complete", "reason": "selected 1 files, 1 ranges, and 1 reading-plan steps within the token budget; read src/main.ts first (candidate rank 1) via inspect_seed_file, use file_outline when deeper evidence is needed; no omitted candidate follow-up is needed before the selected context is read; continuation read_selected_context"},
    {"order": 4, "tool": "impact_analysis", "status": "complete", "reason": "pre-edit impact check estimated 2 impacted files at medium risk"}
  ],
  "execution_plan": [
    {"order": 1, "action": "read_selected_context", "status": "ready"},
    {
      "order": 2,
      "action": "use_current_reading_step_suggested_tool",
      "status": "available_after_current_file",
      "suggested_tool": {"tool": "file_outline"}
    },
    {"order": 3, "action": "use_continuation_if_needed", "status": "complete"},
    {"order": 4, "action": "review_impact_before_edits", "status": "complete"}
  ],
  "index_report": {
    "indexed_files": 3,
    "symbols": 8,
    "errors": []
  },
  "overview": {
    "total_lines": 120,
    "entrypoints": [{"file": "src/main.ts"}],
    "recommended_next_tools": [{}, {}]
  },
  "context_pack": {
    "seed_strategy": "auto_task_match",
    "selected_seeds": [
      {
        "kind": "file",
        "value": "src/router.ts",
        "source": "task_match",
        "role": "source",
        "matched_keywords": ["router"]
      },
      {
        "kind": "file",
        "value": "src/main.ts",
        "source": "overview_entrypoint",
        "role": "source",
        "matched_keywords": []
      }
    ],
    "estimated_tokens": 180,
    "reading_plan": [
      {
        "order": 1,
        "file": "src/main.ts",
        "selection_rank": 1,
        "next_action": "inspect_seed_file",
        "question": "What setup code defines the main application flow?",
        "selection_reason": "Selected for high relevance via seed_file"
      }
    ],
    "budget": {
      "selected_ranges": 1
    },
    "continuation_summary": {"status": "complete"},
    "files": [
      {
        "file": "src/main.ts",
        "ranges": [
          {"start_line": 1, "end_line": 12}
        ]
      }
    ]
  },
  "impact_analysis": {
    "risk_level": "medium",
    "impact_counts": {
      "impacted_files": 2
    },
    "suggested_checks": [{}, {}]
  }
}
JSON
EOF
  chmod +x "$TEMP_DIR/codeinsight"

  CODEINSIGHT_BIN="$TEMP_DIR/codeinsight" \
    "$ROOT_DIR/scripts/local-repo-evidence.sh" \
    "$TEMP_DIR/repo" \
    --output "$TEMP_DIR/evidence.md" \
    --json "$TEMP_DIR/agent-route.json" \
    --summary-json "$TEMP_DIR/summary.json" >"$TEMP_DIR/output.log"

  grep -Fq "local repo evidence summary JSON written to $TEMP_DIR/summary.json" "$TEMP_DIR/output.log" ||
    fail "missing summary JSON output path"

  grep -Fq '# CodeInsight Local Repository Evidence' "$TEMP_DIR/evidence.md" ||
    fail "missing evidence title"
  grep -Fq -- '- Route: `index_project -> project_overview -> context_pack -> impact_analysis`' "$TEMP_DIR/evidence.md" ||
    fail "missing route"
  grep -Fq -- '- Selected context: `12/120` source lines, `90.0%` reduction' "$TEMP_DIR/evidence.md" ||
    fail "missing selected context reduction"
  grep -Fq -- '- First selected file: `src/main.ts`' "$TEMP_DIR/evidence.md" ||
    fail "missing first selected file"
  grep -Fq -- '- Seed strategy: `auto_task_match`' "$TEMP_DIR/evidence.md" ||
    fail "missing seed strategy"
  grep -Fq -- '- Selected seeds: `2`' "$TEMP_DIR/evidence.md" ||
    fail "missing selected seed count"
  grep -Fq -- '- First seed source: `task_match`' "$TEMP_DIR/evidence.md" ||
    fail "missing first seed source"
  grep -Fq -- '- Companion entrypoint: `src/main.ts`' "$TEMP_DIR/evidence.md" ||
    fail "missing companion entrypoint"
  grep -Fq -- '- First reading question: What setup code defines the main application flow?' "$TEMP_DIR/evidence.md" ||
    fail "missing first reading question"
  grep -Fq -- '- First suggested tool: `file_outline`' "$TEMP_DIR/evidence.md" ||
    fail "missing first suggested tool"
  grep -Fq -- '- Impact risk: `medium`' "$TEMP_DIR/evidence.md" ||
    fail "missing impact risk"
  grep -Fq 'Raw agent_route JSON:' "$TEMP_DIR/evidence.md" ||
    fail "missing raw JSON path"

  jq -e '.route[0].tool == "index_project" and .context_pack.files[0].file == "src/main.ts"' \
    "$TEMP_DIR/agent-route.json" >/dev/null ||
    fail "raw JSON file does not contain the expected route payload"
  jq -e \
    '.status == "pass"
      and .route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .metrics.total_lines == 120
      and .metrics.selected_lines == 12
      and .metrics.line_reduction == "90.0%"
      and .metrics.seed_strategy == "auto_task_match"
      and .metrics.selected_seed_count == 2
      and .metrics.first_seed_source == "task_match"
      and .metrics.first_seed_value == "src/router.ts"
      and .metrics.companion_entrypoint == "src/main.ts"
      and .metrics.first_file == "src/main.ts"
      and .metrics.first_reading_question == "What setup code defines the main application flow?"
      and .metrics.first_suggested_tool == "file_outline"
      and .metrics.risk_level == "medium"
      and .metrics.impacted_files == 2
      and .artifacts.markdown == "'"$TEMP_DIR"'/evidence.md"
      and .artifacts.raw_agent_route_json == "'"$TEMP_DIR"'/agent-route.json"' \
    "$TEMP_DIR/summary.json" >/dev/null ||
    fail "summary JSON file does not contain the expected evidence metrics"

  echo "local repo evidence smoke passed"
}

main "$@"
