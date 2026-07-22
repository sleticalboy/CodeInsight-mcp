#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TOTAL=25
TEMP_FILES=()

source "$ROOT_DIR/scripts/smoke-lib.sh"

cleanup() {
  local file

  for file in ${TEMP_FILES[@]+"${TEMP_FILES[@]}"}; do
    rm -f "$file"
  done
}

context_pack_quality_smoke() {
  local summary_json

  summary_json="$(mktemp "${TMPDIR:-/tmp}/codeinsight-context-pack-quality.XXXXXX")"
  TEMP_FILES+=("$summary_json")

  scripts/context-pack-quality-smoke.sh --summary-json "$summary_json"
  jq -e \
    '.status == "pass"
      and .scenarios_passed == 10
      and (.scenarios | length) == 10
      and .question_checks_passed == 10
      and (.question_checks | length) == 10
      and all(.scenarios[]; .status == "pass")
      and (.scenarios[] | select(.name == "budget_continuation"))
      and (.scenarios[] | select(.name == "minimum_budget"))
      and (.scenarios[] | select(.name == "token_exhaustion"))
      and (.scenarios[] | select(.name == "task_aware_question_coverage"))
      and (.scenarios[] | select(.name == "core_analysis_question_coverage"))
      and (.question_checks[] | select(.name == "seed_file_auth_question"))
      and (.question_checks[] | select(.name == "call_graph_auth_question"))
      and (.question_checks[] | select(.name == "reference_auth_question"))
      and (.question_checks[] | select(.name == "dependency_auth_question"))
      and (.question_checks[] | select(.name == "semantic_session_cookie_question"))
      and (.question_checks[] | select(.name == "core_indexing_pipeline_question"))
      and (.question_checks[] | select(.name == "core_dependency_graph_question"))
      and (.question_checks[] | select(.name == "core_semantic_fallback_question"))
      and (.question_checks[] | select(.name == "core_find_references_question"))
      and (.question_checks[] | select(.name == "core_call_graph_traversal_question"))' \
    "$summary_json" >/dev/null
}

main() {
  trap cleanup EXIT INT TERM
  cd "$ROOT_DIR"

  smoke_run_step "$SMOKE_TOTAL" 1 "cargo fmt" cargo fmt --check
  smoke_run_step "$SMOKE_TOTAL" 2 "cargo test" cargo test --locked
  smoke_run_step "$SMOKE_TOTAL" 3 "clippy smoke" scripts/clippy-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 4 "script syntax smoke" scripts/script-syntax-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 5 "workflow actions smoke" scripts/workflow-actions-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 6 "benchmark step summary smoke" scripts/benchmark-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 7 "benchmark summary text smoke" scripts/benchmark-summary-text-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 8 "benchmark reuse checkout smoke" scripts/benchmark-reuse-checkout-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 9 "benchmark local smoke" scripts/benchmark-local-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 10 "context pack quality step summary smoke" scripts/context-pack-quality-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 11 "agent route step summary smoke" scripts/agent-route-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 12 "MCP first-call step summary smoke" scripts/mcp-first-call-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 13 "public task routing matrix step summary smoke" scripts/public-task-routing-matrix-step-summary-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 14 "release tooling smoke" scripts/release-tooling-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 15 "docs smoke" scripts/docs-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 16 "context pack quality smoke" context_pack_quality_smoke
  smoke_run_step "$SMOKE_TOTAL" 17 "agent route smoke" scripts/agent-route-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 18 "MCP first-call smoke" scripts/mcp-first-call-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 19 "MCP first-call failure smoke" scripts/mcp-first-call-failure-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 20 "agent router demo" scripts/agent-router-demo.sh
  smoke_run_step "$SMOKE_TOTAL" 21 "framework entrypoint demo" scripts/framework-entrypoint-demo.sh
  smoke_run_step "$SMOKE_TOTAL" 22 "task routing matrix smoke" scripts/task-routing-matrix-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 23 "public task routing matrix smoke" scripts/public-task-routing-matrix-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 24 "update public task routing matrix smoke" scripts/update-public-task-routing-matrix-smoke.sh
  smoke_run_step "$SMOKE_TOTAL" 25 "git diff whitespace check" git diff --check

  echo "local CI smoke passed"
}

main "$@"
