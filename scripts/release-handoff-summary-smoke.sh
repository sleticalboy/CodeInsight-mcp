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
  local generated_evidence_json="$TEMP_DIR/generated-release-evidence/v9.8.7.json"
  local generated_evidence_md="$TEMP_DIR/generated-release-evidence/v9.8.7.md"

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
      "report": "/tmp/benchmark.md",
      "summary": "/tmp/benchmark-summary.json",
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
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/2",
      "summary": "/tmp/context-pack-quality.json"
    },
    "agent_route": {
      "name": "codeinsight-agent-route-smoke",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/3",
      "summary": "/tmp/agent-route.json",
      "metrics": {
        "first_selection_rank": 1,
        "first_selection_reason": "Selected for high relevance via seed_file: Seed file header and imports for task: src/auth.ts",
        "continuation_status": "lower_ranked_context_omitted",
        "continuation_next_action": "narrow_task_or_seed"
      }
    },
    "mcp_first_call": {
      "name": "codeinsight-mcp-first-call",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/4",
      "summary": "/tmp/mcp-first-call.json"
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
    "coverage": ["version", "index", "overview", "context-pack", "agent-route", "mcp_stdio", "mcp_agent_route", "agent_route_execution_plan", "reading_plan_focus", "reading_plan_question", "reading_plan_reason", "selection_reason", "selection_rank", "continuation_evidence"]
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
  grep -Fq -- '- Benchmark routing: `context_pack` first for 1/1 repositories' "$TEMP_DIR/stdout.md" ||
    fail "missing benchmark routing"
  grep -Fq -- '- Benchmark line reduction: `99.0%`' "$TEMP_DIR/stdout.md" ||
    fail "missing benchmark line reduction"
  grep -Fq -- '- `github_asset_downloads`: `metadata_only`' "$TEMP_DIR/stdout.md" ||
    fail "missing release gate"
  grep -Fq -- '- Agent-route artifact: [codeinsight-agent-route-smoke](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/3)' "$TEMP_DIR/stdout.md" ||
    fail "missing agent-route artifact"
  grep -Fq -- '- Agent-route first selection: rank `1`, Selected for high relevance via seed_file: Seed file header and imports for task: src/auth.ts' "$TEMP_DIR/stdout.md" ||
    fail "missing agent-route first selection"
  grep -Fq -- '- Agent-route continuation: `lower_ranked_context_omitted`, next action `narrow_task_or_seed`' "$TEMP_DIR/stdout.md" ||
    fail "missing agent-route continuation"
  grep -Fq -- '- MCP first-call artifact: [codeinsight-mcp-first-call](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/4)' "$TEMP_DIR/stdout.md" ||
    fail "missing MCP first-call artifact"
  grep -Fq -- '- Adoption report: [CodeInsight self adoption report](docs/adoption-report-codeinsight.md)' "$TEMP_DIR/stdout.md" ||
    fail "missing adoption report link"
  grep -Fq -- '- Adoption report routed first-read: `439/28433` source lines, `98.5%` reduction' "$TEMP_DIR/stdout.md" ||
    fail "missing adoption report metrics"
  grep -Fq -- '- Adoption report MCP first-call contract: `reading_order=true`, `suggested_tool_handoff=true`, `continuation_after_selected_context=true`, `suggested_tool_executed=true`' "$TEMP_DIR/stdout.md" ||
    fail "missing adoption report contract"

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
    .pre_release.artifacts.benchmark.metrics.line_reduction == "99.0%" and
    .pre_release.artifacts.adoption_report.metrics.line_reduction == "98.5%" and
    .pre_release.artifacts.adoption_report.metrics.mcp_first_call_contract.reading_order == true and
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

  cat >"$TEMP_DIR/release-evidence-summary" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

json_output=""
repo=""
run_id=""
head_sha=""
tag=""
branch=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      repo="$2"
      shift 2
      ;;
    --run-id)
      run_id="$2"
      shift 2
      ;;
    --head-sha)
      head_sha="$2"
      shift 2
      ;;
    --json-output)
      json_output="$2"
      shift 2
      ;;
    *)
      if [ -z "$tag" ]; then
        tag="$1"
      elif [ -z "$branch" ]; then
        branch="$1"
      else
        echo "unexpected argument: $1" >&2
        exit 2
      fi
      shift
      ;;
  esac
done

