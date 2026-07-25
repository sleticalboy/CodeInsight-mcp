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
  echo "external beta cohort summary smoke failed: $*" >&2
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
  write_beta_summary "$TEMP_DIR/miss" route_miss "demo/miss" "understand login routing" "README.md"
  write_beta_summary "$TEMP_DIR/friction" workflow_friction "demo/friction" "configure MCP first call" "scripts/setup.sh"

  "$ROOT_DIR/scripts/external-beta-cohort-summary.sh" \
    "$TEMP_DIR/hit" \
    "$TEMP_DIR/miss/beta-summary.json" \
    "$TEMP_DIR/friction" \
    --output "$TEMP_DIR/cohort.md" \
    --json "$TEMP_DIR/cohort.json" \
    --min-route-quality-score 70 \
    --check >"$TEMP_DIR/output.log"

  grep -Fq "external beta cohort summary written to $TEMP_DIR/cohort.md" "$TEMP_DIR/output.log" ||
    fail "missing output path"
  grep -Fq "status: complete" "$TEMP_DIR/output.log" ||
    fail "complete cohort should pass check"
  grep -Fq "next_action: fix_workflow_friction" "$TEMP_DIR/output.log" ||
    fail "workflow friction should be highest priority"
  grep -Fq "quality_gate: pass >= 70" "$TEMP_DIR/output.log" ||
    fail "route quality gate should pass"

  grep -Fq '# External Beta Cohort Summary' "$TEMP_DIR/cohort.md" ||
    fail "missing markdown title"
  grep -Fq -- '- Reports: `3/3`' "$TEMP_DIR/cohort.md" ||
    fail "missing report count"
  grep -Fq -- '- Next action: `fix_workflow_friction`' "$TEMP_DIR/cohort.md" ||
    fail "missing next action"
  grep -Fq -- '- Route quality gate: `pass >= 70`' "$TEMP_DIR/cohort.md" ||
    fail "missing route quality gate"
  grep -Fq '| `workflow_friction` | https://example.com/demo/friction | configure MCP first call | `scripts/setup.sh` | `20.0x` | `pass` | `high / 96` |' "$TEMP_DIR/cohort.md" ||
    fail "missing workflow friction row"

  jq -e \
    '.status == "complete"
      and .report_count == 3
      and .complete_cohort == true
      and .needs_triage_count == 0
      and .classification_counts.route_hit == 1
      and .classification_counts.route_miss == 1
      and .classification_counts.workflow_friction == 1
      and .quality_gate.status == "pass"
      and .quality_gate.min_route_quality_score == 70
      and .quality_gate.failure_count == 0
      and .next_action == "fix_workflow_friction"
      and .priority_reports[0].outcome == "workflow_friction"' \
    "$TEMP_DIR/cohort.json" >/dev/null ||
    fail "summary JSON does not match complete cohort contract"

  write_beta_summary "$TEMP_DIR/triage" needs_triage "demo/triage" "understand routing" "src/router.ts"
  if "$ROOT_DIR/scripts/external-beta-cohort-summary.sh" \
    "$TEMP_DIR/hit" "$TEMP_DIR/triage" \
    --output "$TEMP_DIR/incomplete.md" \
    --json "$TEMP_DIR/incomplete.json" \
    --check >"$TEMP_DIR/incomplete.out" 2>"$TEMP_DIR/incomplete.err"; then
    fail "check should fail for insufficient reports with needs_triage"
  fi
  grep -Fq 'cohort is not complete: status=insufficient_reports, reports=2/3, needs_triage=1' "$TEMP_DIR/incomplete.err" ||
    fail "missing incomplete cohort error"

  write_beta_summary "$TEMP_DIR/low-quality" route_hit "demo/low-quality" "understand low quality route" "src/router.ts" 65
  if "$ROOT_DIR/scripts/external-beta-cohort-summary.sh" \
    "$TEMP_DIR/hit" "$TEMP_DIR/miss" "$TEMP_DIR/low-quality" \
    --output "$TEMP_DIR/quality-fail.md" \
    --json "$TEMP_DIR/quality-fail.json" \
    --min-route-quality-score 70 \
    --check >"$TEMP_DIR/quality-fail.out" 2>"$TEMP_DIR/quality-fail.err"; then
    fail "check should fail when a route quality score is below the gate"
  fi
  grep -Fq 'cohort is not complete: status=quality_gate_failed, reports=3/3, needs_triage=0, route_quality_failures=1' "$TEMP_DIR/quality-fail.err" ||
    fail "missing route quality gate failure error"
  jq -e \
    '.status == "quality_gate_failed"
      and .complete_cohort == false
      and .quality_gate.status == "fail"
      and .quality_gate.failure_count == 1
      and .quality_gate.failures[0].route_quality_score == 65
      and .next_action == "fix_low_quality_routes"
      and .priority_outcome == "route_quality_below_threshold"
      and .priority_reports[0].route_quality_score == 65' \
    "$TEMP_DIR/quality-fail.json" >/dev/null ||
    fail "quality gate failure JSON does not match expected contract"

  echo "external beta cohort summary smoke passed"
}

main "$@"
