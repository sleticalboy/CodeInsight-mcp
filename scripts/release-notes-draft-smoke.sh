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
  echo "release notes draft smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  local handoff_json="$TEMP_DIR/release-handoff/v9.8.7.json"
  local changelog_notes="$TEMP_DIR/changelog-notes.md"
  local draft_md="$TEMP_DIR/release-notes-draft/v9.8.7.md"

  mkdir -p "$(dirname "$handoff_json")"
  cat >"$handoff_json" <<'EOF'
{
  "schema_version": 1,
  "tag": "v9.8.7",
  "status": "passed",
  "version": "9.8.7",
  "repo": "sleticalboy/CodeInsight-mcp",
  "target_commit": "abc123",
  "evidence_json": "release-evidence/v9.8.7.json",
  "verification_json": "release-verification/v9.8.7.json",
  "pre_release": {
    "branch": "main",
    "ci": {
      "run_id": "123456",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456"
    },
    "metadata": {
      "cargo": "9.8.7",
      "install": "v9.8.7",
      "changelog": "9.8.7 (2026-07-15)"
    },
    "artifacts": {
      "benchmark": {
        "name": "codeinsight-benchmark-subset",
        "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/1",
        "metrics": {
          "context_pack_first": 1,
          "routing_total": 1,
          "line_reduction": "99.0%",
          "guardrail_failures": 0,
          "truncated_packs": 0
        }
      },
      "context_pack_quality": {
        "name": "codeinsight-context-pack-quality",
        "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/2"
      },
      "agent_route": {
        "name": "codeinsight-agent-route-smoke",
        "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/3",
        "metrics": {
          "first_selection_rank": 1,
          "first_selection_reason": "Selected for high relevance via seed_file: Seed file header and imports for task: src/auth.ts",
          "continuation_status": "lower_ranked_context_omitted",
          "continuation_next_action": "narrow_task_or_seed"
        }
      },
      "adoption_report": {
        "name": "CodeInsight self adoption report",
        "document": "docs/adoption-report-codeinsight.md",
        "command": "scripts/adoption-report.sh . --task \"understand the main application entrypoint\" --token-budget 6000 --output-dir /tmp/codeinsight-self-adoption-report --archive /tmp/codeinsight-self-adoption-report.tar.gz --print-snippet",
        "archive": "/tmp/codeinsight-self-adoption-report.tar.gz",
        "metrics": {
          "selected_lines": 439,
          "total_lines": 28433,
          "line_reduction": "98.5%",
          "mcp_first_call_contract": {
            "reading_order": true,
            "suggested_tool_handoff": true,
            "continuation_after_selected_context": true,
            "suggested_tool_executed": true
          }
        }
      }
    }
  },
  "post_release": {
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
      "skipped": true
    },
    "installed_quickstart": {
      "binary": "/tmp/codeinsight",
      "skipped": true,
      "coverage": ["version", "index", "overview", "context-pack", "agent_route_execution_plan", "reading_plan_question", "reading_plan_reason", "selection_reason", "selection_rank", "continuation_evidence"]
    }
  },
  "handoff_markdown": "## v9.8.7 release handoff"
}
EOF

  cat >"$changelog_notes" <<'EOF'
### Highlights

- Ship release handoff automation.
EOF

  "$ROOT_DIR/scripts/release-notes-draft.sh" \
    --changelog-notes "$changelog_notes" \
    --output "$draft_md" \
    "$handoff_json" >"$TEMP_DIR/output.log"

  grep -Fq "release notes draft written: $draft_md" "$TEMP_DIR/output.log" ||
    fail "missing output diagnostic"
  grep -Fq '## v9.8.7 release notes draft' "$draft_md" ||
    fail "missing draft heading"
  grep -Fq -- '- Ship release handoff automation.' "$draft_md" ||
    fail "missing changelog notes"
  grep -Fq '### Verification Evidence' "$draft_md" ||
    fail "missing verification section"
  grep -Fq -- '- Pre-release CI: [run 123456](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456)' "$draft_md" ||
    fail "missing CI link"
  grep -Fq -- '- `github_asset_downloads`: `metadata_only`' "$draft_md" ||
    fail "missing release gate"
  grep -Fq -- '- Docker image: `ghcr.io/sleticalboy/codeinsight-mcp` (skipped locally)' "$draft_md" ||
    fail "missing Docker distribution check"
  grep -Fq -- '- Benchmark: [codeinsight-benchmark-subset](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/1)' "$draft_md" ||
    fail "missing benchmark artifact"
  grep -Fq -- '- Agent-route first selection: rank `1`, Selected for high relevance via seed_file: Seed file header and imports for task: src/auth.ts' "$draft_md" ||
    fail "missing agent-route first selection"
  grep -Fq -- '- Agent-route continuation: `lower_ranked_context_omitted`, next action `narrow_task_or_seed`' "$draft_md" ||
    fail "missing agent-route continuation"
  grep -Fq -- '- Adoption report: [CodeInsight self adoption report](docs/adoption-report-codeinsight.md)' "$draft_md" ||
    fail "missing adoption report artifact"
  grep -Fq '### Benchmark Evidence' "$draft_md" ||
    fail "missing benchmark evidence section"
  grep -Fq -- '- Routing: `context_pack` first for 1/1 repositories' "$draft_md" ||
    fail "missing benchmark routing"
  grep -Fq -- '- Line reduction: `99.0%`' "$draft_md" ||
    fail "missing benchmark line reduction"
  grep -Fq '### Adoption Report Evidence' "$draft_md" ||
    fail "missing adoption report evidence section"
  grep -Fq -- '- Routed first-read: `439/28433` source lines' "$draft_md" ||
    fail "missing adoption report routed first-read"
  grep -Fq -- '- MCP first-call contract: `reading_order=true`, `suggested_tool_handoff=true`, `continuation_after_selected_context=true`, `suggested_tool_executed=true`' "$draft_md" ||
    fail "missing adoption report MCP contract"

  CODEINSIGHT_ROOT_DIR="$TEMP_DIR" \
    "$ROOT_DIR/scripts/release-notes-draft.sh" v9.8.7 >"$TEMP_DIR/stdout.md"
  grep -Fq '## v9.8.7 release notes draft' "$TEMP_DIR/stdout.md" ||
    fail "tag input did not resolve default handoff JSON"

  if "$ROOT_DIR/scripts/release-notes-draft.sh" "$TEMP_DIR/missing.json" >"$TEMP_DIR/missing.out" 2>"$TEMP_DIR/missing.err"; then
    fail "missing handoff JSON should fail"
  fi
  grep -Fq 'handoff JSON not found' "$TEMP_DIR/missing.err" ||
    fail "missing handoff diagnostic"

  echo "release notes draft smoke passed"
}

main "$@"
