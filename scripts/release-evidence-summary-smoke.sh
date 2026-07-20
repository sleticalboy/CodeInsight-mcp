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
  echo "release evidence summary smoke failed: $*" >&2
  exit 1
}

main() {
  TEMP_DIR="$(mktemp -d)"
  trap cleanup EXIT INT TERM

  mkdir -p "$TEMP_DIR/repo/docs"
  cat >"$TEMP_DIR/repo/Cargo.toml" <<'EOF'
[package]
name = "codeinsight"
version = "99.88.77"
edition = "2021"
EOF
  cat >"$TEMP_DIR/repo/docs/install.md" <<'EOF'
Install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/sleticalboy/CodeInsight-mcp/main/scripts/install.sh | CODEINSIGHT_VERSION=v99.88.77 sh
```
EOF
  cat >"$TEMP_DIR/repo/CHANGELOG.md" <<'EOF'
# Changelog

## [Unreleased]

## [99.88.77] - 2026-07-15

- Smoke fixture release notes.
EOF
  cat >"$TEMP_DIR/repo/docs/adoption-report-codeinsight.md" <<'EOF'
# CodeInsight Self Adoption Report

| Metric | Value |
| --- | ---: |
| Blind first-read baseline | `1200` source lines |
| CodeInsight routed first-read | `80` source lines |
| Type-relation edges | `7` |
| Top type-relation target | `EmbeddingProvider` |
| Type-relation graph filter | `base_type` |
| First-read reduction | `93.3%` |

| Contract | Value |
| --- | --- |
| Reading order starts with selected context | `true` |
| Current-step suggested tool matches the reading plan | `true` |
| Continuation is checked after selected context | `true` |
| Suggested tool executed through MCP `tools/call` | `true` |
EOF

  mkdir -p "$TEMP_DIR/bin"

  cat >"$TEMP_DIR/bin/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_EVIDENCE_SMOKE_LOG:?}"
printf 'gh %s\n' "$*" >>"$log"

if [ "$1" = "run" ] && [ "$2" = "list" ]; then
  printf '123456\n'
  exit 0
fi

if [ "$1" = "run" ] && [ "$2" = "view" ]; then
  test "$3" = "123456"
  cat <<'JSON'
{"conclusion":"success","databaseId":123456,"headSha":"abc123","status":"completed","url":"https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456"}
JSON
  exit 0
fi

if [ "$1" = "api" ]; then
  test "$2" = "repos/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts"
  case " $* " in
    *'codeinsight-benchmark-subset'*)
      printf '987654\n'
      ;;
    *'codeinsight-context-pack-quality'*)
      printf '987655\n'
      ;;
    *'codeinsight-agent-route-smoke'*)
      printf '987656\n'
      ;;
    *'codeinsight-mcp-first-call'*)
      printf '987657\n'
      ;;
    *)
      exit 13
      ;;
  esac
  exit 0
fi

exit 12
EOF
  chmod +x "$TEMP_DIR/bin/gh"

  cat >"$TEMP_DIR/benchmark-summary.json" <<'JSON'
{
  "routing": {
    "context_pack_first": 1,
    "total": 1
  },
  "context": {
    "line_reduction": "99.0%",
    "truncated_packs": 0
  },
  "failures": {
    "total": 0
  }
}
JSON

  cat >"$TEMP_DIR/agent-route-summary.json" <<'JSON'
{
  "status": "pass",
  "metrics": {
    "first_selection_rank": 1,
    "first_selection_reason": "Selected for high relevance via seed_file: Seed file header and imports for task: src/auth.ts",
    "continuation_status": "lower_ranked_context_omitted",
    "continuation_next_action": "narrow_task_or_seed"
  }
}
JSON

  cat >"$TEMP_DIR/benchmark-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_EVIDENCE_SMOKE_LOG:?}"
printf 'artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--artifact-name"
test "$4" = "codeinsight-benchmark-subset"
test "$5" = "123456"
echo "benchmark artifact smoke passed"
echo "report: /tmp/codeinsight-benchmark-artifact-123456/report.md"
echo "summary: ${CODEINSIGHT_EVIDENCE_BENCHMARK_SUMMARY:?}"
EOF
  chmod +x "$TEMP_DIR/benchmark-artifact-smoke"

  cat >"$TEMP_DIR/context-pack-quality-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_EVIDENCE_SMOKE_LOG:?}"
printf 'quality-artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--artifact-name"
test "$4" = "codeinsight-context-pack-quality"
test "$5" = "123456"
echo "context-pack quality artifact smoke passed"
echo "summary: /tmp/codeinsight-context-pack-quality-artifact-123456/summary.json"
EOF
  chmod +x "$TEMP_DIR/context-pack-quality-artifact-smoke"

  cat >"$TEMP_DIR/agent-route-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_EVIDENCE_SMOKE_LOG:?}"
printf 'agent-route-artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--artifact-name"
test "$4" = "codeinsight-agent-route-smoke"
test "$5" = "123456"
echo "agent-route artifact smoke passed"
echo "summary: ${CODEINSIGHT_EVIDENCE_AGENT_ROUTE_SUMMARY:?}"
EOF
  chmod +x "$TEMP_DIR/agent-route-artifact-smoke"

  cat >"$TEMP_DIR/mcp-first-call-artifact-smoke" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

log="${CODEINSIGHT_EVIDENCE_SMOKE_LOG:?}"
printf 'mcp-first-call-artifact %s\n' "$*" >>"$log"
test "$1" = "--repo"
test "$2" = "sleticalboy/CodeInsight-mcp"
test "$3" = "--artifact-name"
test "$4" = "codeinsight-mcp-first-call"
test "$5" = "123456"
echo "MCP first-call artifact smoke passed"
echo "summary: /tmp/codeinsight-mcp-first-call-artifact-123456/summary.json"
EOF
  chmod +x "$TEMP_DIR/mcp-first-call-artifact-smoke"

    CODEINSIGHT_EVIDENCE_SMOKE_LOG="$TEMP_DIR/calls.log" \
    CODEINSIGHT_EVIDENCE_BENCHMARK_SUMMARY="$TEMP_DIR/benchmark-summary.json" \
    CODEINSIGHT_EVIDENCE_AGENT_ROUTE_SUMMARY="$TEMP_DIR/agent-route-summary.json" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/agent-route-artifact-smoke" \
    CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-evidence-summary.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      --json-output "$TEMP_DIR/evidence.json" \
      v99.88.77 \
      main >"$TEMP_DIR/output.log"

  grep -Fq 'tag: v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing normalized tag output"
  grep -Fq 'head_sha: abc123' "$TEMP_DIR/output.log" ||
    fail "missing head SHA output"
  grep -Fq 'metadata_cargo: 99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing Cargo metadata output"
  grep -Fq 'metadata_install: v99.88.77' "$TEMP_DIR/output.log" ||
    fail "missing install metadata output"
  grep -Fq 'metadata_changelog: 99.88.77 (2026-07-15)' "$TEMP_DIR/output.log" ||
    fail "missing changelog metadata output"
  grep -Fq 'ci_run: 123456' "$TEMP_DIR/output.log" ||
    fail "missing CI run output"
  grep -Fq 'benchmark_artifact_url: https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987654' "$TEMP_DIR/output.log" ||
    fail "missing benchmark artifact URL"
  grep -Fq "benchmark_summary: $TEMP_DIR/benchmark-summary.json" "$TEMP_DIR/output.log" ||
    fail "missing benchmark summary output"
  grep -Fq 'benchmark_context_pack_first: 1/1' "$TEMP_DIR/output.log" ||
    fail "missing benchmark routing output"
  grep -Fq 'benchmark_line_reduction: 99.0%' "$TEMP_DIR/output.log" ||
    fail "missing benchmark line reduction output"
  grep -Fq 'context_pack_quality_artifact_url: https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987655' "$TEMP_DIR/output.log" ||
    fail "missing context-pack quality artifact URL"
  grep -Fq 'context_pack_quality_summary: /tmp/codeinsight-context-pack-quality-artifact-123456/summary.json' "$TEMP_DIR/output.log" ||
    fail "missing context-pack quality summary output"
  grep -Fq 'agent_route_artifact_url: https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987656' "$TEMP_DIR/output.log" ||
    fail "missing agent-route artifact URL"
  grep -Fq "agent_route_summary: $TEMP_DIR/agent-route-summary.json" "$TEMP_DIR/output.log" ||
    fail "missing agent-route summary output"
  grep -Fq 'agent_route_first_selection_rank: 1' "$TEMP_DIR/output.log" ||
    fail "missing agent-route first selection rank output"
  grep -Fq 'agent_route_first_selection_reason: Selected for high relevance via seed_file: Seed file header and imports for task: src/auth.ts' "$TEMP_DIR/output.log" ||
    fail "missing agent-route first selection reason output"
  grep -Fq 'agent_route_continuation_status: lower_ranked_context_omitted' "$TEMP_DIR/output.log" ||
    fail "missing agent-route continuation status output"
  grep -Fq 'agent_route_continuation_next_action: narrow_task_or_seed' "$TEMP_DIR/output.log" ||
    fail "missing agent-route continuation next action output"
  grep -Fq 'mcp_first_call_artifact_url: https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987657' "$TEMP_DIR/output.log" ||
    fail "missing MCP first-call artifact URL"
  grep -Fq 'mcp_first_call_summary: /tmp/codeinsight-mcp-first-call-artifact-123456/summary.json' "$TEMP_DIR/output.log" ||
    fail "missing MCP first-call summary output"
  grep -Fq 'adoption_report_doc: docs/adoption-report-codeinsight.md' "$TEMP_DIR/output.log" ||
    fail "missing adoption report document output"
  grep -Fq 'adoption_report_archive: /tmp/codeinsight-self-adoption-report.tar.gz' "$TEMP_DIR/output.log" ||
    fail "missing adoption report archive output"
  grep -Fq 'adoption_report_selected_lines: 80/1200' "$TEMP_DIR/output.log" ||
    fail "missing adoption report routed first-read output"
  grep -Fq 'adoption_report_line_reduction: 93.3%' "$TEMP_DIR/output.log" ||
    fail "missing adoption report reduction output"
  grep -Fq 'adoption_report_type_relation_edges: 7' "$TEMP_DIR/output.log" ||
    fail "missing adoption report type-relation edge output"
  grep -Fq 'adoption_report_top_type_relation_target: EmbeddingProvider' "$TEMP_DIR/output.log" ||
    fail "missing adoption report top type-relation target output"
  grep -Fq 'adoption_report_type_relation_filter: base_type' "$TEMP_DIR/output.log" ||
    fail "missing adoption report type-relation filter output"
  grep -Fq 'adoption_report_contract_reading_order: true' "$TEMP_DIR/output.log" ||
    fail "missing adoption report reading order contract output"
  grep -Fq 'adoption_report_contract_suggested_tool_handoff: true' "$TEMP_DIR/output.log" ||
    fail "missing adoption report suggested tool handoff contract output"
  grep -Fq 'adoption_report_contract_continuation_after_selected_context: true' "$TEMP_DIR/output.log" ||
    fail "missing adoption report continuation contract output"
  grep -Fq 'adoption_report_contract_suggested_tool_executed: true' "$TEMP_DIR/output.log" ||
    fail "missing adoption report suggested tool execution contract output"
  grep -Fq '## v99.88.77 release evidence' "$TEMP_DIR/output.log" ||
    fail "missing release notes block"
  grep -Fq -- '- CI: [run 123456](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456)' "$TEMP_DIR/output.log" ||
    fail "missing release notes CI link"
  grep -Fq -- '- Context-pack quality artifact: [codeinsight-context-pack-quality](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987655)' "$TEMP_DIR/output.log" ||
    fail "missing release notes context-pack quality artifact link"
  grep -Fq -- "- Benchmark summary: \`$TEMP_DIR/benchmark-summary.json\`" "$TEMP_DIR/output.log" ||
    fail "missing release notes benchmark summary"
  grep -Fq -- '- Benchmark routing: `context_pack` first for 1/1 repositories' "$TEMP_DIR/output.log" ||
    fail "missing release notes benchmark routing"
  grep -Fq -- '- Agent-route artifact: [codeinsight-agent-route-smoke](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987656)' "$TEMP_DIR/output.log" ||
    fail "missing release notes agent-route artifact link"
  grep -Fq -- '- Agent-route first selection: rank `1`, Selected for high relevance via seed_file: Seed file header and imports for task: src/auth.ts' "$TEMP_DIR/output.log" ||
    fail "missing release notes agent-route first selection"
  grep -Fq -- '- Agent-route continuation: `lower_ranked_context_omitted`, next action `narrow_task_or_seed`' "$TEMP_DIR/output.log" ||
    fail "missing release notes agent-route continuation"
  grep -Fq -- '- MCP first-call artifact: [codeinsight-mcp-first-call](https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987657)' "$TEMP_DIR/output.log" ||
    fail "missing release notes MCP first-call artifact link"
  grep -Fq -- '- Adoption report: [CodeInsight self adoption report](docs/adoption-report-codeinsight.md)' "$TEMP_DIR/output.log" ||
    fail "missing release notes adoption report link"
  grep -Fq -- '- Adoption report routed first-read: `80/1200` source lines, `93.3%` reduction' "$TEMP_DIR/output.log" ||
    fail "missing release notes adoption report metrics"
  grep -Fq -- '- Adoption report type-relation routing: `7` edges, top target `EmbeddingProvider`, graph filter `base_type`' "$TEMP_DIR/output.log" ||
    fail "missing release notes adoption report type-relation routing"
  grep -Fq -- '- Adoption report MCP first-call contract: `reading_order=true`, `suggested_tool_handoff=true`, `continuation_after_selected_context=true`, `suggested_tool_executed=true`' "$TEMP_DIR/output.log" ||
    fail "missing release notes adoption report contract"
  jq -e --arg summary_path "$TEMP_DIR/benchmark-summary.json" '
    .schema_version == 1 and
    .tag == "v99.88.77" and
    .branch == "main" and
    .head_sha == "abc123" and
    .repo == "sleticalboy/CodeInsight-mcp" and
    .metadata.cargo == "99.88.77" and
    .metadata.install == "v99.88.77" and
    .metadata.changelog == "99.88.77 (2026-07-15)" and
    .ci.run_id == "123456" and
    .ci.url == "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456" and
    .artifacts.benchmark.name == "codeinsight-benchmark-subset" and
    .artifacts.benchmark.url == "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987654" and
    .artifacts.benchmark.report == "/tmp/codeinsight-benchmark-artifact-123456/report.md" and
    .artifacts.benchmark.summary == $summary_path and
    .artifacts.benchmark.metrics.context_pack_first == 1 and
    .artifacts.benchmark.metrics.routing_total == 1 and
    .artifacts.benchmark.metrics.line_reduction == "99.0%" and
    .artifacts.benchmark.metrics.guardrail_failures == 0 and
    .artifacts.benchmark.metrics.truncated_packs == 0 and
    .artifacts.context_pack_quality.name == "codeinsight-context-pack-quality" and
    .artifacts.context_pack_quality.url == "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987655" and
    .artifacts.context_pack_quality.summary == "/tmp/codeinsight-context-pack-quality-artifact-123456/summary.json" and
    .artifacts.agent_route.name == "codeinsight-agent-route-smoke" and
    .artifacts.agent_route.url == "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987656" and
    .artifacts.agent_route.summary == "'"$TEMP_DIR"'/agent-route-summary.json" and
    .artifacts.agent_route.metrics.first_selection_rank == 1 and
    .artifacts.agent_route.metrics.first_selection_reason == "Selected for high relevance via seed_file: Seed file header and imports for task: src/auth.ts" and
    .artifacts.agent_route.metrics.continuation_status == "lower_ranked_context_omitted" and
    .artifacts.agent_route.metrics.continuation_next_action == "narrow_task_or_seed" and
    .artifacts.mcp_first_call.name == "codeinsight-mcp-first-call" and
    .artifacts.mcp_first_call.url == "https://github.com/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts/987657" and
    .artifacts.mcp_first_call.summary == "/tmp/codeinsight-mcp-first-call-artifact-123456/summary.json" and
    .artifacts.adoption_report.name == "CodeInsight self adoption report" and
    .artifacts.adoption_report.document == "docs/adoption-report-codeinsight.md" and
    .artifacts.adoption_report.archive == "/tmp/codeinsight-self-adoption-report.tar.gz" and
    .artifacts.adoption_report.metrics.selected_lines == 80 and
    .artifacts.adoption_report.metrics.total_lines == 1200 and
    .artifacts.adoption_report.metrics.line_reduction == "93.3%" and
    .artifacts.adoption_report.metrics.type_relation_edges == 7 and
    .artifacts.adoption_report.metrics.top_type_relation_target == "EmbeddingProvider" and
    .artifacts.adoption_report.metrics.type_relation_recommendation_kinds == ["base_type"] and
    .artifacts.adoption_report.metrics.mcp_first_call_contract.reading_order == true and
    .artifacts.adoption_report.metrics.mcp_first_call_contract.suggested_tool_handoff == true and
    .artifacts.adoption_report.metrics.mcp_first_call_contract.continuation_after_selected_context == true and
    .artifacts.adoption_report.metrics.mcp_first_call_contract.suggested_tool_executed == true and
    (.release_notes_block | contains("## v99.88.77 release evidence")) and
    (.release_notes_block | contains("- Agent-route first selection: rank `1`, Selected for high relevance")) and
    (.release_notes_block | contains("- Agent-route continuation: `lower_ranked_context_omitted`, next action `narrow_task_or_seed`")) and
    (.release_notes_block | contains("- Adoption report routed first-read: `80/1200` source lines")) and
    (.release_notes_block | contains("- Adoption report type-relation routing: `7` edges")) and
    (.release_notes_block | contains("- metadata_cargo: 99.88.77"))
  ' "$TEMP_DIR/evidence.json" >/dev/null ||
    fail "invalid evidence JSON output"

  grep -Fq 'gh run list --repo sleticalboy/CodeInsight-mcp --workflow CI --branch main --status success --limit 20 --json databaseId,headSha --jq map(select(.headSha == "abc123"))[0].databaseId // ""' "$TEMP_DIR/calls.log" ||
    fail "missing head SHA CI lookup"
  grep -Fq 'gh run view 123456 --repo sleticalboy/CodeInsight-mcp --json conclusion,databaseId,headSha,status,url' "$TEMP_DIR/calls.log" ||
    fail "missing CI run validation"
  grep -Fq 'gh api repos/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts --jq .artifacts[] | select(.name == "codeinsight-benchmark-subset") | .id' "$TEMP_DIR/calls.log" ||
    fail "missing benchmark artifact lookup"
  grep -Fq 'gh api repos/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts --jq .artifacts[] | select(.name == "codeinsight-context-pack-quality") | .id' "$TEMP_DIR/calls.log" ||
    fail "missing context-pack quality artifact lookup"
  grep -Fq 'gh api repos/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts --jq .artifacts[] | select(.name == "codeinsight-agent-route-smoke") | .id' "$TEMP_DIR/calls.log" ||
    fail "missing agent-route artifact lookup"
  grep -Fq 'gh api repos/sleticalboy/CodeInsight-mcp/actions/runs/123456/artifacts --jq .artifacts[] | select(.name == "codeinsight-mcp-first-call") | .id' "$TEMP_DIR/calls.log" ||
    fail "missing MCP first-call artifact lookup"
  grep -Fq 'artifact --repo sleticalboy/CodeInsight-mcp --artifact-name codeinsight-benchmark-subset 123456' "$TEMP_DIR/calls.log" ||
    fail "missing benchmark artifact validation"
  grep -Fq 'quality-artifact --repo sleticalboy/CodeInsight-mcp --artifact-name codeinsight-context-pack-quality 123456' "$TEMP_DIR/calls.log" ||
    fail "missing context-pack quality artifact validation"
  grep -Fq 'agent-route-artifact --repo sleticalboy/CodeInsight-mcp --artifact-name codeinsight-agent-route-smoke 123456' "$TEMP_DIR/calls.log" ||
    fail "missing agent-route artifact validation"
  grep -Fq 'mcp-first-call-artifact --repo sleticalboy/CodeInsight-mcp --artifact-name codeinsight-mcp-first-call 123456' "$TEMP_DIR/calls.log" ||
    fail "missing MCP first-call artifact validation"

  CODEINSIGHT_EVIDENCE_SMOKE_LOG="$TEMP_DIR/run-id-calls.log" \
    CODEINSIGHT_EVIDENCE_BENCHMARK_SUMMARY="$TEMP_DIR/benchmark-summary.json" \
    CODEINSIGHT_EVIDENCE_AGENT_ROUTE_SUMMARY="$TEMP_DIR/agent-route-summary.json" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/repo" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/agent-route-artifact-smoke" \
    CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-evidence-summary.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --run-id 123456 \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/run-id-output.log"
  grep -Fq 'ci_run: 123456' "$TEMP_DIR/run-id-output.log" ||
    fail "missing explicit run ID output"
  grep -Fq 'gh run view 123456 --repo sleticalboy/CodeInsight-mcp --json conclusion,databaseId,headSha,status,url' "$TEMP_DIR/run-id-calls.log" ||
    fail "missing explicit run ID validation"
  if grep -Fq 'gh run list' "$TEMP_DIR/run-id-calls.log"; then
    fail "explicit run ID should not resolve CI run by head SHA"
  fi

  mkdir -p "$TEMP_DIR/mismatch/docs"
  cp "$TEMP_DIR/repo/CHANGELOG.md" "$TEMP_DIR/mismatch/CHANGELOG.md"
  cp "$TEMP_DIR/repo/docs/install.md" "$TEMP_DIR/mismatch/docs/install.md"
  cat >"$TEMP_DIR/mismatch/Cargo.toml" <<'EOF'
[package]
name = "codeinsight"
version = "99.88.76"
edition = "2021"
EOF

  if CODEINSIGHT_EVIDENCE_SMOKE_LOG="$TEMP_DIR/mismatch.log" \
    CODEINSIGHT_ROOT_DIR="$TEMP_DIR/mismatch" \
    CODEINSIGHT_BENCHMARK_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/benchmark-artifact-smoke" \
    CODEINSIGHT_CONTEXT_PACK_QUALITY_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/context-pack-quality-artifact-smoke" \
    CODEINSIGHT_AGENT_ROUTE_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/agent-route-artifact-smoke" \
    CODEINSIGHT_MCP_FIRST_CALL_ARTIFACT_SMOKE_SCRIPT="$TEMP_DIR/mcp-first-call-artifact-smoke" \
    PATH="$TEMP_DIR/bin:$PATH" \
    "$ROOT_DIR/scripts/release-evidence-summary.sh" \
      --repo sleticalboy/CodeInsight-mcp \
      --head-sha abc123 \
      v99.88.77 \
      main >"$TEMP_DIR/mismatch.out" 2>"$TEMP_DIR/mismatch.err"; then
    fail "version metadata mismatch should fail"
  fi
  grep -Fq 'Cargo.toml version 99.88.76 does not match 99.88.77' "$TEMP_DIR/mismatch.err" ||
    fail "missing version mismatch diagnostic"

  echo "release evidence summary smoke passed"
}

main "$@"
