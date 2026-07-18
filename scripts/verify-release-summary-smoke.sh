#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SMOKE_TEMP_DIR=""

cleanup() {
  if [ -n "$SMOKE_TEMP_DIR" ]; then
    rm -rf "$SMOKE_TEMP_DIR"
  fi
}

main() {
  SMOKE_TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  export CODEINSIGHT_VERIFY_RELEASE_NO_MAIN=1
  export CODEINSIGHT_SKIP_DOCKER=1
  export CODEINSIGHT_SKIP_HOMEBREW=1

  # shellcheck disable=SC1091
  source "$ROOT_DIR/scripts/verify-release.sh"

  release_verification_summary_json v9.8.7 9.8.7 >"$SMOKE_TEMP_DIR/summary.json"

  jq -e '
    .status == "passed" and
    .tag == "v9.8.7" and
    .version == "9.8.7" and
    .repo == "sleticalboy/CodeInsight-mcp" and
    .gates.github_release == "passed" and
    .gates.github_asset_downloads == "passed" and
    .gates.install_script == "passed" and
    .gates.installed_quickstart == "passed" and
    .gates.docker == "skipped" and
    .gates.homebrew_fetch == "skipped" and
    .docker.skipped == true and
    .homebrew.skipped == true and
    .installed_quickstart.skipped == false and
    (.installed_quickstart.coverage | index("agent-route")) and
    (.installed_quickstart.coverage | index("mcp_agent_route")) and
    (.installed_quickstart.coverage | index("agent_route_execution_plan")) and
    (.installed_quickstart.coverage | index("reading_plan_question")) and
    (.installed_quickstart.coverage | index("reading_plan_reason")) and
    (.installed_quickstart.coverage | index("selection_reason")) and
    (.installed_quickstart.coverage | index("selection_rank")) and
    (.installed_quickstart.coverage | index("continuation_evidence")) and
    (.expected_assets | length) == 4
  ' "$SMOKE_TEMP_DIR/summary.json" >/dev/null

  CODEINSIGHT_SKIP_INSTALLED_QUICKSTART=1 \
    release_verification_summary_json v9.8.7 9.8.7 >"$SMOKE_TEMP_DIR/skipped-summary.json"

  jq -e '
    .gates.installed_quickstart == "skipped" and
    .installed_quickstart.skipped == true and
    (.installed_quickstart.coverage | index("agent-route")) and
    (.installed_quickstart.coverage | index("mcp_agent_route")) and
    (.installed_quickstart.coverage | index("agent_route_execution_plan")) and
    (.installed_quickstart.coverage | index("reading_plan_question")) and
    (.installed_quickstart.coverage | index("reading_plan_reason")) and
    (.installed_quickstart.coverage | index("selection_reason")) and
    (.installed_quickstart.coverage | index("selection_rank")) and
    (.installed_quickstart.coverage | index("continuation_evidence"))
  ' "$SMOKE_TEMP_DIR/skipped-summary.json" >/dev/null

  echo "verify-release summary smoke passed"
}

main "$@"
