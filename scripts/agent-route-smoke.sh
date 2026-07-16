#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""

fail() {
  echo "agent-route smoke failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
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

create_fixture() {
  TEMP_DIR="$(mktemp -d)"
  mkdir -p "$TEMP_DIR/repo/src"

  cat >"$TEMP_DIR/repo/package.json" <<'EOF'
{
  "type": "module",
  "scripts": {
    "start": "tsx src/main.ts"
  }
}
EOF

  cat >"$TEMP_DIR/repo/src/main.ts" <<'EOF'
import { AuthService } from "./auth";

export function main() {
  const service = new AuthService();
  return service.login("demo-user");
}

main();
EOF

  cat >"$TEMP_DIR/repo/src/auth.ts" <<'EOF'
import { auditLogin } from "./audit";

export class AuthService {
  login(user: string) {
    return auditLogin(user);
  }
}
EOF

  cat >"$TEMP_DIR/repo/src/audit.ts" <<'EOF'
export function auditLogin(user: string) {
  return { user, status: "accepted" };
}
EOF
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

json_value() {
  local file="$1"
  local query="$2"
  jq -r "$query" "$file"
}

main() {
  require_command jq
  build_binary_if_needed
  create_fixture
  trap cleanup EXIT INT TERM

  local repo_dir="$TEMP_DIR/repo"
  local route_json="$TEMP_DIR/agent-route.json"

  "$CODEINSIGHT_BIN" agent-route "$repo_dir" \
    --task "understand auth entrypoint flow" \
    --token-budget 1600 \
    --force-index \
    --impact-limit 10 \
    --impact-depth 2 \
    --impact-evidence-limit 3 \
    >"$route_json"

  require_jq "$route_json" '.task == "understand auth entrypoint flow"' "task should round-trip"
  require_jq "$route_json" '.token_budget == 1600' "token budget should round-trip"
  require_jq "$route_json" '.route | map(.tool) == ["index_project", "project_overview", "context_pack", "impact_analysis"]' "route should run the first-read pipeline in order"
  require_jq "$route_json" 'all(.route[]; .status == "complete")' "all route steps should complete"
  require_jq "$route_json" '.index_report.indexed_files >= 3' "fixture should index source files"
  require_jq "$route_json" '.index_report.symbols >= 3' "fixture should index symbols"
  require_jq "$route_json" '(.index_report.errors | length) == 0' "fixture should index without errors"
  require_jq "$route_json" '.overview.entrypoints | length >= 1' "overview should find entrypoints"
  require_jq "$route_json" '.context_pack.files | length >= 1' "context_pack should select files"
  require_jq "$route_json" '.context_pack.reading_plan | length >= 1' "context_pack should include a reading plan"
  require_jq "$route_json" '.context_pack.reading_plan[0].next_action != null and .context_pack.reading_plan[0].next_action != ""' "reading plan should include next action"
  require_jq "$route_json" '.context_pack.budget.requested_token_budget == 1600' "context_pack should preserve requested budget"
  require_jq "$route_json" '.context_pack.budget.applied_token_budget == 1600' "context_pack should apply requested budget"
  require_jq "$route_json" '.impact_status == "complete"' "impact_analysis should run when context has a seed"
  require_jq "$route_json" '.impact_seed_files | length >= 1' "impact seeds should include selected context files"
  require_jq "$route_json" '.impact_analysis.format == "summary"' "impact_analysis should use summary format"
  require_jq "$route_json" '.impact_analysis.depth == 2' "impact_analysis should preserve requested depth"
  require_jq "$route_json" '.impact_analysis.evidence_limit == 3' "impact_analysis should preserve evidence limit"
  require_jq "$route_json" '.impact_analysis.impact_counts.impacted_files >= 1' "impact_analysis should report impacted files"
  require_jq "$route_json" '.impact_analysis.suggested_checks | length >= 1' "impact_analysis should suggest checks"

  echo "agent-route smoke passed"
  echo "root: $repo_dir"
  echo "indexed_files: $(json_value "$route_json" '.index_report.indexed_files')"
  echo "symbols: $(json_value "$route_json" '.index_report.symbols')"
  echo "entrypoints: $(json_value "$route_json" '.overview.entrypoints | length')"
  echo "selected_files: $(json_value "$route_json" '.context_pack.files | length')"
  echo "reading_plan_steps: $(json_value "$route_json" '.context_pack.reading_plan | length')"
  echo "impact_status: $(json_value "$route_json" '.impact_status')"
  echo "impacted_files: $(json_value "$route_json" '.impact_analysis.impact_counts.impacted_files')"
}

main "$@"
