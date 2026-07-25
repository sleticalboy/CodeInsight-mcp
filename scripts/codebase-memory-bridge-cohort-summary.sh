#!/usr/bin/env bash
set -euo pipefail

OUTPUT="/tmp/codeinsight-codebase-memory-bridge-cohort.md"
JSON_OUTPUT=""
MIN_REPORTS=1
CHECK=false
INPUTS=()
SUMMARY_PATHS=()

fail() {
  echo "codebase-memory bridge cohort summary failed: $*" >&2
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    fail "missing required command: $1"
  fi
}

usage() {
  cat <<'EOF'
usage: scripts/codebase-memory-bridge-cohort-summary.sh [SUMMARY_OR_REPORT_DIR ...] [options]

Aggregate multiple codebase-memory bridge reports into a route-quality cohort
summary. Each input can be a summary.json file or a directory containing one.

Options:
  --output PATH       Markdown output path. Default: /tmp/codeinsight-codebase-memory-bridge-cohort.md.
  --json PATH         JSON output path. Default: <output-dir>/codebase-memory-bridge-cohort-summary.json.
  --min-reports N     Minimum report count for --check. Default: 1.
  --check             Exit non-zero unless enough reports exist and no conflicts are present.
  -h, --help          Show this help text.
EOF
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --output)
        [ "$#" -ge 2 ] || fail "--output requires a path"
        OUTPUT="$2"
        shift 2
        ;;
      --json)
        [ "$#" -ge 2 ] || fail "--json requires a path"
        JSON_OUTPUT="$2"
        shift 2
        ;;
      --min-reports)
        [ "$#" -ge 2 ] || fail "--min-reports requires a number"
        MIN_REPORTS="$2"
        shift 2
        ;;
      --check)
        CHECK=true
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        INPUTS+=("$1")
        shift
        ;;
    esac
  done
}

summary_path_for_input() {
  local input="$1"

  if [ -d "$input" ]; then
    input="$input/summary.json"
  fi
  [ -f "$input" ] || fail "summary JSON does not exist: $input"
  jq empty "$input" >/dev/null || fail "invalid summary JSON: $input"
  printf '%s\n' "$input"
}

validate_args() {
  require_command jq

  [ "${#INPUTS[@]}" -gt 0 ] || fail "provide at least one bridge report summary or report directory"
  case "$MIN_REPORTS" in
    ''|*[!0-9]*) fail "--min-reports must be a positive integer" ;;
  esac
  [ "$MIN_REPORTS" -gt 0 ] || fail "--min-reports must be > 0"

  if [ -z "$JSON_OUTPUT" ]; then
    JSON_OUTPUT="$(dirname "$OUTPUT")/codebase-memory-bridge-cohort-summary.json"
  fi
}

collect_summary_paths() {
  local input
  for input in "${INPUTS[@]}"; do
    SUMMARY_PATHS+=("$(summary_path_for_input "$input")")
  done
}

summary_paths_json() {
  printf '%s\n' "${SUMMARY_PATHS[@]}" | jq -R 'select(length > 0)' | jq -s .
}

