#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

fail() {
  echo "codebase-memory bridge cohort summary smoke failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

write_summary() {
  local path="$1"
  local task="$2"
  local status="$3"
  local backend_top="$4"
  local first_file="$5"
  local top_match="$6"
  local candidate_match="$7"
  local next_action="$8"
  local route_action="${9:-read_selected_context}"
  local warning_count="${10:-0}"
  local backend_agreement_status="${11:-agree}"
  local backend_agreement_action="${12:-read_selected_context}"

  mkdir -p "$(dirname "$path")"
  cat >"$path" <<JSON
{
  "status": "$status",
  "task": "$task",
  "provider": "codebase-memory-mcp",
  "backend": {
    "top_file": "$backend_top",
    "evidence_count": 4
  },
  "route": {
    "first_file": "$first_file",
    "route_quality_level": "high",
    "route_quality_score": 100,
    "route_quality_recommended_action": "$route_action",
    "route_quality_warnings": $(if [ "$warning_count" -gt 0 ]; then printf '["backend/local first-file conflict"]'; else printf '[]'; fi),
    "backend_route_agreement": {
      "status": "$backend_agreement_status",
      "recommended_action": "$backend_agreement_action",
      "message": "fixture agreement status $backend_agreement_status",
      "provider": "codebase-memory-mcp",
      "local_first_file": "$first_file",
      "backend_first_file": "$backend_top",
      "candidate_file_count": 4
    }
  },
  "agreement": {
    "backend_route_agreement_status": "$backend_agreement_status",
    "backend_route_agreement_recommended_action": "$backend_agreement_action",
    "first_file_matches_backend_top": $top_match,
    "first_file_in_backend_candidates": $candidate_match,
    "selected_backend_candidate_count": 2,
    "backend_candidate_count": 4
  },
  "next_action": "$next_action"
}
JSON
}

require_jq() {
  local file="$1"
  local query="$2"
  local description="$3"

  if ! jq -e "$query" "$file" >/dev/null; then
    echo "query: $query" >&2
    fail "$description"
  fi
}

main() {
  require_command jq

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  write_summary "$TEMP_DIR/pass-1/summary.json" \
    "understand agent routing" pass src/tools.rs src/tools.rs true true use_agent_route_selected_context
  write_summary "$TEMP_DIR/pass-2/summary.json" \
    "understand mcp dispatch" pass src/mcp.rs src/mcp.rs true true use_agent_route_selected_context
  write_summary "$TEMP_DIR/conflict/summary.json" \
    "understand embedding provider" warn src/embedding.rs src/tools.rs false false investigate_backend_local_conflict compare_backend_route_before_edits 1 conflict compare_backend_route_before_edits

  "$ROOT_DIR/scripts/codebase-memory-bridge-cohort-summary.sh" \
    "$TEMP_DIR/pass-1" \
    "$TEMP_DIR/pass-2/summary.json" \
    --min-reports 2 \
    --check \
    --output "$TEMP_DIR/pass.md" \
    --json "$TEMP_DIR/pass.json" >/dev/null

  require_jq "$TEMP_DIR/pass.json" '.status == "pass"' "pass cohort should pass"
  require_jq "$TEMP_DIR/pass.json" '.report_count == 2 and .pass_count == 2 and .warn_count == 0' "pass cohort counts should match"
  require_jq "$TEMP_DIR/pass.json" '.backend_route_agreement_rate == 100' "backend agreement rate should be 100"
  require_jq "$TEMP_DIR/pass.json" '.backend_route_agreement_counts.agree == 2' "backend agreement count should aggregate"
  require_jq "$TEMP_DIR/pass.json" '.first_file_top_match_rate == 100' "top match rate should be 100"
  require_jq "$TEMP_DIR/pass.json" '.selected_backend_candidate_rate == 50' "candidate coverage should aggregate"
  require_jq "$TEMP_DIR/pass.json" 'all(.reports[]; .backend_route_agreement_status == "agree")' "pass cohort should expose agreement status"
  require_jq "$TEMP_DIR/pass.json" 'all(.reports[]; .route_quality_recommended_action == "read_selected_context")' "pass cohort should preserve route actions"
  grep -Fq 'Backend agreement rate: `100%`' "$TEMP_DIR/pass.md" ||
    fail "pass markdown should include backend agreement rate"
  grep -Fq 'Backend agreement counts: `agree=2, overlap=0, conflict=0, backend_only=0`' "$TEMP_DIR/pass.md" ||
    fail "pass markdown should include backend agreement counts"
  grep -Fq 'First-file top match rate: `100%`' "$TEMP_DIR/pass.md" ||
    fail "pass markdown should include top match rate"
  grep -Fq 'Backend agreement' "$TEMP_DIR/pass.md" ||
    fail "pass markdown should include backend agreement column"
  grep -Fq 'Agent route action' "$TEMP_DIR/pass.md" ||
    fail "pass markdown should include agent route action column"

  if "$ROOT_DIR/scripts/codebase-memory-bridge-cohort-summary.sh" \
    "$TEMP_DIR/pass-1" \
    "$TEMP_DIR/conflict" \
    --min-reports 2 \
    --check \
    --output "$TEMP_DIR/conflict.md" \
    --json "$TEMP_DIR/conflict.json" 2>"$TEMP_DIR/conflict.err"; then
    fail "conflict cohort should fail --check"
  fi

  require_jq "$TEMP_DIR/conflict.json" '.status == "needs_review"' "conflict cohort should need review"
  require_jq "$TEMP_DIR/conflict.json" '.conflicts | length == 1' "conflict cohort should list one conflict"
  require_jq "$TEMP_DIR/conflict.json" '.backend_route_agreement_rate == 50' "conflict cohort should expose backend agreement rate"
  require_jq "$TEMP_DIR/conflict.json" '.backend_route_agreement_counts.agree == 1 and .backend_route_agreement_counts.conflict == 1' "conflict cohort should count agreement statuses"
  require_jq "$TEMP_DIR/conflict.json" '.conflicts[0].backend_route_agreement_status == "conflict"' "conflict cohort should preserve agreement status"
  require_jq "$TEMP_DIR/conflict.json" '.next_action == "review_backend_local_conflicts"' "conflict next action should use agreement status"
  require_jq "$TEMP_DIR/conflict.json" '.conflicts[0].route_quality_recommended_action == "compare_backend_route_before_edits"' "conflict cohort should preserve route action"
  require_jq "$TEMP_DIR/conflict.json" '.conflicts[0].route_warning_count == 1' "conflict cohort should preserve warning count"
  grep -Fq 'cohort is not clean: status=needs_review, reports=2/2, backend_agreement_rate=50/100, conflicts=1' "$TEMP_DIR/conflict.err" ||
    fail "conflict error should explain check failure"
  grep -Fq 'Backend agreement rate: `50%`' "$TEMP_DIR/conflict.md" ||
    fail "conflict markdown should include backend agreement rate"
  grep -Fq 'Backend agreement counts: `agree=1, overlap=0, conflict=1, backend_only=0`' "$TEMP_DIR/conflict.md" ||
    fail "conflict markdown should include backend agreement counts"
  grep -Fq 'compare_backend_route_before_edits' "$TEMP_DIR/conflict.md" ||
    fail "conflict markdown should include route action"

  echo "codebase-memory bridge cohort summary smoke passed"
}

main "$@"