test "$repo" = "sleticalboy/CodeInsight-mcp"
test "$run_id" = "123456"
test "$head_sha" = "abc123"
test "$tag" = "v9.8.7"
test "$branch" = "release"
test -n "$json_output"
mkdir -p "$(dirname "$json_output")"

cat >"$json_output" <<JSON
{
  "schema_version": 1,
  "tag": "v9.8.7",
  "branch": "release",
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
      "report": "/tmp/generated-benchmark.md",
      "summary": "/tmp/generated-benchmark-summary.json",
      "metrics": {
        "context_pack_first": 2,
        "routing_total": 2,
        "line_reduction": "97.7%",
        "guardrail_failures": 0,
        "truncated_packs": 0
      }
    },
    "context_pack_quality": {
      "name": "codeinsight-context-pack-quality",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/2",
      "summary": "/tmp/generated-context-pack-quality.json"
    },
    "agent_route": {
      "name": "codeinsight-agent-route-smoke",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/3",
      "summary": "/tmp/generated-agent-route.json",
      "metrics": {
        "first_selection_rank": 2,
        "first_selection_reason": "Selected for medium relevance via dependency: Local dependency of src/main.ts via ./auth",
        "continuation_status": "complete",
        "continuation_next_action": "read_selected_context"
      }
    },
    "mcp_first_call": {
      "name": "codeinsight-mcp-first-call",
      "url": "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/4",
      "summary": "/tmp/generated-mcp-first-call.json"
    },
    "adoption_report": {
      "name": "CodeInsight self adoption report",
      "document": "docs/adoption-report-codeinsight.md",
      "command": "scripts/adoption-report.sh .",
      "archive": "/tmp/codeinsight-self-adoption-report.tar.gz",
      "metrics": {
        "selected_lines": 80,
        "total_lines": 1200,
        "line_reduction": "93.3%",
        "mcp_first_call_contract": {
          "reading_order": true,
          "suggested_tool_handoff": true,
          "continuation_after_selected_context": true,
          "suggested_tool_executed": true
        }
      }
    }
  },
  "release_notes_block": "## v9.8.7 release evidence"
}
JSON

echo "generated release evidence for $tag on $branch"
EOF
  chmod +x "$TEMP_DIR/release-evidence-summary"

  "$ROOT_DIR/scripts/release-handoff-summary.sh" \
    --generate-evidence \
    --repo sleticalboy/CodeInsight-mcp \
    --evidence-run-id 123456 \
    --evidence-head-sha abc123 \
    --evidence-branch release \
    --release-evidence-summary-script "$TEMP_DIR/release-evidence-summary" \
    --evidence-json "$generated_evidence_json" \
    --verification-json "$verification_json" \
    v9.8.7 >"$TEMP_DIR/generated-stdout.md"

  [ -f "$generated_evidence_json" ] ||
    fail "generate-evidence did not write evidence JSON"
  [ -f "$generated_evidence_md" ] ||
    fail "generate-evidence did not write evidence Markdown"
  grep -Fq 'generated release evidence for v9.8.7 on release' "$generated_evidence_md" ||
    fail "generate-evidence did not archive evidence Markdown stdout"
  grep -Fq -- '- Benchmark routing: `context_pack` first for 2/2 repositories' "$TEMP_DIR/generated-stdout.md" ||
    fail "handoff did not use generated evidence benchmark metrics"
  grep -Fq -- '- Agent-route first selection: rank `2`, Selected for medium relevance via dependency: Local dependency of src/main.ts via ./auth' "$TEMP_DIR/generated-stdout.md" ||
    fail "handoff did not use generated evidence agent-route metrics"
  grep -Fq -- '- Adoption report routed first-read: `80/1200` source lines, `93.3%` reduction' "$TEMP_DIR/generated-stdout.md" ||
    fail "handoff did not use generated evidence adoption metrics"
	jq -e '
	  .schema_version == 1 and
	  .tag == "v9.8.7" and
	  .branch == "release" and
	  .artifacts.benchmark.metrics.context_pack_first == 2 and
	  .artifacts.agent_route.metrics.first_selection_rank == 2 and
	  .artifacts.agent_route.metrics.continuation_next_action == "read_selected_context" and
	  .artifacts.adoption_report.metrics.selected_lines == 80
	' "$generated_evidence_json" >/dev/null ||
    fail "generated evidence JSON does not match expected fixture"

  echo "release handoff summary smoke passed"
}

main "$@"
