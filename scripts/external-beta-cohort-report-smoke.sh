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
  echo "external beta cohort report smoke failed: $*" >&2
  exit 1
}

write_beta_summary() {
  local dir="$1"
  local outcome="$2"
  local repo="$3"
  local task="$4"
  local first_file="$5"
  local route_quality_score="${6:-96}"

  mkdir -p "$dir"
  cat >"$dir/beta-summary.json" <<JSON
{
  "status": "pass",
  "stage": "external_beta_trial",
  "repository": "$repo",
  "repository_url": "https://example.com/$repo",
  "task": "$task",
  "expected_first_read": "$first_file",
  "install_method": "Source",
  "mcp_client": "Codex",
  "codeinsight_version": "codeinsight 0.1.0 smoke",
  "outcome": "$outcome",
  "private_repo": false,
  "adoption_summary": {
    "status": "pass",
    "local_evidence": {
      "metrics": {
        "first_file": "$first_file",
        "first_seed_value": "$first_file",
        "line_reduction": "95.0%",
        "read_less_ratio": "20.0x",
        "risk_level": "medium",
        "first_suggested_tool": "file_outline"
      }
    },
    "mcp_first_call": {
      "status": "pass",
      "route_quality": {
        "level": "high",
        "score": $route_quality_score,
        "recommended_action": "read_selected_context"
      }
    }
  },
  "artifacts": {
    "issue_body": "$dir/issue-body.md",
    "maintainer_triage": "$dir/maintainer-triage.md"
  }
}
JSON
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  write_beta_summary "$TEMP_DIR/hit" route_hit "demo/hit" "understand auth entrypoint" "src/main.ts"
  write_beta_summary "$TEMP_DIR/friction" workflow_friction "demo/friction" "configure MCP first call" "scripts/setup.sh"
  write_beta_summary "$TEMP_DIR/miss" route_miss "demo/miss" "understand login routing" "README.md"

  "$ROOT_DIR/scripts/external-beta-cohort-report.sh" \
    "$TEMP_DIR/hit" \
    "$TEMP_DIR/friction/beta-summary.json" \
    "$TEMP_DIR/miss" \
    --output-dir "$TEMP_DIR/handoff" \
    --min-route-quality-score 70 \
    --max-items 2 \
    --check >"$TEMP_DIR/output.log"

  grep -Fq "external beta cohort report written to $TEMP_DIR/handoff/README.md" "$TEMP_DIR/output.log" ||
    fail "missing handoff README output"
  [ -f "$TEMP_DIR/handoff/external-beta-cohort.md" ] || fail "cohort markdown missing"
  [ -f "$TEMP_DIR/handoff/external-beta-cohort-summary.json" ] || fail "cohort JSON missing"
  [ -f "$TEMP_DIR/handoff/external-beta-fix-queue.md" ] || fail "fix queue markdown missing"
  [ -f "$TEMP_DIR/handoff/external-beta-fix-queue.json" ] || fail "fix queue JSON missing"
  [ -f "$TEMP_DIR/handoff/README.md" ] || fail "handoff README missing"

  grep -Fq '# External Beta Cohort Handoff' "$TEMP_DIR/handoff/README.md" ||
    fail "handoff README title missing"
  grep -Fq -- '- Cohort status: `complete`' "$TEMP_DIR/handoff/README.md" ||
    fail "handoff README cohort status missing"
  grep -Fq -- '- Next action: `fix_workflow_friction`' "$TEMP_DIR/handoff/README.md" ||
    fail "handoff README next action missing"
  grep -Fq -- '- Fix queue items: `2`' "$TEMP_DIR/handoff/README.md" ||
    fail "handoff README max-items count missing"

  jq -e \
    '.status == "complete"
      and .report_count == 3
      and .next_action == "fix_workflow_friction"
      and .quality_gate.status == "pass"' \
    "$TEMP_DIR/handoff/external-beta-cohort-summary.json" >/dev/null ||
    fail "cohort summary JSON contract mismatch"

  jq -e \
    '.status == "actionable"
      and .item_count == 2
      and .items[0].priority == "workflow_friction"
      and .items[1].priority == "route_miss"' \
    "$TEMP_DIR/handoff/external-beta-fix-queue.json" >/dev/null ||
    fail "fix queue JSON contract mismatch"

  echo "external beta cohort report smoke passed"
}

main "$@"
