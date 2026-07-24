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
  backend_json="$TEMP_DIR/backend-evidence.json"
  route_json="$TEMP_DIR/agent-route.json"
  output_dir="$TEMP_DIR/report"
  summary_json="$output_dir/summary.json"
  report_md="$output_dir/codebase-memory-bridge-report.md"

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
  require_jq "$summary_json" '.agreement.first_file_in_backend_candidates == true' "first file should appear in backend candidates"
  require_jq "$summary_json" '.agreement.selected_backend_candidate_count == 2' "selected backend candidate count should be measured"
  require_jq "$summary_json" '.route.backend_advisory_verification_step_present == true' "advisory verification step should be detected"
  require_jq "$summary_json" '.route.route_preserved_backend_evidence == true' "route should preserve backend evidence"
  require_jq "$summary_json" '.next_action == "use_agent_route_selected_context"' "next action should keep agent-route context first"

  grep -Fq 'First file matches backend top: `true`' "$report_md" ||
    fail "markdown should summarize top-file agreement"
  grep -Fq 'Backend evidence preserved in route JSON' "$report_md" ||
    fail "markdown should include preservation check"

  echo "codebase-memory bridge report smoke passed"
}

main "$@"
