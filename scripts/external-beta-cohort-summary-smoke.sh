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
      "status": "pass"
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
    --check >"$TEMP_DIR/output.log"

  grep -Fq "external beta cohort summary written to $TEMP_DIR/cohort.md" "$TEMP_DIR/output.log" ||
    fail "missing output path"
  grep -Fq "status: complete" "$TEMP_DIR/output.log" ||
    fail "complete cohort should pass check"
  grep -Fq "next_action: fix_workflow_friction" "$TEMP_DIR/output.log" ||
    fail "workflow friction should be highest priority"

  grep -Fq '# External Beta Cohort Summary' "$TEMP_DIR/cohort.md" ||
    fail "missing markdown title"
  grep -Fq -- '- Reports: `3/3`' "$TEMP_DIR/cohort.md" ||
    fail "missing report count"
  grep -Fq -- '- Next action: `fix_workflow_friction`' "$TEMP_DIR/cohort.md" ||
    fail "missing next action"
  grep -Fq '| `workflow_friction` | https://example.com/demo/friction | configure MCP first call | `scripts/setup.sh` | `20.0x` | `pass` |' "$TEMP_DIR/cohort.md" ||
    fail "missing workflow friction row"

  jq -e \
    '.status == "complete"
      and .report_count == 3
      and .complete_cohort == true
      and .needs_triage_count == 0
      and .classification_counts.route_hit == 1
      and .classification_counts.route_miss == 1
      and .classification_counts.workflow_friction == 1
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

  echo "external beta cohort summary smoke passed"
}

main "$@"
