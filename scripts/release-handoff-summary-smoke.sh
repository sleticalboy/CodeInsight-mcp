#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

fail() {
  echo "release handoff summary smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local evidence_json="$TEMP_DIR/release-evidence/v9.8.7.json"
  local verification_json="$TEMP_DIR/release-verification/v9.8.7.json"
  local handoff_md="$TEMP_DIR/release-handoff/v9.8.7.md"
  local handoff_json="$TEMP_DIR/release-handoff/v9.8.7.json"

  mkdir -p "$(dirname "$evidence_json")" "$(dirname "$verification_json")"
  cat >"$evidence_json" <<'EOF'
{
  "schema_version": 1,
  "tag": "v9.8.7",
  "branch": "main",
  "head_sha": "abc123",
  "repo": "sleticalboy/CodeInsight-mcp",
  "metadata": {
    "cargo": "9.8.7",
    "install": "v9.8.7",
    "changelog": "9.8.7 (2026-07-15)"
  },
  "ci": {
    "run_id": "123456",
    "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456"
  },
  "artifacts": {
    "benchmark": {
      "name": "codeinsight-benchmark-subset",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/1",
      "report": "/tmp/benchmark.md"
    },
    "context_pack_quality": {
      "name": "codeinsight-context-pack-quality",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/2",
      "summary": "/tmp/context-pack-quality.json"
    },
    "agent_route": {
      "name": "codeinsight-agent-route-smoke",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/3",
      "summary": "/tmp/agent-route.json"
    },
    "mcp_first_call": {
      "name": "codeinsight-mcp-first-call",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/4",
      "summary": "/tmp/mcp-first-call.json"
    }
  },
  "release_notes_block": "## v9.8.7 release evidence"
}
EOF

  cat >"$verification_json" <<'EOF'
{
  "status": "passed",
  "tag": "v9.8.7",
  "version": "9.8.7",
  "repo": "sleticalboy/CodeInsight-mcp",
  "gates": {
    "github_release": "passed",
    "github_asset_downloads": "metadata_only",
    "release_notes": "passed",
    "install_script": "passed",
    "installed_quickstart": "skipped",
    "docker": "skipped",
    "homebrew_remote_formula": "passed",
    "homebrew_fetch": "skipped"
  },
  "expected_assets": [
    "codeinsight-aarch64-apple-darwin.tar.gz",
    "codeinsight-x86_64-unknown-linux-gnu.tar.gz"
  ],
  "docker": {
    "image": "ghcr.io/sleticalboy/codeinsight-mcp",
    "skipped": true
  },
  "homebrew": {
    "tap": "sleticalboy/tap",
    "repo": "sleticalboy/homebrew-tap",
    "skipped": true
  },
  "installed_quickstart": {
    "binary": "/tmp/codeinsight",
    "skipped": true,
    "coverage": ["version", "index", "overview", "context-pack", "agent-route", "mcp_stdio", "mcp_agent_route", "agent_route_execution_plan", "reading_plan_reason", "selection_reason"]
  }
}
EOF

  "$ROOT_DIR/scripts/release-handoff-summary.sh" \
    --evidence-json "$evidence_json" \
    --verification-json "$verification_json" \
    v9.8.7 >"$TEMP_DIR/stdout.md"

  grep -Fq '## v9.8.7 release handoff' "$TEMP_DIR/stdout.md" ||
    fail "missing handoff heading"
  grep -Fq -- '- Status: `passed`' "$TEMP_DIR/stdout.md" ||
    fail "missing status"
  grep -Fq -- '- Target commit: `abc123`' "$TEMP_DIR/stdout.md" ||
    fail "missing target commit"
  grep -Fq -- '- Pre-release CI: [run 123456](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456)' "$TEMP_DIR/stdout.md" ||
    fail "missing pre-release CI"
  grep -Fq -- '- `github_asset_downloads`: `metadata_only`' "$TEMP_DIR/stdout.md" ||
    fail "missing release gate"
  grep -Fq -- '- Agent-route artifact: [codeinsight-agent-route-smoke](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/3)' "$TEMP_DIR/stdout.md" ||
    fail "missing agent-route artifact"
  grep -Fq -- '- MCP first-call artifact: [codeinsight-mcp-first-call](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/4)' "$TEMP_DIR/stdout.md" ||
    fail "missing MCP first-call artifact"

  "$ROOT_DIR/scripts/release-handoff-summary.sh" \
    --evidence-json "$evidence_json" \
    --verification-json "$verification_json" \
    --output "$handoff_md" \
    --json-output "$handoff_json" \
    9.8.7 >"$TEMP_DIR/output.log"

  grep -Fq "release handoff summary written: $handoff_md" "$TEMP_DIR/output.log" ||
    fail "missing Markdown output diagnostic"
  grep -Fq "release handoff JSON written: $handoff_json" "$TEMP_DIR/output.log" ||
    fail "missing JSON output diagnostic"
  grep -Fq '## v9.8.7 release handoff' "$handoff_md" ||
    fail "missing handoff output file"
  jq -e '
    .schema_version == 1 and
    .tag == "v9.8.7" and
    .status == "passed" and
    .target_commit == "abc123" and
    .pre_release.ci.run_id == "123456" and
    .post_release.gates.github_asset_downloads == "metadata_only" and
    (.post_release.expected_assets | length) == 2 and
    (.handoff_markdown | contains("## v9.8.7 release handoff"))
  ' "$handoff_json" >/dev/null ||
    fail "invalid handoff JSON"

  if "$ROOT_DIR/scripts/release-handoff-summary.sh" \
    --evidence-json "$evidence_json" \
    --verification-json "$verification_json" \
    v9.8.8 >"$TEMP_DIR/mismatch.out" 2>"$TEMP_DIR/mismatch.err"; then
    fail "tag mismatch should fail"
  fi
  grep -Fq 'evidence tag v9.8.7 does not match v9.8.8' "$TEMP_DIR/mismatch.err" ||
    fail "missing tag mismatch diagnostic"

  echo "release handoff summary smoke passed"
}

main "$@"
