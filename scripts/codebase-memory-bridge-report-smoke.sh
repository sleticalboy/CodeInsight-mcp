#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

fail() {
  echo "codebase-memory bridge report smoke failed: $*" >&2
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

  local backend_json route_json output_dir summary_json report_md
  local conflict_backend_json conflict_route_json conflict_output_dir conflict_summary_json conflict_report_md
  backend_json="$TEMP_DIR/backend-evidence.json"
  route_json="$TEMP_DIR/agent-route.json"
  output_dir="$TEMP_DIR/report"
  summary_json="$output_dir/summary.json"
  report_md="$output_dir/codebase-memory-bridge-report.md"
  conflict_backend_json="$TEMP_DIR/backend-conflict-evidence.json"
  conflict_route_json="$TEMP_DIR/agent-route-conflict.json"
  conflict_output_dir="$TEMP_DIR/conflict-report"
  conflict_summary_json="$conflict_output_dir/summary.json"
  conflict_report_md="$conflict_output_dir/codebase-memory-bridge-report.md"

  cat >"$backend_json" <<'JSON'
{
  "provider": "codebase-memory-mcp",
  "candidate_files": ["src/auth.ts", "src/audit.ts", "src/main.ts"],
  "evidence_sources": ["search_graph", "search_code", "get_architecture:entry_points"],
  "evidence_count": 5,
  "latency_ms": 33,
  "confidence": 0.86,
  "notes": ["smoke fixture"]
}
JSON

  cat >"$route_json" <<'JSON'
{
  "context_pack": {
    "files": [
      {"file": "src/auth.ts"},
      {"file": "src/audit.ts"}
    ]
  },
  "routing_decision": {
    "first_file": "src/auth.ts",
    "backend_route_agreement": {
      "status": "agree",
      "message": "Backend codebase-memory-mcp and local routing agree on first-read file src/auth.ts.",
      "recommended_action": "read_selected_context",
      "provider": "codebase-memory-mcp",
      "local_first_file": "src/auth.ts",
      "backend_first_file": "src/auth.ts",
      "candidate_file_count": 3,
      "common_files": ["src/auth.ts"]
    },
    "backend_evidence": {
      "provider": "codebase-memory-mcp",
      "candidate_files": ["src/auth.ts", "src/audit.ts", "src/main.ts"],
      "evidence_sources": ["search_graph", "search_code", "get_architecture:entry_points"],
      "evidence_count": 5,
      "latency_ms": 33,
      "confidence": 0.86,
      "notes": ["smoke fixture"]
    },
    "route_quality": {
      "level": "high",
      "score": 100,
      "evidence_count": 17,
      "evidence_sources": [
        "seed file",
        "backend:codebase-memory-mcp",
        "backend:codebase-memory-mcp:search_graph",
        "backend:codebase-memory-mcp:search_code"
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

  "$ROOT_DIR/scripts/codebase-memory-bridge-report.sh" \
    --backend-evidence "$backend_json" \
    --agent-route-json "$route_json" \
    --task "inspect authentication routing" \
    --output-dir "$output_dir" >/dev/null

  [ -f "$summary_json" ] || fail "summary.json missing"
  [ -f "$report_md" ] || fail "markdown report missing"

  require_jq "$summary_json" '.status == "pass"' "summary should pass"
  require_jq "$summary_json" '.provider == "codebase-memory-mcp"' "provider should be preserved"
  require_jq "$summary_json" '.agreement.first_file_matches_backend_top == true' "first file should match backend top"
  require_jq "$summary_json" '.agreement.backend_route_agreement_status == "agree"' "backend route agreement should be surfaced"
  require_jq "$summary_json" '.route.backend_route_agreement.recommended_action == "read_selected_context"' "backend route agreement action should be surfaced"
  require_jq "$summary_json" '.agreement.first_file_in_backend_candidates == true' "first file should appear in backend candidates"
  require_jq "$summary_json" '.agreement.selected_backend_candidate_count == 2' "selected backend candidate count should be measured"
  require_jq "$summary_json" '.route.backend_advisory_verification_step_present == true' "advisory verification step should be detected"
  require_jq "$summary_json" '.route.route_preserved_backend_evidence == true' "route should preserve backend evidence"
  require_jq "$summary_json" '.next_action == "use_agent_route_selected_context"' "next action should keep agent-route context first"

  grep -Fq 'First file matches backend top: `true`' "$report_md" ||
    fail "markdown should summarize top-file agreement"
  grep -Fq 'Backend route agreement: `agree`' "$report_md" ||
    fail "markdown should summarize backend route agreement"
  grep -Fq 'Agent route action: `read_selected_context`' "$report_md" ||
    fail "markdown should include route quality recommended action"
  grep -Fq 'Backend evidence preserved in route JSON' "$report_md" ||
    fail "markdown should include preservation check"

  cat >"$conflict_backend_json" <<'JSON'
{
  "provider": "codebase-memory-mcp",
  "candidate_files": ["src/server.ts", "src/main.ts"],
  "evidence_sources": ["search_graph", "get_architecture:entry_points"],
  "evidence_count": 3,
  "latency_ms": 28,
  "confidence": 0.81,
  "notes": ["backend preferred a different first file"]
}
JSON

  cat >"$conflict_route_json" <<'JSON'
{
  "context_pack": {
    "files": [
      {"file": "src/auth.ts"},
      {"file": "src/audit.ts"}
    ]
  },
  "routing_decision": {
    "first_file": "src/auth.ts",
    "backend_route_agreement": {
      "status": "conflict",
      "message": "Local routing selected src/auth.ts, but backend codebase-memory-mcp preferred src/server.ts.",
      "recommended_action": "compare_backend_route_before_edits",
      "provider": "codebase-memory-mcp",
      "local_first_file": "src/auth.ts",
      "backend_first_file": "src/server.ts",
      "candidate_file_count": 2
    },
    "backend_evidence": {
      "provider": "codebase-memory-mcp",
      "candidate_files": ["src/server.ts", "src/main.ts"],
      "evidence_sources": ["search_graph", "get_architecture:entry_points"],
      "evidence_count": 3,
      "latency_ms": 28,
      "confidence": 0.81,
      "notes": ["backend preferred a different first file"]
    },
    "route_quality": {
      "level": "medium",
      "score": 74,
      "evidence_count": 11,
      "evidence_sources": [
        "seed file",
        "backend:codebase-memory-mcp",
        "backend:codebase-memory-mcp:search_graph"
      ],
      "warnings": [
        "Backend codebase-memory-mcp preferred src/server.ts; verify before editing because local routing selected src/auth.ts."
      ],
      "verification_steps": [
        "Treat backend codebase-memory-mcp evidence as advisory unless the selected file and verification checks agree.",
        "Compare local route with backend codebase-memory-mcp candidate src/server.ts before editing."
      ],
      "recommended_action": "compare_backend_route_before_edits"
    }
  }
}
JSON

  "$ROOT_DIR/scripts/codebase-memory-bridge-report.sh" \
    --backend-evidence "$conflict_backend_json" \
    --agent-route-json "$conflict_route_json" \
    --task "inspect authentication routing" \
    --output-dir "$conflict_output_dir" >/dev/null

  [ -f "$conflict_summary_json" ] || fail "conflict summary.json missing"
  [ -f "$conflict_report_md" ] || fail "conflict markdown report missing"

  require_jq "$conflict_summary_json" '.status == "warn"' "conflict summary should warn"
  require_jq "$conflict_summary_json" '.agreement.backend_route_agreement_status == "conflict"' "conflict backend route agreement should be surfaced"
  require_jq "$conflict_summary_json" '.route.backend_route_agreement.recommended_action == "compare_backend_route_before_edits"' "conflict backend route agreement action should be surfaced"
  require_jq "$conflict_summary_json" '.agreement.first_file_matches_backend_top == false' "conflict first file should not match backend top"
  require_jq "$conflict_summary_json" '.agreement.first_file_in_backend_candidates == false' "conflict first file should not be in backend candidates"
  require_jq "$conflict_summary_json" '.route.route_quality_recommended_action == "compare_backend_route_before_edits"' "conflict route action should be preserved"
  require_jq "$conflict_summary_json" '.route.route_quality_warnings | length == 1' "conflict route warning should be preserved"
  require_jq "$conflict_summary_json" '.next_action == "investigate_backend_local_conflict"' "conflict report next action should require investigation"

  grep -Fq 'Agent route action: `compare_backend_route_before_edits`' "$conflict_report_md" ||
    fail "conflict markdown should include route quality recommended action"
  grep -Fq 'Backend route agreement: `conflict`' "$conflict_report_md" ||
    fail "conflict markdown should include backend route agreement"
  grep -Fq 'Route warning count | `1`' "$conflict_report_md" ||
    fail "conflict markdown should include route warning count"

  echo "codebase-memory bridge report smoke passed"
}

main "$@"
