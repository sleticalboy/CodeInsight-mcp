#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "codebase-memory backend evidence smoke failed: $*" >&2
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

write_file() {
  local path="$1"
  local content="$2"

  mkdir -p "$(dirname "$path")"
  printf '%s\n' "$content" >"$path"
}

build_binary_if_needed() {
  if [ -z "$CODEINSIGHT_BIN" ]; then
    require_command cargo
    cargo build --release --locked --manifest-path "$ROOT_DIR/Cargo.toml" >/dev/null
    CODEINSIGHT_BIN="$(cargo metadata --no-deps --format-version 1 --manifest-path "$ROOT_DIR/Cargo.toml" | jq -r '.target_directory')/release/codeinsight"
  fi

  if [ ! -x "$CODEINSIGHT_BIN" ]; then
    fail "CODEINSIGHT_BIN is not executable: $CODEINSIGHT_BIN"
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

create_fixture() {
  local repo="$1"

  write_file "$repo/package.json" '{
  "type": "module",
  "scripts": {
    "test": "vitest"
  }
}'
  write_file "$repo/src/main.ts" 'import { AuthService } from "./auth";

export function main() {
  return new AuthService().login("demo-user");
}
'
  write_file "$repo/src/auth.ts" 'import { auditLogin } from "./audit";

export class AuthService {
  login(user: string) {
    return auditLogin(user);
  }
}
'
  write_file "$repo/src/audit.ts" 'export function auditLogin(user: string) {
  return { user, status: "accepted" };
}
'
  write_file "$repo/src/server.ts" 'import { main } from "./main";

export function startServer() {
  return main();
}
'
}

write_codebase_memory_exports() {
  local repo="$1"
  local output_dir="$2"

  cat >"$output_dir/search-graph.json" <<EOF
{
  "structuredContent": {
    "total": 2,
    "search_mode": "bm25",
    "elapsed_ms": 7,
    "results": [
      {
        "name": "workflowNoise",
        "qualified_name": "fixture.workflowNoise",
        "label": "Module",
        "file_path": ".github/workflows/ci.yml"
      }
    ],
    "semantic_results": [
      {
        "name": "AuthService",
        "qualified_name": "fixture.src.auth.AuthService",
        "label": "Class",
        "file_path": "$repo/src/auth.ts",
        "score": 0.97
      },
      {
        "name": "auditLogin",
        "qualified_name": "fixture.src.audit.auditLogin",
        "label": "Function",
        "file_path": "src/audit.ts",
        "score": 0.91
      }
    ]
  }
}
EOF

  cat >"$output_dir/search-code.json" <<'EOF'
{
  "elapsed_ms": 23,
  "results": [
    {
      "node": "AuthService.login",
      "qualified_name": "fixture.src.auth.AuthService.login",
      "label": "Method",
      "file": "src/auth.ts"
    },
    {
      "node": "auditLogin",
      "qualified_name": "fixture.src.audit.auditLogin",
      "label": "Function",
      "file": "src/audit.ts"
    }
  ]
}
EOF

  cat >"$output_dir/search-graph-empty-semantic.json" <<'EOF'
{
  "results": [
    {
      "name": "main",
      "label": "Function",
      "file_path": "src/main.ts"
    }
  ],
  "semantic_results": [],
  "elapsed_ms": 1
}
EOF

  cat >"$output_dir/search-code-alternates.json" <<'EOF'
{
  "results": [
    {
      "node": "AuthService.login",
      "label": "Method",
      "file": "src/auth.ts",
      "score": 0.94
    }
  ],
  "files": [
    "src/audit.ts",
    {
      "file_path": "src/main.ts",
      "name": "main"
    }
  ],
  "raw_matches": [
    {
      "file": "src/server.ts",
      "line": 1,
      "content": "startServer();"
    }
  ],
  "elapsed_ms": 4
}
EOF

  cat >"$output_dir/code-snippet.json" <<'EOF'
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "get_code_snippet completed; structured payload follows"
      },
      {
        "type": "text",
        "text": "{\"name\":\"login\",\"qualified_name\":\"AuthService.login\",\"label\":\"Method\",\"file_path\":\"src/auth.ts\",\"start_line\":4,\"end_line\":7,\"source\":\"source body must not be copied into bridge evidence\",\"elapsed_ms\":2}"
      }
    ]
  }
}
EOF

  cat >"$output_dir/query-graph.json" <<'EOF'
{
  "columns": ["f.name", "f.file_path", "f.qualified_name"],
  "rows": [
    ["AuthService", "src/auth.ts", "fixture.src.auth.AuthService"]
  ],
  "total": 1,
  "elapsed_ms": 13
}
EOF

  cat >"$output_dir/trace-path.json" <<'EOF'
{
  "function": "AuthService.login",
  "direction": "both",
  "callers": [
    {"name": "main", "qualified_name": "fixture.src.main.main", "hop": 1}
  ],
  "callees": [
    {"name": "auditLogin", "qualified_name": "fixture.src.audit.auditLogin", "hop": 1}
  ],
  "elapsed_ms": 5
}
EOF

  cat >"$output_dir/architecture.json" <<'EOF'
{
  "elapsed_ms": 3,
  "entry_points": [
    {
      "name": "main",
      "qualified_name": "fixture.src.main.main",
      "file": "src/main.ts"
    }
  ]
}
EOF
}

