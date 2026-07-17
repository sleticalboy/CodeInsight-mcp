#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CODEINSIGHT_BIN="${CODEINSIGHT_BIN:-}"
TEMP_DIR=""
SUMMARY_JSON=""

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

usage() {
  cat <<'EOF'
usage: scripts/agent-route-smoke.sh [--summary-json PATH]

Options:
  --summary-json PATH  Write a machine-readable agent-route evidence summary.
  -h, --help           Show this help text.
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --summary-json)
        if [ "$#" -lt 2 ]; then
          fail "--summary-json requires a path"
        fi
        SUMMARY_JSON="$2"
        shift 2
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        fail "unknown argument: $1"
        ;;
    esac
  done
}

write_summary_json() {
  local route_json="$1"

  if [ -z "$SUMMARY_JSON" ]; then
    return
  fi

  mkdir -p "$(dirname "$SUMMARY_JSON")"
  jq '{
    status: "pass",
    task,
    token_budget,
    route_tools: [.route[].tool],
    execution_plan_actions: [.execution_plan[].action],
    metrics: {
      indexed_files: .index_report.indexed_files,
      symbols: .index_report.symbols,
      index_errors: (.index_report.errors | length),
      entrypoints: (.overview.entrypoints | length),
      selected_files: (.context_pack.files | length),
      selected_ranges: .context_pack.budget.selected_ranges,
      reading_plan_steps: (.context_pack.reading_plan | length),
      execution_plan_steps: (.execution_plan | length),
      requested_token_budget: .context_pack.budget.requested_token_budget,
      applied_token_budget: .context_pack.budget.applied_token_budget,
      seed_strategy: (.context_pack.seed_strategy // ""),
      selected_seed_count: (.context_pack.selected_seeds | length),
      first_seed_source: (.context_pack.selected_seeds[0].source // ""),
      first_seed_value: (.context_pack.selected_seeds[0].value // ""),
      companion_entrypoint: (([.context_pack.selected_seeds[1:][]? | select(.source == "overview_entrypoint") | .value] | first) // ""),
      first_context_file: (.context_pack.files[0].file // ""),
      first_reading_file: (.context_pack.reading_plan[0].file // ""),
      first_execution_action: (.execution_plan[0].action // ""),
      second_execution_action: (.execution_plan[1].action // ""),
      first_execution_suggested_tool: (.execution_plan[1].suggested_tool.tool // ""),
      first_next_action: (.context_pack.reading_plan[0].next_action // ""),
      first_reading_question: (.context_pack.reading_plan[0].question // ""),
      context_route_reason: (.route[] | select(.tool == "context_pack") | .reason),
      impact_route_reason: (.route[] | select(.tool == "impact_analysis") | .reason),
      impact_status,
      impacted_files: (.impact_analysis.impact_counts.impacted_files // 0),
      suggested_checks: (.impact_analysis.suggested_checks | length)
    }
  }' "$route_json" >"$SUMMARY_JSON"

  require_jq "$SUMMARY_JSON" \
    '.status == "pass"
      and .route_tools == ["index_project", "project_overview", "context_pack", "impact_analysis"]
      and .execution_plan_actions == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]
      and .metrics.indexed_files >= 3
      and .metrics.index_errors == 0
      and .metrics.reading_plan_steps >= 1
      and .metrics.execution_plan_steps == 4
      and (.metrics.seed_strategy | type == "string" and length > 0)
      and (.metrics.selected_seed_count | type == "number")
      and (.metrics.first_seed_source | type == "string")
      and (.metrics.companion_entrypoint | type == "string")
      and .metrics.first_execution_action == "read_selected_context"
      and .metrics.second_execution_action == "use_current_reading_step_suggested_tool"
      and (.metrics.first_execution_suggested_tool | type == "string" and length > 0)
      and (.metrics.first_reading_question | type == "string" and length > 0)
      and .metrics.impact_status == "complete"
      and .metrics.impacted_files >= 1' \
    "summary JSON should match the agent-route evidence contract"
}

create_task_focused_fixture() {
  local repo_dir="$1"

  mkdir -p "$repo_dir/src"
  cat >"$repo_dir/package.json" <<'EOF'
{
  "type": "module",
  "scripts": {
    "start": "tsx src/main.ts"
  }
}
EOF

  cat >"$repo_dir/src/main.ts" <<'EOF'
import { bootRouter } from "./router";

export function main() {
  return bootRouter();
}

main();
EOF

  cat >"$repo_dir/src/router.ts" <<'EOF'
import { authenticate } from "./auth";

export function bootRouter() {
  return authenticate("demo-user");
}
EOF

  cat >"$repo_dir/src/auth.ts" <<'EOF'
export function authenticate(user: string) {
  return { user, status: "accepted" };
}
EOF
}

create_noisy_entrypoint_fixture() {
  local repo_dir="$1"

  mkdir -p "$repo_dir/src"
  cat >"$repo_dir/package.json" <<'EOF'
{
  "type": "module",
  "description": "auth entrypoint router bootstrap",
  "scripts": {
    "start": "tsx src/main.ts"
  }
}
EOF

  cat >"$repo_dir/README.md" <<'EOF'
# Auth entrypoint router bootstrap

This document repeatedly mentions auth entrypoint router bootstrap main server
start flow, but it is not executable source and should not be the first read.
EOF

  cat >"$repo_dir/src/main.ts" <<'EOF'
import { bootRouter } from "./router";

export function main() {
  return bootRouter();
}

main();
EOF

  cat >"$repo_dir/src/router.ts" <<'EOF'
import { authenticate } from "./auth";

export function bootRouter() {
  return authenticate("demo-user");
}
EOF

  cat >"$repo_dir/src/auth.ts" <<'EOF'
export function authenticate(user: string) {
  return { user, status: "accepted" };
}
EOF
}

main() {
  parse_args "$@"
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
  require_jq "$route_json" '.execution_plan | map(.action) == ["read_selected_context", "use_current_reading_step_suggested_tool", "use_continuation_if_needed", "review_impact_before_edits"]' "execution plan should describe the client follow-up path"
  require_jq "$route_json" '.execution_plan[0].status == "ready" and (.execution_plan[0].files | length >= 1)' "execution plan should start by reading selected context"
  require_jq "$route_json" '.execution_plan[1].suggested_tool.tool != null and .execution_plan[1].suggested_tool.tool != ""' "execution plan should expose the current-step suggested tool"
  require_jq "$route_json" 'all(.route[]; .status == "complete")' "all route steps should complete"
  require_jq "$route_json" '.index_report.indexed_files >= 3' "fixture should index source files"
  require_jq "$route_json" '.index_report.symbols >= 3' "fixture should index symbols"
  require_jq "$route_json" '(.index_report.errors | length) == 0' "fixture should index without errors"
  require_jq "$route_json" '.overview.entrypoints | length >= 1' "overview should find entrypoints"
  require_jq "$route_json" '.context_pack.files | length >= 1' "context_pack should select files"
  require_jq "$route_json" '.context_pack.reading_plan | length >= 1' "context_pack should include a reading plan"
  require_jq "$route_json" '.context_pack.reading_plan[0].next_action != null and .context_pack.reading_plan[0].next_action != ""' "reading plan should include next action"
  require_jq "$route_json" '.context_pack.reading_plan[0].question != null and .context_pack.reading_plan[0].question != ""' "reading plan should include a question"
  require_jq "$route_json" '.context_pack.budget.requested_token_budget == 1600' "context_pack should preserve requested budget"
  require_jq "$route_json" '.context_pack.budget.applied_token_budget == 1600' "context_pack should apply requested budget"
  require_jq "$route_json" '.route[2].reason as $reason | .context_pack.reading_plan[0] as $step | $reason | contains("read \($step.file) first") and contains($step.next_action) and contains($step.suggested_tool.tool) and contains("follow continuation")' "context route step should explain the first read path"
  require_jq "$route_json" '.route[3].reason | contains("pre-edit impact check")' "impact route step should frame impact analysis as a pre-edit check"
  require_jq "$route_json" '.route[3].reason | contains("call-related files") and contains("dependency-related files") and contains("call paths") and contains("dependency paths")' "impact route step should summarize the impact breakdown"
  require_jq "$route_json" '.impact_status == "complete"' "impact_analysis should run when context has a seed"
  require_jq "$route_json" '.impact_seed_files | length >= 1' "impact seeds should include selected context files"
  require_jq "$route_json" '.impact_analysis.format == "summary"' "impact_analysis should use summary format"
  require_jq "$route_json" '.impact_analysis.depth == 2' "impact_analysis should preserve requested depth"
  require_jq "$route_json" '.impact_analysis.evidence_limit == 3' "impact_analysis should preserve evidence limit"
  require_jq "$route_json" '.impact_analysis.impact_counts.impacted_files >= 1' "impact_analysis should report impacted files"
  require_jq "$route_json" '.impact_analysis.suggested_checks | length >= 1' "impact_analysis should suggest checks"
  write_summary_json "$route_json"

  local focused_repo="$TEMP_DIR/task-focused-repo"
  local focused_route_json="$TEMP_DIR/task-focused-agent-route.json"
  create_task_focused_fixture "$focused_repo"
  "$CODEINSIGHT_BIN" agent-route "$focused_repo" \
    --task "understand router auth flow" \
    --token-budget 1600 \
    --force-index \
    >"$focused_route_json"

  require_jq "$focused_route_json" '.context_pack.seed_strategy == "auto_task_match"' \
    "task-focused route should report task-match seed strategy"
  require_jq "$focused_route_json" '.context_pack.selected_seeds[0].value == "src/router.ts"' \
    "task keywords should choose router seed before generic main entrypoint"
  require_jq "$focused_route_json" '.context_pack.selected_seeds[0].matched_keywords == ["router"]' \
    "task-focused seed should explain matched keywords"
  require_jq "$focused_route_json" '.context_pack.selected_seeds[1].value == "src/main.ts"' \
    "task-focused route should keep the main entrypoint as a companion seed"
  require_jq "$focused_route_json" '.context_pack.selected_seeds[1].source == "overview_entrypoint"' \
    "task-focused companion seed should come from overview entrypoints"
  require_jq "$focused_route_json" '.context_pack.files[0].file == "src/router.ts"' \
    "task-focused route should read router first"
  require_jq "$focused_route_json" '[.context_pack.files[].file] | index("src/main.ts")' \
    "task-focused route should retain main entrypoint context"
  require_jq "$focused_route_json" '.context_pack.files[0].reason | contains("matched task keywords: router")' \
    "task-focused file reason should explain matched task keywords"
  require_jq "$focused_route_json" '.context_pack.reading_plan[0].file == "src/router.ts"' \
    "task-focused reading plan should start with router"
  require_jq "$focused_route_json" '[.context_pack.reading_plan[].file] | index("src/main.ts")' \
    "task-focused reading plan should include main entrypoint context"
  require_jq "$focused_route_json" '[.execution_plan[0].files[]] | index("src/main.ts")' \
    "task-focused execution plan should include main entrypoint context"

  local noisy_repo="$TEMP_DIR/noisy-entrypoint-repo"
  local noisy_route_json="$TEMP_DIR/noisy-entrypoint-agent-route.json"
  create_noisy_entrypoint_fixture "$noisy_repo"
  "$CODEINSIGHT_BIN" agent-route "$noisy_repo" \
    --task "understand main application entrypoint" \
    --token-budget 1600 \
    --force-index \
    >"$noisy_route_json"

  require_jq "$noisy_route_json" '.overview.entrypoints[0].file == "src/main.ts"' \
    "noisy entrypoint route should detect src/main.ts as the top entrypoint"
  require_jq "$noisy_route_json" '.context_pack.seed_strategy == "auto_entrypoint"' \
    "noisy entrypoint route should use auto_entrypoint strategy"
  require_jq "$noisy_route_json" '.context_pack.selected_seeds[0].value == "src/main.ts"' \
    "noisy entrypoint route should seed from src/main.ts"
  require_jq "$noisy_route_json" '.context_pack.selected_seeds[0].source == "overview_entrypoint"' \
    "noisy entrypoint route should seed from overview entrypoint"
  require_jq "$noisy_route_json" '.context_pack.files[0].file == "src/main.ts"' \
    "noisy entrypoint route should read src/main.ts first"
  require_jq "$noisy_route_json" '.context_pack.reading_plan[0].file == "src/main.ts"' \
    "noisy entrypoint reading plan should start with src/main.ts"
  require_jq "$noisy_route_json" '.execution_plan[0].files[0] == "src/main.ts"' \
    "noisy entrypoint execution plan should start with src/main.ts"
  require_jq "$noisy_route_json" '[.context_pack.files[].file] | index("README.md") | not' \
    "noisy entrypoint route should not select README as first-read context"

  echo "agent-route smoke passed"
  echo "root: $repo_dir"
  echo "indexed_files: $(json_value "$route_json" '.index_report.indexed_files')"
  echo "symbols: $(json_value "$route_json" '.index_report.symbols')"
  echo "entrypoints: $(json_value "$route_json" '.overview.entrypoints | length')"
  echo "selected_files: $(json_value "$route_json" '.context_pack.files | length')"
  echo "reading_plan_steps: $(json_value "$route_json" '.context_pack.reading_plan | length')"
  echo "execution_plan_steps: $(json_value "$route_json" '.execution_plan | length')"
  echo "first_execution_action: $(json_value "$route_json" '.execution_plan[0].action')"
  echo "first_context_file: $(json_value "$route_json" '.context_pack.files[0].file')"
  echo "first_reading_file: $(json_value "$route_json" '.context_pack.reading_plan[0].file')"
  echo "first_reading_question: $(json_value "$route_json" '.context_pack.reading_plan[0].question')"
  echo "noisy_entrypoint_first_file: $(json_value "$noisy_route_json" '.context_pack.files[0].file')"
  echo "impact_status: $(json_value "$route_json" '.impact_status')"
  echo "impacted_files: $(json_value "$route_json" '.impact_analysis.impact_counts.impacted_files')"
}

main "$@"