write_json_summary() {
  local paths_json="$1"

  mkdir -p "$(dirname "$JSON_OUTPUT")"
  jq -s \
    --argjson paths "$paths_json" \
    --arg output "$OUTPUT" \
    --argjson min_reports "$MIN_REPORTS" '
      def report_row($summary; $path):
        {
          path: $path,
          status: ($summary.status // "unknown"),
          task: ($summary.task // ""),
          provider: ($summary.provider // ""),
          backend_top_file: ($summary.backend.top_file // ""),
          route_first_file: ($summary.route.first_file // ""),
          route_quality_level: ($summary.route.route_quality_level // ""),
          route_quality_score: ($summary.route.route_quality_score // 0),
          route_quality_recommended_action: ($summary.route.route_quality_recommended_action // ""),
          route_warning_count: (($summary.route.route_quality_warnings // []) | length),
          first_file_matches_backend_top: ($summary.agreement.first_file_matches_backend_top // false),
          first_file_in_backend_candidates: ($summary.agreement.first_file_in_backend_candidates // false),
          selected_backend_candidate_count: ($summary.agreement.selected_backend_candidate_count // 0),
          backend_candidate_count: ($summary.agreement.backend_candidate_count // 0),
          next_action: ($summary.next_action // "")
        };

      . as $summaries |
      [range(0; $summaries | length) as $i | report_row($summaries[$i]; $paths[$i])] as $reports |
      ($reports | length) as $report_count |
      ($reports | map(select(.status == "pass")) | length) as $pass_count |
      ($reports | map(select(.status != "pass")) | length) as $warn_count |
      ($reports | map(select(.first_file_matches_backend_top)) | length) as $top_match_count |
      ($reports | map(select(.first_file_in_backend_candidates)) | length) as $candidate_match_count |
      ($reports | map(.selected_backend_candidate_count) | add // 0) as $selected_backend_total |
      ($reports | map(.backend_candidate_count) | add // 0) as $backend_candidate_total |
      ($reports | map(select((.status != "pass") or (.first_file_matches_backend_top | not)))) as $conflicts |
      {
        status: (if $report_count < $min_reports then "insufficient_reports" elif ($conflicts | length) > 0 then "needs_review" else "pass" end),
        report_count: $report_count,
        min_reports: $min_reports,
        pass_count: $pass_count,
        warn_count: $warn_count,
        first_file_top_match_count: $top_match_count,
        first_file_candidate_match_count: $candidate_match_count,
        first_file_top_match_rate: (if $report_count > 0 then (($top_match_count * 10000 / $report_count | floor) / 100) else 0 end),
        first_file_candidate_match_rate: (if $report_count > 0 then (($candidate_match_count * 10000 / $report_count | floor) / 100) else 0 end),
        selected_backend_candidate_count: $selected_backend_total,
        backend_candidate_count: $backend_candidate_total,
        selected_backend_candidate_rate: (if $backend_candidate_total > 0 then (($selected_backend_total * 10000 / $backend_candidate_total | floor) / 100) else 0 end),
        conflicts: $conflicts,
        reports: $reports,
        artifacts: {
          markdown: $output
        },
        next_action: (if $report_count < $min_reports then "collect_more_bridge_reports" elif ($conflicts | length) > 0 then "review_backend_local_conflicts" else "run_more_real_backend_tasks" end)
      }
    ' "${SUMMARY_PATHS[@]}" >"$JSON_OUTPUT"
}

write_markdown() {
  mkdir -p "$(dirname "$OUTPUT")"
  jq -r '
    def tick($value): "`" + ($value | tostring) + "`";
    "# codebase-memory bridge cohort summary",
    "",
    "- Status: " + tick(.status),
    "- Reports: " + tick((.report_count | tostring) + "/" + (.min_reports | tostring)),
    "- Pass reports: " + tick(.pass_count),
    "- Warn reports: " + tick(.warn_count),
    "- First-file top match rate: " + tick((.first_file_top_match_rate | tostring) + "%"),
    "- First-file candidate match rate: " + tick((.first_file_candidate_match_rate | tostring) + "%"),
    "- Selected backend candidate rate: " + tick((.selected_backend_candidate_rate | tostring) + "%"),
    "- Next action: " + tick(.next_action),
    "",
    "| Task | Status | Backend top | Route first | Top match | Candidate match | Route quality | Agent route action | Warnings | Next action |",
    "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    (.reports[] |
      "| " + (if .task == "" then "-" else .task end) +
      " | " + tick(.status) +
      " | " + tick(.backend_top_file) +
      " | " + tick(.route_first_file) +
      " | " + tick(.first_file_matches_backend_top) +
      " | " + tick(.first_file_in_backend_candidates) +
      " | " + tick(.route_quality_level + " " + (.route_quality_score | tostring) + "/100") +
      " | " + tick(if .route_quality_recommended_action == "" then "n/a" else .route_quality_recommended_action end) +
      " | " + tick(.route_warning_count) +
      " | " + tick(.next_action) + " |"
    )
  ' "$JSON_OUTPUT" >"$OUTPUT"
}

run_check() {
  if [ "$CHECK" != true ]; then
    return
  fi

  local status report_count conflict_count
  status="$(jq -r '.status' "$JSON_OUTPUT")"
  report_count="$(jq -r '.report_count' "$JSON_OUTPUT")"
  conflict_count="$(jq -r '.conflicts | length' "$JSON_OUTPUT")"

  [ "$status" = "pass" ] ||
    fail "cohort is not clean: status=$status, reports=$report_count/$MIN_REPORTS, conflicts=$conflict_count"
}

main() {
  parse_args "$@"
  validate_args

  local paths_json
  collect_summary_paths
  paths_json="$(summary_paths_json)"
  write_json_summary "$paths_json"
  write_markdown
  run_check

  echo "codebase-memory bridge cohort summary written to $OUTPUT"
  echo "summary: $JSON_OUTPUT"
}

main "$@"
