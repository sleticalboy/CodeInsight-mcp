#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

fail() {
  echo "codebase-memory bridge cohort report smoke failed: $*" >&2
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

write_pair() {
  local slug="$1"
  local task="$2"
  local backend_top="$3"
  local selected_second="$4"

  local backend_json="$TEMP_DIR/$slug-backend.json"
  local route_json="$TEMP_DIR/$slug-route.json"

  cat >"$backend_json" <<JSON
{
  "provider": "codebase-memory-mcp",
  "candidate_files": ["$backend_top", "$selected_second", "src/main.rs"],
  "evidence_sources": ["search_graph"],
  "evidence_count": 3,
  "latency_ms": 12,
  "confidence": 0.9,
  "notes": ["cohort report smoke"]
}
JSON

  cat >"$route_json" <<JSON
{
  "context_pack": {
    "files": [
      {"file": "$backend_top"},
      {"file": "$selected_second"}
    ]
  },
  "routing_decision": {
    "first_file": "$backend_top",
    "backend_evidence": {
      "provider": "codebase-memory-mcp",
      "candidate_files": ["$backend_top", "$selected_second", "src/main.rs"],
      "evidence_sources": ["search_graph"],
      "evidence_count": 3,
      "latency_ms": 12,
      "confidence": 0.9,
      "notes": ["cohort report smoke"]
    },
    "route_quality": {
      "level": "high",
      "score": 100,
      "evidence_count": 9,
      "evidence_sources": [
        "seed file",
        "backend:codebase-memory-mcp",
        "backend:codebase-memory-mcp:search_graph"
      ],
      "warnings": [],
      "verification_steps": [
        "Treat backend codebase-memory-mcp evidence as advisory unless the selected file and verification checks agree."
      ],
      "recommended_action": "read_selected_context"
    }
  }
}
JSON

  printf '%s\t%s\t%s\t%s\n' "$slug" "$task" "$backend_json" "$route_json" >>"$TEMP_DIR/manifest.tsv"
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

  printf 'slug\ttask\tbackend_evidence_json\tagent_route_json\n' >"$TEMP_DIR/manifest.tsv"
  write_pair agent-route "understand agent route backend evidence" src/tools.rs src/mcp.rs
  write_pair mcp-dispatch "understand MCP tool dispatch" src/mcp.rs src/tools.rs

  "$ROOT_DIR/scripts/codebase-memory-bridge-cohort-report.sh" \
    --manifest "$TEMP_DIR/manifest.tsv" \
    --output-dir "$TEMP_DIR/output" \
    --min-reports 2 \
    --check >/dev/null

  [ -f "$TEMP_DIR/output/reports/agent-route/summary.json" ] ||
    fail "agent-route report summary missing"
  [ -f "$TEMP_DIR/output/reports/mcp-dispatch/codebase-memory-bridge-report.md" ] ||
    fail "mcp-dispatch markdown report missing"
  [ -f "$TEMP_DIR/output/cohort.md" ] || fail "cohort markdown missing"
  [ -f "$TEMP_DIR/output/cohort.json" ] || fail "cohort JSON missing"

  require_jq "$TEMP_DIR/output/cohort.json" '.status == "pass"' "cohort should pass"
  require_jq "$TEMP_DIR/output/cohort.json" '.report_count == 2 and .pass_count == 2' "cohort counts should match"
  require_jq "$TEMP_DIR/output/cohort.json" '.first_file_top_match_rate == 100' "top match rate should be 100"
  require_jq "$TEMP_DIR/output/cohort.json" 'all(.reports[]; .route_quality_recommended_action == "read_selected_context")' "cohort should preserve route actions"

  grep -Fq 'understand MCP tool dispatch' "$TEMP_DIR/output/cohort.md" ||
    fail "cohort markdown should include task label"
  grep -Fq 'Agent route action' "$TEMP_DIR/output/cohort.md" ||
    fail "cohort markdown should include route action column"

  echo "codebase-memory bridge cohort report smoke passed"
}

main "$@"