main() {
  require_command jq
  build_binary_if_needed

  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local repo evidence route_json inline_evidence inline_route_json preferred_route_json fallback_route_json conflict_evidence conflict_route_json trace_only_evidence trace_only_route_json
  repo="$TEMP_DIR/repo"
  evidence="$TEMP_DIR/backend-evidence.json"
  route_json="$TEMP_DIR/agent-route.json"
  inline_evidence="$TEMP_DIR/backend-inline-evidence.json"
  inline_route_json="$TEMP_DIR/agent-route-inline.json"
  preferred_route_json="$TEMP_DIR/agent-route-preferred.json"
  fallback_route_json="$TEMP_DIR/agent-route-fallback.json"
  conflict_evidence="$TEMP_DIR/backend-conflict-evidence.json"
  conflict_route_json="$TEMP_DIR/agent-route-conflict.json"
  trace_only_evidence="$TEMP_DIR/backend-trace-only-evidence.json"
  trace_only_route_json="$TEMP_DIR/agent-route-trace-only.json"

  create_fixture "$repo"
  write_codebase_memory_exports "$repo" "$TEMP_DIR"

  if "$ROOT_DIR/scripts/codebase-memory-backend-evidence.sh" \
    --search-graph-json "$TEMP_DIR/search-graph.json" \
    --candidate-limit 17 >"$TEMP_DIR/invalid-limit.json" 2>"$TEMP_DIR/invalid-limit.err"; then
    fail "candidate limit above the runtime contract should be rejected"
  fi
  if ! grep -q -- "--candidate-limit must be <= 16" "$TEMP_DIR/invalid-limit.err"; then
    fail "candidate limit rejection should explain the maximum"
  fi

  "$ROOT_DIR/scripts/codebase-memory-backend-evidence.sh" \
    --root "$repo" \
    --search-graph-json "$TEMP_DIR/search-graph-empty-semantic.json" \
    --output "$TEMP_DIR/empty-semantic-evidence.json"
  require_jq \
    "$TEMP_DIR/empty-semantic-evidence.json" \
    '.candidate_files == ["src/main.ts"] and .candidates[0].symbol == "main"' \
    "empty semantic search results should fall back to keyword candidates"

  "$ROOT_DIR/scripts/codebase-memory-backend-evidence.sh" \
    --root "$repo" \
    --search-code-json "$TEMP_DIR/search-code-alternates.json" \
    --output "$TEMP_DIR/search-code-alternates-evidence.json"
  require_jq \
    "$TEMP_DIR/search-code-alternates-evidence.json" \
    '.candidate_files == ["src/auth.ts", "src/audit.ts", "src/main.ts", "src/server.ts"]' \
    "search_code results, files, and raw matches should all become candidates"
  require_jq \
    "$TEMP_DIR/search-code-alternates-evidence.json" \
    '.candidates[0].symbol == "AuthService.login" and .candidates[1].reason == "search_code File" and .candidates[2].symbol == "main" and .candidates[3].reason == "search_code raw match"' \
    "search_code alternate result shapes should preserve available metadata"

  "$ROOT_DIR/scripts/codebase-memory-backend-evidence.sh" \
    --root "$repo" \
    --code-snippet-json "$TEMP_DIR/code-snippet.json" \
    --search-graph-json "$TEMP_DIR/search-graph.json" \
    --search-code-json "$TEMP_DIR/search-code.json" \
    --query-graph-json "$TEMP_DIR/query-graph.json" \
    --trace-path-json "$TEMP_DIR/trace-path.json" \
    --architecture-json "$TEMP_DIR/architecture.json" \
    --candidate-limit 3 \
    --confidence 0.86 \
    --note "smoke fixture for backend evidence bridge" \
    --output "$evidence"

  require_jq "$evidence" '.provider == "codebase-memory-mcp"' "provider should be codebase-memory-mcp"
  require_jq "$evidence" '.candidate_files == ["src/auth.ts", "src/audit.ts", "src/main.ts"]' "candidate files should be normalized and stable"
  require_jq "$evidence" '.candidates | map(.file) == ["src/auth.ts", "src/audit.ts", "src/main.ts"]' "structured candidates should preserve stable file ranking"
  require_jq "$evidence" '.candidates[0].symbol == "AuthService.login" and .candidates[0].source == "get_code_snippet"' "structured candidates should prioritize the exact snippet symbol"
  require_jq "$evidence" '.candidates[0].reason == "get_code_snippet Method" and .candidates[0].evidence == ["get_code_snippet"]' "structured candidates should explain exact snippet evidence"
  require_jq "$evidence" '.candidates[1].file == "src/audit.ts" and .candidates[1].score == 0.91' "search_graph should prefer semantic candidates over keyword noise"
  require_jq "$evidence" '.evidence_sources | index("get_code_snippet") and index("search_graph") and index("search_code") and index("query_graph") and index("get_architecture:entry_points")' "evidence sources should include all bridge inputs"
  require_jq "$evidence" '.evidence_count == 7' "evidence count should include duplicate backend signals"
  require_jq "$evidence" '.latency_ms == 48' "latency should aggregate exported backend timings"
  require_jq "$evidence" 'tostring | contains("source body must not be copied") | not' "bridge evidence should omit snippet source bodies"
  require_jq "$evidence" '.tool_results.trace_path.callers[0].name == "main" and .tool_results.trace_path.callees[0].name == "auditLogin"' "trace_path should remain raw for runtime symbol resolution"
  require_jq "$evidence" '.confidence == 0.86' "confidence should be preserved"

  jq -n \
    --slurpfile search_graph "$TEMP_DIR/search-graph.json" \
    --slurpfile search_code "$TEMP_DIR/search-code.json" \
    --slurpfile query_graph "$TEMP_DIR/query-graph.json" \
    --slurpfile architecture "$TEMP_DIR/architecture.json" \
    '{
      provider: "codebase-memory-mcp",
      confidence: 0.86,
      tool_results: {
        search_graph: $search_graph[0],
        search_code: $search_code[0],
        query_graph: $query_graph[0],
        get_architecture: $architecture[0]
      }
    }' >"$inline_evidence"

  "$CODEINSIGHT_BIN" agent-route "$repo" \
    --task "inspect src/auth.ts before editing login behavior" \
    --token-budget 1600 \
    --force-index \
    --backend-evidence "$inline_evidence" >"$inline_route_json"

  require_jq "$inline_route_json" '.routing_decision.backend_evidence.candidate_files == ["src/auth.ts", "src/audit.ts", "src/main.ts"]' "inline tool results should preserve normalized candidate ranking"
  require_jq "$inline_route_json" '.routing_decision.backend_evidence.candidates | map(.file) == ["src/auth.ts", "src/audit.ts", "src/main.ts"]' "inline tool results should produce structured candidates"
  require_jq "$inline_route_json" '.routing_decision.backend_evidence.evidence_count == 6 and .routing_decision.backend_evidence.latency_ms == 46' "inline tool results should aggregate evidence count and latency"
  require_jq "$inline_route_json" '.routing_decision.backend_evidence.tool_results == null' "inline tool results should be omitted from the compact route response"
  require_jq "$inline_route_json" 'any(.routing_decision.backend_evidence.notes[]; contains("normalized from inline backend tool_results"))' "inline tool result normalization should remain observable"

  "$CODEINSIGHT_BIN" agent-route "$repo" \
    --task "inspect src/auth.ts before editing login behavior" \
    --token-budget 1600 \
    --force-index \
    --backend-evidence "$evidence" >"$route_json"

  require_jq "$route_json" '.routing_decision.backend_evidence.provider == "codebase-memory-mcp"' "agent_route should preserve backend evidence"
  require_jq "$route_json" '.routing_decision.backend_evidence.evidence_count == 10 and .routing_decision.backend_evidence.latency_ms == 53' "agent_route should combine exact snippet and resolved trace_path evidence"
  require_jq "$route_json" '.routing_decision.backend_evidence.evidence_sources | index("trace_path")' "agent_route should expose trace_path as a backend evidence source"
  require_jq "$route_json" '.routing_decision.backend_evidence.tool_results == null' "agent_route should consume raw trace_path evidence"
  require_jq "$route_json" '.routing_decision.first_file == "src/auth.ts"' "local route should select auth seed file"
  require_jq "$route_json" '.routing_decision.route_quality.evidence_sources | index("backend:codebase-memory-mcp:search_graph")' "route quality should expose backend search_graph evidence"
  require_jq "$route_json" 'any(.routing_decision.route_quality.confidence_factors[]; contains("backend codebase-memory-mcp independently selected the same first file"))' "route quality should record backend agreement"
  require_jq "$route_json" 'any(.routing_decision.route_quality.verification_steps[]; contains("Treat backend codebase-memory-mcp evidence as advisory"))' "route quality should keep backend evidence advisory"

  "$ROOT_DIR/scripts/codebase-memory-backend-evidence.sh" \
    --root "$repo" \
    --trace-path-json "$TEMP_DIR/trace-path.json" \
    --output "$trace_only_evidence"

  require_jq "$trace_only_evidence" '.candidate_files == [] and .candidates == [] and .tool_results.trace_path.function == "AuthService.login"' "trace-only bridge evidence should defer symbol resolution to agent_route"

  "$CODEINSIGHT_BIN" agent-route "$repo" \
    --task "understand login call chain" \
    --token-budget 1600 \
    --force-index \
    --backend-evidence "$trace_only_evidence" \
    --prefer-backend-context >"$trace_only_route_json"

  require_jq "$trace_only_route_json" '.routing_decision.seed_strategy == "backend_preferred" and .routing_decision.first_file == "src/auth.ts"' "trace-only evidence should seed preferred context from the traced subject"
  require_jq "$trace_only_route_json" '.routing_decision.backend_evidence.candidate_files == ["src/auth.ts", "src/main.ts", "src/audit.ts"]' "trace-only evidence should resolve the subject, callers, and callees to indexed files"
  require_jq "$trace_only_route_json" '.routing_decision.backend_evidence.evidence_count == 3 and .routing_decision.backend_evidence.latency_ms == 5' "trace-only evidence should preserve resolved count and latency"

  "$CODEINSIGHT_BIN" agent-route "$repo" \
    --task "understand app entrypoint flow" \
    --token-budget 6000 \
    --force-index \
    --backend-evidence "$evidence" \
    --prefer-backend-context >"$preferred_route_json"

  require_jq "$preferred_route_json" '.routing_decision.seed_strategy == "backend_preferred"' "preferred backend evidence should seed bounded context"
  require_jq "$preferred_route_json" '.routing_decision.first_file == "src/auth.ts"' "preferred backend evidence should select the graph-ranked first candidate"
  require_jq "$preferred_route_json" '.routing_decision.backend_route_agreement.status == "backend_preferred"' "preferred backend evidence should be explicit in route agreement"
  require_jq "$preferred_route_json" '.routing_decision.backend_route_agreement.local_first_file == "src/main.ts"' "preferred routing should preserve the original local first candidate"
  require_jq "$preferred_route_json" '.routing_decision.backend_route_agreement.selected_context_file == "src/auth.ts"' "preferred routing should expose the selected backend context file"
  require_jq "$preferred_route_json" '.routing_decision.backend_route_agreement.selected_context_files == ["src/auth.ts", "src/audit.ts", "src/main.ts"]' "preferred routing should preserve graph-ranked candidates that fit the context budget"
  require_jq "$preferred_route_json" '.routing_decision.backend_route_agreement.candidate_dispositions | length == 3 and all(.[]; .context_status == "selected" and .context_reason == "selected_within_token_budget" and .next_action == "read_selected_context" and .symbol_status == "valid")' "preferred routing should make every selected backend candidate actionable"
  require_jq "$preferred_route_json" '.routing_decision.backend_route_agreement.next_candidate_continuation == null' "preferred routing should not invent a continuation when every backend candidate was selected"
  require_jq "$preferred_route_json" '.routing_decision.continuation_source == "context_pack" and .routing_decision.continuation_status == .context_pack.continuation_summary.status and .routing_decision.continuation_next_action == .context_pack.continuation_summary.next_action' "preferred routing should preserve the local continuation when no backend candidate remains"
  require_jq "$preferred_route_json" '([.context_pack.reading_plan[].file] | index("src/auth.ts")) < ([.context_pack.reading_plan[].file] | index("src/audit.ts")) and ([.context_pack.reading_plan[].file] | index("src/audit.ts")) < ([.context_pack.reading_plan[].file] | index("src/main.ts"))' "preferred reading plan should retain graph candidate order"
  require_jq "$preferred_route_json" '.impact_seed_files == ["src/audit.ts", "src/auth.ts", "src/main.ts"]' "preferred impact analysis should follow all selected backend files"
  require_jq "$preferred_route_json" '.impact_seed_symbols == ["AuthService.login", "auditLogin", "main"]' "preferred impact analysis should follow selected backend symbols"
  require_jq "$preferred_route_json" '.routing_decision.route_quality.recommended_action == "read_backend_seeded_context"' "preferred routing should direct the agent to backend-seeded context"

  "$CODEINSIGHT_BIN" agent-route "$repo" \
    --task "understand invalid local seed" \
    --file "does/not/exist.ts" \
    --token-budget 1600 \
    --force-index \
    --backend-evidence "$evidence" \
    --backend-fallback >"$fallback_route_json"

  require_jq "$fallback_route_json" '.routing_decision.backend_route_agreement.status == "backend_fallback"' "generated structured evidence should seed fallback routing"
  require_jq "$fallback_route_json" '.routing_decision.first_file == "src/auth.ts"' "fallback should use the first generated backend candidate"
  require_jq "$fallback_route_json" '.routing_decision.backend_selected_candidate.symbol == "AuthService.login"' "fallback should preserve the exact snippet symbol"
  require_jq "$fallback_route_json" '.impact_seed_symbols == ["AuthService.login"]' "fallback should reuse the exact snippet symbol for impact analysis"
  require_jq "$fallback_route_json" '.routing_decision.backend_route_agreement.candidate_dispositions[0].context_status == "selected" and .routing_decision.backend_route_agreement.candidate_dispositions[0].next_action == "read_selected_context" and ([.routing_decision.backend_route_agreement.candidate_dispositions[1:][]] | all(.[]; .context_status == "omitted" and .context_reason == "fallback_not_selected" and .next_action == "use_if_fallback_context_insufficient"))' "fallback routing should make selected and lower-ranked candidates actionable"
  require_jq "$fallback_route_json" '.routing_decision.backend_route_agreement.next_candidate_continuation.file == "src/audit.ts" and .routing_decision.backend_route_agreement.next_candidate_continuation.rank == 2 and .routing_decision.backend_route_agreement.next_candidate_continuation.symbol == "auditLogin" and .routing_decision.backend_route_agreement.next_candidate_continuation.context_reason == "fallback_not_selected" and .routing_decision.backend_route_agreement.next_candidate_continuation.next_action == "use_if_fallback_context_insufficient"' "fallback routing should expose the highest-ranked unselected backend candidate"
  require_jq "$fallback_route_json" '.routing_decision.backend_route_agreement.next_candidate_continuation.suggested_tool.tool == "context_pack" and .routing_decision.backend_route_agreement.next_candidate_continuation.suggested_tool.suggested_arguments.files == ["src/audit.ts"] and .routing_decision.backend_route_agreement.next_candidate_continuation.suggested_tool.suggested_arguments.symbols == ["auditLogin"] and .routing_decision.backend_route_agreement.next_candidate_continuation.suggested_tool.suggested_arguments.token_budget == 4000' "fallback continuation should be directly callable"
  require_jq "$fallback_route_json" '.routing_decision.backend_route_agreement.next_candidate_continuation.suggested_tool as $tool | any(.execution_plan[]; .action == "use_if_fallback_context_insufficient" and .status == "available_after_selected_context" and .files == ["src/audit.ts"] and .suggested_tool == $tool)' "fallback execution plan should surface the backend continuation"
  require_jq "$fallback_route_json" '.routing_decision.continuation_source == "backend_route_agreement" and .routing_decision.continuation_status == "backend_candidate_available" and .routing_decision.continuation_next_action == .routing_decision.backend_route_agreement.next_candidate_continuation.next_action and .routing_decision.continuation_next_action == .execution_plan[2].action' "fallback routing decision should mirror the effective backend continuation"

  jq 'del(.tool_results) | .candidate_files = ["src/main.ts"] | .candidates = [.candidates[] | select(.file == "src/main.ts")] | .notes += ["conflict fixture: backend preferred app entrypoint"]' \
    "$evidence" >"$conflict_evidence"

  "$CODEINSIGHT_BIN" agent-route "$repo" \
    --task "inspect src/auth.ts before editing login behavior" \
    --token-budget 1600 \
    --force-index \
    --backend-evidence "$conflict_evidence" >"$conflict_route_json"

  require_jq "$conflict_route_json" '.routing_decision.first_file == "src/auth.ts"' "conflict route should keep local auth seed file"
  require_jq "$conflict_route_json" '.routing_decision.route_quality.recommended_action == "compare_backend_route_before_edits"' "backend conflict should change recommended action"
  require_jq "$conflict_route_json" 'any(.routing_decision.route_quality.warnings[]; contains("Backend codebase-memory-mcp preferred src/main.ts"))' "backend conflict should create a warning"
  require_jq "$conflict_route_json" 'any(.routing_decision.route_quality.verification_steps[]; contains("Compare local route with backend codebase-memory-mcp candidate src/main.ts"))' "backend conflict should require route comparison"

  echo "codebase-memory backend evidence smoke passed"
  echo "evidence: $evidence"
  echo "route: $route_json"
  echo "preferred_route: $preferred_route_json"
  echo "fallback_route: $fallback_route_json"
  echo "conflict_route: $conflict_route_json"
  echo "trace_only_route: $trace_only_route_json"
}

main "$@"
