#!/usr/bin/env bash
set -euo pipefail

BACKEND_EVIDENCE_JSON=""
AGENT_ROUTE_JSON=""
OUTPUT_DIR=""
TASK=""

fail() {
  echo "codebase-memory bridge report failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

usage() {
  cat <<'EOF'
usage: scripts/codebase-memory-bridge-report.sh --backend-evidence PATH --agent-route-json PATH --output-dir DIR [options]

Summarize how exported codebase-memory evidence aligns with a CodeInsight
agent_route decision. This consumes artifacts; it does not call MCP tools.

Options:
  --backend-evidence PATH  CodeInsight backend_evidence JSON.
  --agent-route-json PATH  Raw codeinsight agent-route JSON.
  --output-dir DIR         Directory for summary.json and markdown report.
  --task TEXT              Optional task label for the report.
  -h, --help               Show this help text.

Output:
  <output-dir>/summary.json
  <output-dir>/codebase-memory-bridge-report.md
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --backend-evidence)
        [ "$#" -ge 2 ] || fail "--backend-evidence requires a path"
        BACKEND_EVIDENCE_JSON="$2"
        shift 2
        ;;
      --agent-route-json)
        [ "$#" -ge 2 ] || fail "--agent-route-json requires a path"
        AGENT_ROUTE_JSON="$2"
        shift 2
        ;;
      --output-dir)
        [ "$#" -ge 2 ] || fail "--output-dir requires a path"
        OUTPUT_DIR="$2"
        shift 2
        ;;
      --task)
        [ "$#" -ge 2 ] || fail "--task requires text"
        TASK="$2"
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

validate_args() {
  require_command jq

  [ -n "$BACKEND_EVIDENCE_JSON" ] || fail "--backend-evidence is required"
  [ -n "$AGENT_ROUTE_JSON" ] || fail "--agent-route-json is required"
  [ -n "$OUTPUT_DIR" ] || fail "--output-dir is required"
  [ -f "$BACKEND_EVIDENCE_JSON" ] || fail "backend evidence JSON does not exist: $BACKEND_EVIDENCE_JSON"
  [ -f "$AGENT_ROUTE_JSON" ] || fail "agent-route JSON does not exist: $AGENT_ROUTE_JSON"

  jq empty "$BACKEND_EVIDENCE_JSON" >/dev/null || fail "invalid backend evidence JSON: $BACKEND_EVIDENCE_JSON"
  jq empty "$AGENT_ROUTE_JSON" >/dev/null || fail "invalid agent-route JSON: $AGENT_ROUTE_JSON"
}

