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
  echo "external beta fix queue smoke failed: $*" >&2
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

  write_beta_summary "$TEMP_DIR/friction" workflow_friction "demo/friction" "configure MCP first call" "scripts/setup.sh"
  write_beta_summary "$TEMP_DIR/low-quality" route_hit "demo/low-quality" "understand low quality route" "src/router.ts" 65
  write_beta_summary "$TEMP_DIR/miss" route_miss "demo/miss" "understand login routing" "README.md"

  "$ROOT_DIR/scripts/external-beta-cohort-summary.sh" \
    "$TEMP_DIR/friction" \
    "$TEMP_DIR/low-quality" \
    "$TEMP_DIR/miss" \
    --output "$TEMP_DIR/cohort.md" \
    --json "$TEMP_DIR/cohort.json" \
    --min-route-quality-score 70 >"$TEMP_DIR/cohort.out"

  grep -Fq "next_action: fix_workflow_friction" "$TEMP_DIR/cohort.out" ||
    fail "workflow friction should stay ahead of route quality failures"

  "$ROOT_DIR/scripts/external-beta-fix-queue.sh" \
    "$TEMP_DIR/cohort.json" \
    --output "$TEMP_DIR/fix-queue.md" \
    --json "$TEMP_DIR/fix-queue.json" \
    --check >"$TEMP_DIR/queue.out"

  grep -Fq "external beta fix queue written to $TEMP_DIR/fix-queue.md" "$TEMP_DIR/queue.out" ||
    fail "missing queue output path"
  grep -Fq "status: actionable" "$TEMP_DIR/queue.out" ||
    fail "queue should be actionable"
  grep -Fq "items: 3" "$TEMP_DIR/queue.out" ||
    fail "queue should include three actionable items"
  grep -Fq '| 1 | `workflow_friction` | https://example.com/demo/friction | configure MCP first call | `scripts/setup.sh` | `high / 96` | Fix the trial workflow' "$TEMP_DIR/fix-queue.md" ||
    fail "workflow friction should be first in markdown queue"
  grep -Fq '| 2 | `route_quality_below_threshold` | https://example.com/demo/low-quality | understand low quality route | `src/router.ts` | `high / 65` | Improve the route' "$TEMP_DIR/fix-queue.md" ||
    fail "low route quality should be second in markdown queue"
  grep -Fq '| 3 | `route_miss` | https://example.com/demo/miss | understand login routing | `README.md` | `high / 96` | Reproduce the task' "$TEMP_DIR/fix-queue.md" ||
    fail "route miss should be third in markdown queue"

  jq -e \
    '.status == "actionable"
      and .item_count == 3
      and .cohort_next_action == "fix_workflow_friction"
      and .items[0].priority == "workflow_friction"
      and .items[1].priority == "route_quality_below_threshold"
      and .items[1].route_quality_score == 65
      and .items[2].priority == "route_miss"' \
    "$TEMP_DIR/fix-queue.json" >/dev/null ||
    fail "queue JSON does not match expected priority order"

  "$ROOT_DIR/scripts/external-beta-fix-queue.sh" \
    "$TEMP_DIR/cohort.json" \
    --output "$TEMP_DIR/fix-queue-limited.md" \
    --json "$TEMP_DIR/fix-queue-limited.json" \
    --max-items 1 >/dev/null
  jq -e '.item_count == 1 and .items[0].priority == "workflow_friction"' "$TEMP_DIR/fix-queue-limited.json" >/dev/null ||
    fail "max-items should limit the queue"

  echo "external beta fix queue smoke passed"
}

main "$@"
