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
  echo "adoption comparison smoke failed: $*" >&2
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
json=""
summary_json=""
task=""
token_budget=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output)
      output="$2"
      shift 2
      ;;
    --json)
      json="$2"
      shift 2
      ;;
    --summary-json)
      summary_json="$2"
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

[ -n "$output" ] || exit 3
[ -n "$json" ] || exit 4
[ -n "$summary_json" ] || exit 5
mkdir -p "$(dirname "$output")" "$(dirname "$json")" "$(dirname "$summary_json")"

cat >"$output" <<'MARKDOWN'
# CodeInsight Local Repository Evidence
MARKDOWN
cat >"$json" <<'JSON'
{"route":[{"tool":"index_project"},{"tool":"project_overview"},{"tool":"context_pack"},{"tool":"impact_analysis"}]}
JSON
cat >"$summary_json" <<JSON
{
  "status": "pass",
  "repository": "$repo_root",
  "task": "$task",
  "token_budget": $token_budget,
  "route_tools": ["index_project", "project_overview", "context_pack", "impact_analysis"],
  "metrics": {
    "total_lines": 1200,
    "selected_lines": 60,
    "line_reduction": "95.0%",
    "selected_files": 3,
    "selected_ranges": 5,
    "estimated_tokens": 900,
    "seed_strategy": "auto_task_match",
    "selected_seed_count": 2,
    "first_seed_source": "task_match",
    "first_seed_value": "src/router.ts",
    "companion_entrypoint": "src/main.ts",
    "first_file": "src/router.ts",
    "first_reading_focus": "Trace login route ownership.",
    "first_reading_question": "What route owns login?",
    "first_selection_rank": 1,
    "first_selection_reason": "Selected for high relevance via seed_file",
    "first_suggested_tool": "file_outline",
    "continuation_status": "complete",
    "continuation_next_action": "read_selected_context",
    "first_omitted_file": "",
    "first_omitted_selection_rank": null,
    "first_omitted_omission_reason": "",
    "first_omitted_next_action": "",
    "risk_level": "medium",
    "impacted_files": 4
  }
}
JSON
EOF
  chmod +x "$TEMP_DIR/local-repo-evidence"

  CODEINSIGHT_LOCAL_REPO_EVIDENCE_SCRIPT="$TEMP_DIR/local-repo-evidence" \
    "$ROOT_DIR/scripts/adoption-comparison.sh" \
    "$TEMP_DIR/repo" \
    --task "understand login routing" \
    --token-budget 6000 \
    --output-dir "$TEMP_DIR/comparison" \
    >"$TEMP_DIR/output.log"

  grep -Fq "adoption comparison written to $TEMP_DIR/comparison/adoption-comparison.md" "$TEMP_DIR/output.log" ||
    fail "missing output path"
  grep -Fq "summary: $TEMP_DIR/comparison/summary.json" "$TEMP_DIR/output.log" ||
    fail "missing summary path"

  grep -Fq -- '- Blind first-read baseline: `1200` source lines' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing blind baseline"
  grep -Fq -- '- CodeInsight routed first-read: `60/1200` source lines' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing routed first read"
  grep -Fq -- '- Source lines avoided: `1140`' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing avoided lines"
  grep -Fq -- '- Read less: `20.0x`' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing read less ratio"
  grep -Fq -- '- Companion entrypoint: `src/main.ts`' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing companion entrypoint"
  grep -Fq -- '- First reading focus: Trace login route ownership.' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing first reading focus"
  grep -Fq -- '- First selection rank: `1`' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing first selection rank"
  grep -Fq -- '- First selection reason: Selected for high relevance via seed_file' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing first selection reason"
  grep -Fq -- '- Continuation next action: `read_selected_context`' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing continuation next action"
  grep -Fq -- '- First omitted candidate: none' "$TEMP_DIR/comparison/adoption-comparison.md" ||
    fail "missing omitted candidate status"

  jq -e \
    '.status == "pass"
      and .metrics.blind_first_read_lines == 1200
      and .metrics.routed_first_read_lines == 60
      and .metrics.source_lines_avoided == 1140
      and .metrics.read_less_ratio == "20.0x"
      and .metrics.first_seed_source == "task_match"
      and .metrics.first_reading_focus == "Trace login route ownership."
      and .metrics.first_selection_rank == 1
      and .metrics.first_selection_reason == "Selected for high relevance via seed_file"
      and .metrics.continuation_status == "complete"
      and .metrics.continuation_next_action == "read_selected_context"
      and .metrics.first_omitted_file == ""
      and .artifacts.raw_agent_route_json == "'"$TEMP_DIR"'/comparison/agent-route.json"' \
    "$TEMP_DIR/comparison/summary.json" >/dev/null ||
    fail "summary JSON does not match expected contract"

  test -f "$TEMP_DIR/comparison/local-repo-evidence.out" ||
    fail "missing local evidence stdout log"
  test -f "$TEMP_DIR/comparison/local-repo-evidence.err" ||
    fail "missing local evidence stderr log"

  echo "adoption comparison smoke passed"
}

main "$@"