write_summary() {
  local summary_json="$OUTPUT_DIR/summary.json"

  mkdir -p "$OUTPUT_DIR"
  jq -n \
    --slurpfile backend "$BACKEND_EVIDENCE_JSON" \
    --slurpfile route "$AGENT_ROUTE_JSON" \
    --arg task "$TASK" \
    --arg backend_evidence_json "$BACKEND_EVIDENCE_JSON" \
    --arg agent_route_json "$AGENT_ROUTE_JSON" \
    --arg markdown_report "$OUTPUT_DIR/codebase-memory-bridge-report.md" '
      def route_selected_files($route):
        (($route.context_pack.files // []) | map(.file));

      $backend[0] as $b |
      $route[0] as $r |
      ($b.provider // "unknown") as $provider |
      (($b.candidate_files // []) | map(select(type == "string" and length > 0))) as $candidates |
      (route_selected_files($r)) as $selected |
      ($r.routing_decision.first_file // "") as $first_file |
      ($r.routing_decision.route_quality // {}) as $quality |
      (($quality.evidence_sources // []) | map(select(startswith("backend:" + $provider)))) as $backend_quality_sources |
      ([$selected[] as $file | select($candidates | index($file)) | $file]) as $selected_backend_candidates |
      ($candidates[0] // "") as $backend_top_file |
      (($first_file != "") and ($first_file == $backend_top_file)) as $matches_top |
      (($first_file != "") and (($candidates | index($first_file)) != null)) as $in_candidates |
      (($quality.verification_steps // []) | any(. | contains("Treat backend " + $provider + " evidence as advisory"))) as $advisory_step |
      (($r.routing_decision.backend_evidence.provider // "") == $provider
        and (($r.routing_decision.backend_evidence.candidate_files // []) == $candidates)) as $route_preserved_backend_evidence |
      {
        status: (if $matches_top and ($backend_quality_sources | length > 0) and $advisory_step then "pass" else "warn" end),
        task: $task,
        artifacts: {
          backend_evidence_json: $backend_evidence_json,
          agent_route_json: $agent_route_json,
          markdown_report: $markdown_report
        },
        provider: $provider,
        backend: {
          candidate_files: $candidates,
          top_file: $backend_top_file,
          evidence_sources: ($b.evidence_sources // []),
          evidence_count: ($b.evidence_count // 0),
          confidence: ($b.confidence // null),
          latency_ms: ($b.latency_ms // null),
          notes: ($b.notes // [])
        },
        route: {
          first_file: $first_file,
          selected_files: $selected,
          route_quality_level: ($quality.level // ""),
          route_quality_score: ($quality.score // 0),
          route_quality_evidence_count: ($quality.evidence_count // 0),
          route_quality_recommended_action: ($quality.recommended_action // ""),
          route_quality_warnings: ($quality.warnings // []),
          backend_evidence_sources_in_route_quality: $backend_quality_sources,
          backend_advisory_verification_step_present: $advisory_step,
          route_preserved_backend_evidence: $route_preserved_backend_evidence
        },
        agreement: {
          first_file_matches_backend_top: $matches_top,
          first_file_in_backend_candidates: $in_candidates,
          selected_backend_candidate_files: $selected_backend_candidates,
          selected_backend_candidate_count: ($selected_backend_candidates | length),
          backend_candidate_count: ($candidates | length),
          selected_backend_candidate_ratio: (if ($candidates | length) > 0 then (($selected_backend_candidates | length) / ($candidates | length)) else 0 end)
        },
        next_action: (if $matches_top then "use_agent_route_selected_context" elif $in_candidates then "review_backend_order_vs_local_route" else "investigate_backend_local_conflict" end)
      }
    ' >"$summary_json"
}

write_markdown() {
  local summary_json="$OUTPUT_DIR/summary.json"
  local markdown="$OUTPUT_DIR/codebase-memory-bridge-report.md"

  jq -r '
    def tick($value): "`" + ($value | tostring) + "`";
    "# codebase-memory bridge report",
    "",
    "- Status: " + tick(.status),
    "- Task: " + tick(if .task == "" then "unspecified" else .task end),
    "- Provider: " + tick(.provider),
    "- Backend top file: " + tick(.backend.top_file),
    "- CodeInsight first file: " + tick(.route.first_file),
    "- First file matches backend top: " + tick(.agreement.first_file_matches_backend_top),
    "- First file appears in backend candidates: " + tick(.agreement.first_file_in_backend_candidates),
    "- Selected backend candidates: " + tick((.agreement.selected_backend_candidate_files | join(", "))),
    "- Route quality: " + tick(.route.route_quality_level + " " + (.route.route_quality_score | tostring) + "/100"),
    "- Backend evidence sources in route quality: " + tick((.route.backend_evidence_sources_in_route_quality | join(", "))),
    "- Advisory verification step present: " + tick(.route.backend_advisory_verification_step_present),
    "- Next action: " + tick(.next_action),
    "",
    "| Check | Result |",
    "| --- | --- |",
    "| Backend candidate count | " + tick(.agreement.backend_candidate_count) + " |",
    "| Selected backend candidate count | " + tick(.agreement.selected_backend_candidate_count) + " |",
    "| Backend evidence preserved in route JSON | " + tick(.route.route_preserved_backend_evidence) + " |",
    "| Backend evidence count | " + tick(.backend.evidence_count) + " |",
    "| Backend latency ms | " + tick(.backend.latency_ms // "n/a") + " |",
    "| Backend confidence | " + tick(.backend.confidence // "n/a") + " |"
  ' "$summary_json" >"$markdown"
}

main() {
  parse_args "$@"
  validate_args

  write_summary
  write_markdown

  echo "codebase-memory bridge report written to $OUTPUT_DIR/codebase-memory-bridge-report.md"
  echo "summary: $OUTPUT_DIR/summary.json"
}

main "$@"
