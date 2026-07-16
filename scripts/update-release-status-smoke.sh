#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMP_DIR=""

cleanup() {
  if [ -n "$TEMP_DIR" ]; then
    rm -rf "$TEMP_DIR"
  fi
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local status_doc="$TEMP_DIR/status.md"
  local fallback_status_doc="$TEMP_DIR/status-md.md"
  local summary_json="$TEMP_DIR/summary.json"
  local evidence_json_file="$TEMP_DIR/release-evidence/v9.8.7.json"
  local evidence_md_file="$TEMP_DIR/release-evidence/v9.8.7.md"

  cat >"$status_doc" <<'EOF'
# Current Status

## Latest Verified Release

`v0.1.0` was published and verified on 2026-07-01.

## Current Release Tooling State

Existing tooling notes.
EOF
  cp "$status_doc" "$fallback_status_doc"

  cat >"$summary_json" <<'EOF'
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
    "coverage": ["version", "index", "overview", "context-pack", "agent-route", "mcp_stdio", "mcp_agent_route", "reading_plan_reason", "selection_reason"]
  }
}
EOF

  mkdir -p "$(dirname "$evidence_json_file")"
  cat >"$evidence_json_file" <<'EOF'
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
    }
  },
  "release_notes_block": "## v9.8.7 release evidence"
}
EOF
  cat >"$evidence_md_file" <<'EOF'
release evidence summary
tag: v9.8.7
branch: main
head_sha: md456
metadata_cargo: 9.8.7
metadata_install: v9.8.7
metadata_changelog: 9.8.7 (2026-07-15)
ci_run: 654321
benchmark_artifact_url: https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/654321/artifacts/1
context_pack_quality_artifact_url: https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/654321/artifacts/2
agent_route_artifact_url: https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/654321/artifacts/3
EOF

  CODEINSIGHT_STATUS_DATE=2026-07-14 \
    "$ROOT_DIR/scripts/update-release-status.sh" --evidence-json-file "$evidence_json_file" "$summary_json" "$status_doc" \
    >"$TEMP_DIR/update.out"
  CODEINSIGHT_STATUS_DATE=2026-07-15 \
    "$ROOT_DIR/scripts/update-release-status.sh" --evidence-json-file "$evidence_json_file" "$summary_json" "$status_doc" \
    >"$TEMP_DIR/update-again.out"

  test "$(grep -c '<!-- release-verification-summary:start -->' "$status_doc")" -eq 1
  test "$(grep -c '<!-- release-verification-summary:end -->' "$status_doc")" -eq 1
  grep -q 'Generated from `scripts/verify-release.sh --json` on 2026-07-15.' "$status_doc"
  grep -q -- '- Tag: `v9.8.7`' "$status_doc"
  grep -q -- '  - `github_asset_downloads`: `metadata-only`' "$status_doc"
  grep -q -- '  - `installed_quickstart`: `skipped`' "$status_doc"
  grep -q -- '- Docker image: `ghcr.io/sleticalboy/codeinsight-mcp` (skipped locally)' "$status_doc"
  grep -q -- '- Homebrew tap: `sleticalboy/tap` (skipped locally)' "$status_doc"
  grep -q -- '- Installed quickstart binary: `/tmp/codeinsight` (skipped locally)' "$status_doc"
  grep -q -- '- Installed quickstart coverage: `version`, `index`, `overview`, `context-pack`, `agent-route`, `mcp_stdio`, `mcp_agent_route`, `reading_plan_reason`, `selection_reason`' "$status_doc"
  grep -q -- '- Pre-release evidence:' "$status_doc"
  grep -q -- "  - Evidence file: \`$evidence_json_file\`" "$status_doc"
  grep -q -- '  - Target commit: `abc123`' "$status_doc"
  grep -q -- '  - CI run: `123456`' "$status_doc"
  grep -q -- '  - Metadata: `cargo=9.8.7`, `install=v9.8.7`, `changelog=9.8.7 (2026-07-15)`' "$status_doc"
  grep -q -- '  - Agent-route artifact: \[codeinsight-agent-route-smoke\](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/3)' "$status_doc"
  grep -q '## Current Release Tooling State' "$status_doc"

  CODEINSIGHT_STATUS_DATE=2026-07-15 \
    "$ROOT_DIR/scripts/update-release-status.sh" --evidence-file "$evidence_md_file" "$summary_json" "$fallback_status_doc" \
    >"$TEMP_DIR/update-md.out"
  grep -q -- "  - Evidence file: \`$evidence_md_file\`" "$fallback_status_doc"
  grep -q -- '  - Target commit: `md456`' "$fallback_status_doc"
  grep -q -- '  - CI run: `654321`' "$fallback_status_doc"

  echo "update release status smoke passed"
}

main "$@"
