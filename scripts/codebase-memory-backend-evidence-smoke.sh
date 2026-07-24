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
}

write_codebase_memory_exports() {
  local repo="$1"
  local output_dir="$2"

  cat >"$output_dir/search-graph.json" <<EOF
{
  "total": 2,
  "search_mode": "bm25",
  "elapsed_ms": 7,
  "results": [
    {
      "name": "AuthService",
      "qualified_name": "fixture.src.auth.AuthService",
      "label": "Class",
      "file_path": "$repo/src/auth.ts"
    },
    {
      "name": "auditLogin",
      "qualified_name": "fixture.src.audit.auditLogin",
      "label": "Function",
      "file_path": "src/audit.ts"
    }
  ]
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

  local repo evidence route_json
  repo="$TEMP_DIR/repo"
  evidence="$TEMP_DIR/backend-evidence.json"
  route_json="$TEMP_DIR/agent-route.json"

  create_fixture "$repo"
  write_codebase_memory_exports "$repo" "$TEMP_DIR"

  "$ROOT_DIR/scripts/codebase-memory-backend-evidence.sh" \
    --root "$repo" \
    --search-graph-json "$TEMP_DIR/search-graph.json" \
    --search-code-json "$TEMP_DIR/search-code.json" \
    --architecture-json "$TEMP_DIR/architecture.json" \
    --candidate-limit 3 \
    --confidence 0.86 \
    --note "smoke fixture for backend evidence bridge" \
    --output "$evidence"

  require_jq "$evidence" '.provider == "codebase-memory-mcp"' "provider should be codebase-memory-mcp"
  require_jq "$evidence" '.candidate_files == ["src/auth.ts", "src/audit.ts", "src/main.ts"]' "candidate files should be normalized and stable"
  require_jq "$evidence" '.evidence_sources | index("search_graph") and index("search_code") and index("get_architecture:entry_points")' "evidence sources should include all bridge inputs"
  require_jq "$evidence" '.evidence_count == 5' "evidence count should include duplicate backend signals"
  require_jq "$evidence" '.latency_ms == 33' "latency should aggregate exported backend timings"
  require_jq "$evidence" '.confidence == 0.86' "confidence should be preserved"

  "$CODEINSIGHT_BIN" agent-route "$repo" \
    --task "inspect src/auth.ts before editing login behavior" \
    --token-budget 1600 \
    --force-index \
    --backend-evidence "$evidence" >"$route_json"

  require_jq "$route_json" '.routing_decision.backend_evidence.provider == "codebase-memory-mcp"' "agent_route should preserve backend evidence"
  require_jq "$route_json" '.routing_decision.first_file == "src/auth.ts"' "local route should select auth seed file"
  require_jq "$route_json" '.routing_decision.route_quality.evidence_sources | index("backend:codebase-memory-mcp:search_graph")' "route quality should expose backend search_graph evidence"
  require_jq "$route_json" 'any(.routing_decision.route_quality.confidence_factors[]; contains("backend codebase-memory-mcp independently selected the same first file"))' "route quality should record backend agreement"
  require_jq "$route_json" 'any(.routing_decision.route_quality.verification_steps[]; contains("Treat backend codebase-memory-mcp evidence as advisory"))' "route quality should keep backend evidence advisory"

  echo "codebase-memory backend evidence smoke passed"
  echo "evidence: $evidence"
  echo "route: $route_json"
}

main "$@"
