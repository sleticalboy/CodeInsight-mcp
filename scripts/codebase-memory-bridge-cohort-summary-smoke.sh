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
    "route_quality_score": 100
  },
  "agreement": {
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
    "understand embedding provider" warn src/embedding.rs src/tools.rs false false investigate_backend_local_conflict

  "$ROOT_DIR/scripts/codebase-memory-bridge-cohort-summary.sh" \
    "$TEMP_DIR/pass-1" \
    "$TEMP_DIR/pass-2/summary.json" \
    --min-reports 2 \
    --check \
    --output "$TEMP_DIR/pass.md" \
    --json "$TEMP_DIR/pass.json" >/dev/null

  require_jq "$TEMP_DIR/pass.json" '.status == "pass"' "pass cohort should pass"
  require_jq "$TEMP_DIR/pass.json" '.report_count == 2 and .pass_count == 2 and .warn_count == 0' "pass cohort counts should match"
  require_jq "$TEMP_DIR/pass.json" '.first_file_top_match_rate == 100' "top match rate should be 100"
  require_jq "$TEMP_DIR/pass.json" '.selected_backend_candidate_rate == 50' "candidate coverage should aggregate"
  grep -Fq 'First-file top match rate: `100%`' "$TEMP_DIR/pass.md" ||
    fail "pass markdown should include top match rate"

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
  grep -Fq 'cohort is not clean: status=needs_review, reports=2/2, conflicts=1' "$TEMP_DIR/conflict.err" ||
    fail "conflict error should explain check failure"

  echo "codebase-memory bridge cohort summary smoke passed"
}

main "$@"
