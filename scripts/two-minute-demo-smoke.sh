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

  cat >"$TEMP_DIR/agent-router-demo" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

cat <<'DEMO'
CodeInsight agent context router demo

1. index_project
   indexed_files: 23
   symbols: 911

2. project_overview
   entrypoints: 7
   recommended_next_tools: 4

3. context_pack
   selected_files: 10
   selected_ranges: 11
   reading_plan_steps: 8
   first_next_action: inspect_seed_file
   line_reduction: 98.4%
   continuation: complete

4. impact_analysis
   risk_level: high
   impacted_files: 11
   suggested_checks: 3
DEMO
EOF
  chmod +x "$TEMP_DIR/agent-router-demo"

  CODEINSIGHT_AGENT_ROUTER_DEMO_SCRIPT="$TEMP_DIR/agent-router-demo" \
    "$ROOT_DIR/scripts/two-minute-demo.sh" >"$TEMP_DIR/output.log"

  grep -Fq 'Problem: AI agents waste the first read' "$TEMP_DIR/output.log" ||
    fail "missing problem statement"
  grep -Fq 'Promise: route the agent through project_overview, context_pack, and impact_analysis before edits.' "$TEMP_DIR/output.log" ||
    fail "missing product promise"
  grep -Fq 'project_overview found 7 entrypoints and 4 recommended next tools.' "$TEMP_DIR/output.log" ||
    fail "missing overview talk track"
  grep -Fq 'context_pack selected 10 files and 11 ranges, then produced 8 reading-plan steps.' "$TEMP_DIR/output.log" ||
    fail "missing context_pack talk track"
  grep -Fq 'selected context reduced source reading by 98.4%.' "$TEMP_DIR/output.log" ||
    fail "missing line reduction talk track"
  grep -Fq 'impact_analysis reports high risk across 11 impacted files with 3 suggested checks.' "$TEMP_DIR/output.log" ||
    fail "missing impact_analysis talk track"
  grep -Fq 'Read context_pack.files in reading_plan order' "$TEMP_DIR/output.log" ||
    fail "missing agent policy"

  echo "two-minute demo smoke passed"
}

main "$@"
